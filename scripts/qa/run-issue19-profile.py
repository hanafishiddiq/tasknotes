#!/usr/bin/env python3
"""Run one *non-WebDriver* Tauri build against the same isolated QA SQLite.

The two build profiles use identical generated Tasknotes frontend resources.
The runner reads only the exact test-only app-data identifier, never Tasknotes
personal files. It does NOT assert that rAF callbacks measure physical FPS.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
QA_ID = "store.adamaja.tasknotes.issue19benchmark"
APP_NAME = "Tasknotes Issue19 Bench"
METRICS = "__ISSUE19_QA_AB__METRICS"
DATA = Path.home() / "Library" / "Application Support" / QA_ID
DB = DATA / "tasknotes.db"


def get_history():
    if not DB.is_file():
        return []
    # Read-only: do not create a DB accidentally when checking readiness.
    with sqlite3.connect(f"file:{DB}?mode=ro", uri=True, timeout=0.5) as conn:
        match = conn.execute(
            "SELECT description FROM tasks WHERE title=?", (METRICS,)
        ).fetchone()
        return [] if not match else json.loads(match[0]).get("runs", [])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", choices=["debug", "release"])
    parser.add_argument("--timeout", type=int, default=80)
    args = parser.parse_args()
    if args.timeout < 20 or args.timeout > 180:
        parser.error("--timeout must be between 20 and 180 seconds")

    binary = (ROOT / "src-tauri" / "target" / args.profile / "bundle"
              / "macos" / (APP_NAME + ".app") / "Contents" / "MacOS" / "tasknotes")
    if not binary.is_file():
        print("ERROR: QA-only native binary not found: " + str(binary))
        return 2
    config = json.loads((ROOT / "src-tauri" / "target" / "issue19-native-ab"
                         / "issue19-ab.conf.json").read_text())
    if config["identifier"] != QA_ID or config["identifier"] == "store.adamaja.tasknotes":
        raise ValueError("Refusing normal app identifier")
    if DATA.is_symlink() or DB.is_symlink():
        raise ValueError("Refusing symlinked QA database directory")
    if DB.exists():
        with sqlite3.connect(f"file:{DB}?mode=ro", uri=True) as conn:
            titles = [r[0] for r in conn.execute("SELECT title FROM tasks")]
            if not titles or any(not t.startswith("__ISSUE19_QA_AB__") for t in titles):
                raise ValueError("Unexpected data in QA-only DB; refusing app launch")

    previous = len(get_history())
    print(f"[QA AB] Starting {args.profile} app; prior valid/inconclusive runs={previous}", flush=True)
    logs = ROOT / "src-tauri" / "target" / "issue19-native-ab" / ("app-" + args.profile + ".log")
    with logs.open("ab") as log:
        proc = subprocess.Popen([str(binary)], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                start_new_session=True)
        try:
            until = time.monotonic() + args.timeout
            while time.monotonic() < until:
                time.sleep(0.4)
                if proc.poll() is not None:
                    print("[QA AB] Native app terminated before test produced a result")
                    return 2
                try:
                    samples = get_history()
                except (sqlite3.Error, json.JSONDecodeError):
                    continue
                if len(samples) > previous:
                    report = samples[-1]
                    print("[QA AB] " + json.dumps(report, indent=2), flush=True)
                    print(f"[QA AB] Source: {DB} (synthetic QA identifier ONLY)",flush=True)
                    return 0 if report.get("valid") else 3
            print(f"[QA AB] INCONCLUSIVE: no result after {args.timeout}s",flush=True)
            return 3
        finally:
            if proc.poll() is None:
                proc.send_signal(signal.SIGTERM)
                try:
                    proc.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    # Only terminate the subprocess created by this runner.
                    proc.kill()
                    proc.wait(timeout=3)


if __name__ == "__main__":
    sys.exit(main())
