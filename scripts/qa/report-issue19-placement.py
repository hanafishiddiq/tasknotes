#!/usr/bin/env python3
"""Read-only ABBA decision for Issue19 native TOP vs CHILD WKWebView.

Metrics are rAF callback intervals, not physical display FPS. Exit 2 for
invalid/incomplete paired experiments and never mislabel a focus-loss as RED.
"""
import json
import sqlite3
from pathlib import Path
import statistics
import sys

ROOT=Path(__file__).resolve().parents[2]
TEST_DIR=ROOT/"src-tauri/target/issue19-placement"
IDENTIFIER="store.adamaja.tasknotes.issue19placement"
DB=Path.home()/"Library/Application Support"/IDENTIFIER/"tasknotes.db"
assert not DB.is_symlink() and not DB.parent.is_symlink()
MANIFEST=json.loads((TEST_DIR/"manifest.json").read_text())
assert MANIFEST["applicationId"]==IDENTIFIER

def summarize(rows, name, expected):
    if len(rows)<4:
        print(f"INCONCLUSIVE: {name} needs four matched runs")
        return False
    selected=rows[:4]
    for i,(r,mode) in enumerate(zip(selected,expected)):
        actual=r.get("webviewLabel")
        if not r.get("valid") or actual != mode or r["scroll"]["frameSamples"]<12:
            print(f"INCONCLUSIVE {name} run {i}: valid={r.get('valid')}, webview={actual} expected={mode}")
            return False
        if r.get("viewport") != [1120,708] or r.get("scrollHeight") != 3039:
            print(f"INCONCLUSIVE {name} run {i}: mismatched frontend size/scroll workload")
            return False
    top=[selected[i]["scroll"]["frameP95Ms"] for i in [0,3]]
    child=[selected[i]["scroll"]["frameP95Ms"] for i in [1,2]]
    a=statistics.mean(top);b=statistics.mean(child)
    delta=a-b
    print(f"{name}: TOP p95={top} mean={a:.2f}ms; CHILD p95={child} mean={b:.2f}ms")
    print(f"  CHILD improvement={delta:+.2f}ms ({delta/a*100:+.1f}%)")
    if delta>=30 and delta/a>=0.30:
        print("  CANDIDATE: child materially improves rAF (needs independent release/mouse validation).")
    else:
        print("  REJECT: child did not meet predeclared 30ms AND 30% threshold.")
    return True

with sqlite3.connect(f"file:{DB}?mode=ro",uri=True) as conn:
    assert conn.execute("PRAGMA integrity_check").fetchone()[0]=="ok"
    titles=[r[0] for r in conn.execute("SELECT title FROM tasks")]
    assert titles and all(x.startswith("__ISSUE19_QA_AB__") for x in titles)
    row=conn.execute("SELECT description FROM tasks WHERE title='__ISSUE19_QA_AB__METRICS'").fetchone()
    assert row is not None
    release=json.loads(row[0])["runs"]
archive=json.loads((TEST_DIR/"debug-placement-results.json").read_text())
assert archive["assets"]["htmlSha256"]==MANIFEST["htmlSha256"]
assert archive["assets"]["probeSha256"]==MANIFEST["probeSha256"]
print("Identical frontend/probe SHA256:",MANIFEST["htmlSha256"],MANIFEST["probeSha256"])
debug=archive["runs"]
if len(debug)>=6:
    # The two early focus-loss runs were preserved, excluded from valid A-B-B-A.
    summarize(debug[2:6],"DEBUG",["main","qa-content","qa-content","main"])
    controls=debug[6:8]
    if len(controls)==2 and all(r.get("valid") for r in controls):
        print("DEBUG child no-throttle p95:",[r["scroll"]["frameP95Ms"] for r in controls])
valid_release=summarize(release,"RELEASE",["main","qa-content","qa-content","main"])
if len(release)>=6 and all(r.get("valid") for r in release[4:6]):
    print("RELEASE child no-throttle p95:",[r["scroll"]["frameP95Ms"] for r in release[4:6]])
print(f"Raw synthetic dataset: debug {len(debug)} records, release {len(release)} records. No personal Tasks.")
sys.exit(0 if valid_release else 2)
