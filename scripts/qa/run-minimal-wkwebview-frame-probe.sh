#!/bin/bash
# Native macOS WebKit comparator for Tasknotes Issue #19.
# Uses synthetic HTML only, no Tauri/React/SQLite/WebDriver or screen capture.
# Exit: 0 = fast; 1 = reproducible slow WebKit callback; 2 = inconclusive.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORKDIR="$(mktemp -d -t tasknotes-wk-probe)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "[Tasknotes WK QA] Compiling minimal independent WKWebView comparator"
swiftc -O -framework AppKit -framework WebKit -framework QuartzCore \
  "$ROOT/scripts/qa/minimal-wkwebview-frame-probe.swift" \
  -o "$WORKDIR/wkframe-probe"

echo "[Tasknotes WK QA] Running synthetic, focus-validated WKWebView / native display-link differential"
"$WORKDIR/wkframe-probe" > "$WORKDIR/result.log"

python3 - "$WORKDIR/result.log" <<'PY'
import json
import re
import sys
from pathlib import Path

lines = Path(sys.argv[1]).read_text().splitlines()
if any("[WK-EMERGENCY-TIMEOUT]" in line for line in lines):
    print("INCONCLUSIVE: WKWebView timed out or lost focus")
    sys.exit(2)
prefix = "[WK-MINIMAL-RESULT] "
results = [json.loads(line[len(prefix):]) for line in lines if line.startswith(prefix)]
if len(results) != 1 or results[0].get("error"):
    print("INCONCLUSIVE: WKWebView produced no valid independent result")
    sys.exit(2)
idle = next((item for item in results[0].get("results", []) if item.get("name") == "idle"), None)
native = next((line for line in lines if line.startswith("[WK-NATIVE-DISPLAY-LINK]")), None)
match = re.search(r"count=(\d+),p95Ms=([\d.]+),screenHz=(\d+)", native or "")
if not idle or not match:
    print("INCONCLUSIVE: missing rAF/display link measurement")
    sys.exit(2)
samples = int(idle.get("sampleCount", 0))
raf_p95 = idle.get("p95Ms")
foreground = idle.get("focusAtStart", False) and not idle.get("lostFocus", True)
native_count = int(match.group(1))
native_p95 = float(match.group(2))
refresh_hz = int(match.group(3))
print(f"[Tasknotes WK QA] Native display link: {native_count} samples, p95={native_p95:.1f}ms, screen={refresh_hz}Hz")
print(f"[Tasknotes WK QA] Focused independent WKWebView: {samples} samples, rAF p95={raf_p95}ms, validFocus={foreground}")
if (samples < 20 or not foreground or native_count < 35
        or native_p95 > 33 or refresh_hz < 60 or raf_p95 is None):
    print("INCONCLUSIVE: insufficient focused samples or unstable host display timing")
    sys.exit(2)
if raf_p95 >= 50:
    print("RED: independent WKWebView rAF p95 violates 50ms experimental threshold; Issue #19 remains unresolved")
    sys.exit(1)
print("GREEN: independent WKWebView rAF p95 below experimental 50ms threshold")
sys.exit(0)
PY
