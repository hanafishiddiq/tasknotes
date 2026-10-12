#!/usr/bin/env python3
"""Read only the opt-in Tasknotes Issue19 synthetic A-B-B-A metrics row.

The report contains *browser callback timing* and CSS transition activity,
not physical display FPS or Safari's compositor durations. No data mutation.
"""
from pathlib import Path
import json
import sqlite3
import tempfile
import statistics
import sys

MARKER = Path("src-tauri/target/.issue19-hover-data-dir")
EXPECTED = "__ISSUE19_HOVER_QA__"


def report(path: Path) -> int:
    folder = Path(path).resolve(strict=True)
    assert folder.parent == Path(tempfile.gettempdir()).resolve()
    assert folder.name.startswith("tasknotes-hover-ab.")
    assert not folder.is_symlink()
    db = folder / "tasknotes.db"
    assert db.is_file() and not db.is_symlink()
    with sqlite3.connect(db.as_uri() + "?mode=ro", uri=True) as conn:
        assert conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        names = [r[0] for r in conn.execute("SELECT title FROM tasks")]
        assert names and all(n.startswith(EXPECTED) for n in names)
        metrics = conn.execute(
            "SELECT description FROM tasks WHERE title=?",
            (EXPECTED+"METRICS",),
        ).fetchone()
        if not metrics:
            print("PENDING: no synthetic QA results row")
            return 2
        runs = json.loads(metrics[0]).get("runs", [])
    if not runs:
        print("PENDING: Tasknotes QA has not completed any A-B-B-A trials")
        return 2

    print("Synthetic QA only: checked", len(names), "Tasks, stored runs", len(runs))
    for number, item in enumerate(runs):
        phases = item.get("phases", [])
        print("Run", number, "valid=", item.get("valid"), "phases=", len(phases),
              "error=", item.get("error"))
        for phase in phases:
            print(
                " ",phase.get("phase"),"mode=",phase.get("mode"),
                "focus=",not phase.get("focusLost", True),
                "rAF p95ms=",phase.get("rAFP95Ms"),
                "rAF samples=",phase.get("rAFSamples"),
                "CSS transition runs=",phase.get("transitions",{}).get("run"),
                "travelPx=",phase.get("travelPx"),
            )
        if not item.get("valid"):
            continue
        assert [p["phase"] for p in phases] == ["A1","B1","B2","A2"]
        a = [p for p in phases if p["mode"] == "A"]
        b = [p for p in phases if p["mode"] == "B"]
        a_runs = sum(p["transitions"]["run"] for p in a)
        b_runs = sum(p["transitions"]["run"] for p in b)
        print("  A CSS transitions =", a_runs, "B CSS transitions =", b_runs)
        assert a_runs > 0 and b_runs == 0, "CSS experiment did not isolate transition variable"
        a_p95 = statistics.mean(p["rAFP95Ms"] for p in a)
        b_p95 = statistics.mean(p["rAFP95Ms"] for p in b)
        print(f"  rAF callback p95 A={a_p95:.2f}ms B={b_p95:.2f}ms")
        print("  COMPOSITING VERDICT PENDING: a Safari Inspector trace with phase markers is required.")
    return 0


if __name__ == "__main__":
    if not MARKER.is_file():
        print("PENDING: no QA marker")
        sys.exit(2)
    sys.exit(report(Path(MARKER.read_text().strip())))
