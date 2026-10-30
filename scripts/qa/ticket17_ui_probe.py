#!/usr/bin/env python3
"""Run Ticket 17's UI probe in the real window against a copy of a seeded dataset.

Usage: ticket17_ui_probe.py BINARY DATASET_DIR OUT_JSON

BINARY is `src-tauri/target/release/tasknotes` built with the qa-probe feature.
The copy keeps the seeded dataset clean, because the app takes a backup on launch.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time


def rss_kb(pid):
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True).stdout
    return int(out.strip() or 0)


def main():
    binary, dataset, out = sys.argv[1:4]
    if not os.path.exists(os.path.join(dataset, "synthetic-dataset.json")):
        sys.exit(f"{dataset} holds no synthetic dataset")
    with tempfile.TemporaryDirectory(prefix="tasknotes-ticket17-") as work:
        data = os.path.join(work, "data")
        shutil.copytree(dataset, data)
        report_file = os.path.join(work, "report.json")
        env = dict(os.environ, TASKNOTES_DEV_DATA_DIR=data, TASKNOTES_QA_REPORT=report_file)
        child = subprocess.Popen([binary], env=env)
        peak, deadline = 0, time.time() + 600
        while child.poll() is None and time.time() < deadline:
            peak = max(peak, rss_kb(child.pid))
            time.sleep(0.25)
        if child.poll() is None:
            child.terminate()
            sys.exit("the probe did not finish in 10 minutes")
        if not os.path.exists(report_file):
            sys.exit(f"the app exited with {child.returncode} and no report")
        with open(report_file) as handle:
            report = json.load(handle)
    report["app_process_peak_rss_mb"] = round(peak / 1024, 1)
    with open(out, "w") as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(report, indent=2))
    sys.exit(0 if report.get("ok") else 1)


if __name__ == "__main__":
    main()
