#!/usr/bin/env python3
"""Count frames that really change on screen while a trusted wheel scrolls a window.

Usage: ticket17_visual_fps.py LABEL PID X Y W H [SECONDS] [PX_PER_SECOND]
X Y W H: the rectangle, in screen points, that scrolls under the pointer.

Put the window in front first and leave the Mac alone: this moves the pointer and
records the screen. Scripted scrolling reads 60 FPS where a real scroll drops frames,
so this is the number to trust. A steady scroll changes every frame, so a repeated
frame on screen is a dropped frame. Reports stay in the scratch directory in
TASKNOTES_FPS_DIR (default: the temp directory): they name other running apps.
"""
import ctypes
import ctypes.util
import json
import os
import re
import subprocess
import sys
import tempfile
import threading
import time

label, pid, x, y, w, h = sys.argv[1], *map(int, sys.argv[2:7])
seconds = float(sys.argv[7]) if len(sys.argv) > 7 else 8.0
speed = float(sys.argv[8]) if len(sys.argv) > 8 else 1200.0
out_dir = os.environ.get("TASKNOTES_FPS_DIR", tempfile.gettempdir())


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


cg = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))
cf = ctypes.CDLL(ctypes.util.find_library("CoreFoundation"))
cg.CGEventCreateScrollWheelEvent2.restype = ctypes.c_void_p
cg.CGEventCreateScrollWheelEvent2.argtypes = [
    ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32]
cg.CGEventSetIntegerValueField.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int64]
cg.CGEventSetLocation.argtypes = [ctypes.c_void_p, Point]
cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
cg.CGWarpMouseCursorPosition.argtypes = [Point]
cg.CGEventCreate.restype = ctypes.c_void_p
cg.CGEventCreate.argtypes = [ctypes.c_void_p]
cg.CGEventGetLocation.restype = Point
cg.CGEventGetLocation.argtypes = [ctypes.c_void_p]
cf.CFRelease.argtypes = [ctypes.c_void_p]

UNIT_PIXEL = 0
IS_CONTINUOUS, POINT_DELTA, PHASE = 88, 96, 99
BEGAN, CHANGED, ENDED = 1, 2, 4


def wheel(center, delta, phase):
    event = cg.CGEventCreateScrollWheelEvent2(None, UNIT_PIXEL, 1, delta, 0, 0)
    cg.CGEventSetLocation(event, center)
    cg.CGEventSetIntegerValueField(event, IS_CONTINUOUS, 1)
    cg.CGEventSetIntegerValueField(event, POINT_DELTA, delta)
    cg.CGEventSetIntegerValueField(event, PHASE, phase)
    cg.CGEventPost(0, event)
    cf.CFRelease(event)


def scroll(center, stop_at):
    """Up and down at a steady speed, 120 wheel events a second, like a trackpad."""
    step = speed / 120
    direction = -1
    while time.perf_counter() < stop_at:
        phase = BEGAN
        turn = min(stop_at, time.perf_counter() + 1.6)
        tick = time.perf_counter()
        while time.perf_counter() < turn:
            wheel(center, int(round(direction * step)), phase)
            phase = CHANGED
            tick += 1 / 120
            pause = tick - time.perf_counter()
            if pause > 0:
                time.sleep(pause)
        wheel(center, 0, ENDED)
        direction = -direction
        time.sleep(0.05)


def load():
    busy = subprocess.run(["ps", "-Ao", "pcpu=,comm="], capture_output=True, text=True).stdout
    top = sorted(((float(l.split(None, 1)[0]), os.path.basename(l.split(None, 1)[1].strip()))
                  for l in busy.splitlines() if l.strip()), reverse=True)[:3]
    return {"load_1m": round(os.getloadavg()[0], 1), "top": top}


size = subprocess.run(
    ["ffmpeg", "-hide_banner", "-f", "avfoundation", "-framerate", "60", "-capture_cursor", "0",
     "-i", "2:none", "-t", "0.1", "-f", "null", "-"], capture_output=True, text=True).stderr
width = int(re.search(r"Stream #0:0.*?(\d{3,5})x(\d{3,5})", size).group(1))
scale = width / 1710
crop = f"crop={int(w * scale)}:{int(h * scale)}:{int(x * scale)}:{int(y * scale)}"

saved = cg.CGEventCreate(None)
pointer = cg.CGEventGetLocation(saved)
cf.CFRelease(saved)
center = Point(x + w / 2, y + h / 2)
cg.CGWarpMouseCursorPosition(center)
time.sleep(0.2)

before = load()
md5 = os.path.join(out_dir, f"{label}.md5")
capture = subprocess.Popen(
    ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "avfoundation", "-framerate", "60",
     "-capture_cursor", "0", "-i", "2:none", "-t", str(seconds + 1.5), "-vf", crop,
     "-f", "framemd5", md5], stderr=subprocess.PIPE, text=True)
time.sleep(1.0)
started = time.perf_counter()
worker = threading.Thread(target=scroll, args=(center, started + seconds))
worker.start()
worker.join()
_, errors = capture.communicate()
after = load()
cg.CGWarpMouseCursorPosition(pointer)

rows = []
timebase = 1 / 1_000_000
for line in open(md5):
    if line.startswith("#"):
        match = re.match(r"#tb 0: (\d+)/(\d+)", line)
        if match:
            timebase = int(match.group(1)) / int(match.group(2))
        continue
    parts = [p.strip() for p in line.split(",")]
    rows.append((int(parts[1]) * timebase, parts[5]))

changes = [t for (t, digest), (_, prior) in zip(rows[1:], rows) if digest != prior]
gaps = sorted(b - a for a, b in zip(changes, changes[1:]))
span = (changes[-1] - changes[0]) if len(changes) > 1 else 0
per_second = {}
for t in changes:
    per_second[int(t - changes[0])] = per_second.get(int(t - changes[0]), 0) + 1
captured = len(rows) / max(rows[-1][0] - rows[0][0], 0.001) if rows else 0


def pct(p):
    return round(gaps[min(len(gaps) - 1, int(len(gaps) * p))] * 1000, 1) if gaps else None


report = {
    "label": label,
    "captured_fps": round(captured, 1),
    "changed_frames": len(changes),
    "visual_fps": round(len(changes) / span, 1) if span else 0,
    "per_second": list(per_second.values()),
    "gap_ms": {"p50": pct(.5), "p95": pct(.95), "p99": pct(.99),
               "max": round(gaps[-1] * 1000, 1) if gaps else None},
    "gaps_over_20ms": sum(g > .020 for g in gaps),
    "gaps_over_34ms": sum(g > .034 for g in gaps),
    "before": before, "after": after, "errors": errors[-200:],
}
json.dump(report, open(os.path.join(out_dir, f"{label}.json"), "w"), indent=1)
print(json.dumps(report))
