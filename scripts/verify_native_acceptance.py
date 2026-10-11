#!/usr/bin/env python3
"""Read-only QA of the deliberately named synthetic Tasknotes desktop fixture.

Usage:
    python3 scripts/verify_native_acceptance.py --data-dir "$(cat src-tauri/target/.tasknotes-acceptance-data-dir)"

Refuses all normal application-data paths. No data is written.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import sqlite3
import sys


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir", required=True, help="Synthetic acceptance test directory")
    args = parser.parse_args()

    directory = Path(args.data_dir).expanduser().resolve(strict=True)
    if not directory.name.startswith("tasknotes-acceptance-native."):
        print("REFUSED: not an explicitly named synthetic Tasknotes acceptance directory")
        return 2
    database = directory / "tasknotes.db"
    if not database.is_file():
        print("PENDING: no synthetic Tasknotes database found")
        return 3

    passed = 0
    failed = 0

    def check(label: str, condition: bool) -> None:
        nonlocal passed, failed
        passed += int(condition)
        failed += int(not condition)
        print(f'{"PASS" if condition else "NOT VERIFIED"}: {label}')

    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as conn:
        check("Database integrity", conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok")
        check("SQLite v3", conn.execute("PRAGMA user_version").fetchone()[0] == 3)
        rows = conn.execute(
            "SELECT id,title,description,status,priority,progress,next_step,"
            "revision,previous_incomplete_progress "
            "FROM tasks WHERE title='QA-NATIVE-EDITED'"
        ).fetchall()
        check("Edited synthetic Task exists exactly once", len(rows) == 1)
        if len(rows) != 1:
            return 1
        (task_id, _, description, status, priority, progress, next_step,
         revision, previous_incomplete) = rows[0]
        check("Edited description", description == "Edited via native GUI")
        check("In Progress with Very High priority", status == "in_progress" and priority == "very_high")
        check("Reopened at 60%, previous progress retained", progress == 60 and previous_incomplete == 60)
        check("Next Step saved", next_step.strip().casefold() == "verify restart")
        check("Revision reflects native edits", revision >= 5)
        events = conn.execute(
            "SELECT previous_status,status,previous_progress,progress,note "
            "FROM progress_entries WHERE task_id=? ORDER BY sequence", (task_id,)
        ).fetchall()
        check("Completed at 100%", any(status == "completed" and progress == 100
                                       for _, status, _, progress, _ in events))
        check("Completed → In Progress at 60%",
              any(before == "completed" and status == "in_progress" and progress == 60
                  for before, status, _, progress, _ in events))
        check("Progress note saved",
              any(note.strip() == "Native acceptance passed" for *_, note in events))

    print(f"Summary: {passed} passed, {failed} not verified; read-only, synthetic.")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
