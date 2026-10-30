#!/usr/bin/env python3
"""Run Ticket 17's UI probe in the real window against a copy of a seeded dataset.

Usage: ticket17_ui_probe.py BINARY DATASET_DIR OUT_JSON

BINARY is `src-tauri/target/release/tasknotes` built with the qa-probe feature.
The copy keeps the seeded dataset clean, because the app takes a backup on launch.
"""
import ctypes
import ctypes.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time


def screen_state():
    """Frames mean nothing on a sleeping or locked screen; the report says which it was."""
    graphics = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))
    graphics.CGMainDisplayID.restype = ctypes.c_uint32
    graphics.CGDisplayIsAsleep.argtypes = [ctypes.c_uint32]
    session = subprocess.run(["ioreg", "-n", "Root", "-d1"], capture_output=True, text=True).stdout
    return {
        "display_asleep": bool(graphics.CGDisplayIsAsleep(graphics.CGMainDisplayID())),
        "screen_locked": "CGSSessionScreenIsLocked" in session,
    }


def rss_kb(pid):
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True).stdout
    return int(out.strip() or 0)


def webkit_processes():
    """WebKit's helper processes by pid: the page's DOM and script live there."""
    out = subprocess.run(["ps", "-axo", "pid=,rss=,comm="], capture_output=True, text=True).stdout
    found = {}
    for line in out.splitlines():
        pid, rss, command = line.split(None, 2)
        if "com.apple.WebKit" in command:
            found[int(pid)] = int(rss)
    return found


def main():
    binary, dataset, out = sys.argv[1:4]
    with open(binary, "rb") as handle:
        if b"TASKNOTES_QA_REPORT" not in handle.read():
            sys.exit(f"{binary} is not a qa-probe build; a normal build would open real data")
    before = screen_state()
    others = set(webkit_processes())
    if not os.path.exists(os.path.join(dataset, "synthetic-dataset.json")):
        sys.exit(f"{dataset} holds no synthetic dataset")
    with tempfile.TemporaryDirectory(prefix="tasknotes-ticket17-") as work:
        data = os.path.join(work, "data")
        shutil.copytree(dataset, data)
        report_file = os.path.join(work, "report.json")
        env = dict(os.environ, TASKNOTES_DEV_DATA_DIR=data, TASKNOTES_QA_REPORT=report_file)
        child = subprocess.Popen([binary], env=env)
        peak, webkit_peak, deadline = 0, 0, time.time() + 480
        while child.poll() is None and time.time() < deadline:
            peak = max(peak, rss_kb(child.pid))
            ours = webkit_processes()
            webkit_peak = max(webkit_peak, sum(rss for pid, rss in ours.items() if pid not in others))
            time.sleep(0.25)
        if child.poll() is None:
            child.terminate()
            sys.exit("the probe did not finish in 8 minutes")
        if not os.path.exists(report_file):
            sys.exit(f"the app exited with {child.returncode} and no report")
        with open(report_file) as handle:
            report = json.load(handle)
    report["app_process_peak_rss_mb"] = round(peak / 1024, 1)
    report["webkit_processes_peak_rss_mb"] = round(webkit_peak / 1024, 1)
    after = screen_state()
    report["screen"] = {"before": before, "after": after}
    report["frame_timings_valid"] = bool(report.get("frames_flow")) and not any(
        state["display_asleep"] or state["screen_locked"] for state in (before, after)
    )
    with open(out, "w") as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(report, indent=2))
    sys.exit(0 if report.get("ok") else 1)


if __name__ == "__main__":
    main()
