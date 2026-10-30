#!/bin/sh
# Ticket 17's on-screen measurement. Run it with the Mac unlocked and the screen
# on, and leave the window alone for about three minutes. Synthetic data only:
# the measuring build refuses to start without the synthetic dataset.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
dataset="${TMPDIR:-/tmp}/tasknotes-ticket17-dataset"
cd "$root"
if [ ! -f "$dataset/synthetic-dataset.json" ]; then
  (cd crates/tasknotes-bench && cargo run --release -- seed "$dataset")
fi
VITE_TASKNOTES_PERF_PROBE=1 pnpm tauri build --no-bundle --features qa-probe
python3 -I scripts/qa/ticket17_ui_probe.py src-tauri/target/release/tasknotes "$dataset" docs/qa/ticket-17-results/ui.json >/dev/null
python3 -I - <<'SUMMARY'
import json
report = json.load(open("docs/qa/ticket-17-results/ui.json"))
print("Frame timings valid:", report["frame_timings_valid"], report["screen"])
for name in ("scroll_collapsed", "scroll_with_open_row", "scroll_note_list"):
    print(name, report[name])
print("Saved docs/qa/ticket-17-results/ui.json. Tell Claude it is done.")
SUMMARY
