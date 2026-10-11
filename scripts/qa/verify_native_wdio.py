#!/usr/bin/env python3
"""Read-only SQLite assertions after both native WebDriverIO application processes."""
import sqlite3
import sys
from pathlib import Path

directory = Path(sys.argv[1]).expanduser().resolve(strict=True)
assert directory.name.startswith("tasknotes-wdio-"), "Refusing any non-synthetic database"
db = directory / "tasknotes.db"
assert db.is_file(), "Native Tasknotes did not create a synthetic SQLite database"

with sqlite3.connect(f"file:{db}?mode=ro", uri=True) as connection:
    assert connection.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
    assert connection.execute("PRAGMA user_version").fetchone()[0] == 3
    rows = connection.execute(
        "SELECT id, description, status, priority, progress, next_step, previous_incomplete_progress "
        "FROM tasks WHERE title=?", ("QA-WDIO-EDITED",),
    ).fetchall()
    assert len(rows) == 1, f"Expected exactly one Task; got {len(rows)}"
    task_id, description, status, priority, progress, next_step, previous_progress = rows[0]
    assert description == "Edited via actual native WebDriver"
    assert (status, priority, progress, previous_progress) == (
        "in_progress", "very_high", 60, 60
    )
    assert next_step == "Verify automated restart"
    entries = connection.execute(
        "SELECT previous_status, status, previous_progress, progress, note "
        "FROM progress_entries WHERE task_id=? ORDER BY sequence", (task_id,),
    ).fetchall()
    assert any(note == "Native acceptance passed" for *_, note in entries)
    assert any(status == "completed" and progress == 100
               for _, status, _, progress, _ in entries)
    assert any(previous == "completed" and status == "in_progress" and progress == 60
               for previous, status, _, progress, _ in entries)
    print(
        f"PASS: native create/edit/status/priority/progress/Next Step/note, "
        f"restart persistence and {len(entries)} journal entries, SQLite v3."
    )
