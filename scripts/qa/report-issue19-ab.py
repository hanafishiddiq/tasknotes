#!/usr/bin/env python3
"""Read-only, conservative interpretation of paired Tasknotes debug/release QA."""
import json
from pathlib import Path
import sqlite3
import sys

root=Path(__file__).resolve().parents[2]
identifier="store.adamaja.tasknotes.issue19benchmark"
db=Path.home()/"Library"/"Application Support"/identifier/"tasknotes.db"
manifest=json.loads((root/"src-tauri/target/issue19-native-ab/manifest.json").read_text())
if manifest["appId"]!=identifier or not db.is_file():
    raise SystemExit("Refusing non-QA or nonexistent DB")
if db.is_symlink() or db.parent.is_symlink():
    raise SystemExit("Refusing symlink QA DB")
with sqlite3.connect(f"file:{db}?mode=ro",uri=True) as conn:
    titles=[row[0] for row in conn.execute("SELECT title FROM tasks")]
    if not titles or any(not title.startswith("__ISSUE19_QA_AB__") for title in titles):
        raise SystemExit("Refusing to read non-synthetic Task titles")
    row=conn.execute("SELECT description FROM tasks WHERE title=?",("__ISSUE19_QA_AB__METRICS",)).fetchone()
    if not row:
        raise SystemExit("QA metrics Task missing")
    data=json.loads(row[0])
runs=data.get("runs",[])
if not runs:
    raise SystemExit("No QA metrics recorded")
modes=["debug","release","release","debug"]
result=[]
for i,entry in enumerate(runs):
    name=modes[i] if i<len(modes) else f"run-{i}"
    measurement=entry.get("scroll",{})
    result.append({
        "run":i,
        "profile":name,
        "valid":entry.get("valid",False),
        "idleP95Ms":entry.get("idle",{}).get("frameP95Ms"),
        "scrollP95Ms":measurement.get("frameP95Ms"),
        "scrollFrameSamples":measurement.get("frameSamples"),
        "scrollDistancePx":measurement.get("scrollDistancePx"),
        "elapsedMs":measurement.get("elapsedMs"),
        "focusLost":measurement.get("focusLost")
    })
print(json.dumps({"sameAssetsSha256":manifest["htmlSha256"],
                 "benchmarkSha256":manifest["probeSha256"],
                 "runs":result},indent=2))
if len(result)>=4 and all(x["valid"] for x in result[:4]):
    debug=[result[0]["scrollP95Ms"],result[3]["scrollP95Ms"]]
    release=[result[1]["scrollP95Ms"],result[2]["scrollP95Ms"]]
    debug_avg=sum(debug)/2
    release_avg=sum(release)/2
    print()
    print(f"Paired A-B-B-A debug rAF p95: {debug}, release: {release}")
    print(f"debug mean={debug_avg:.2f}ms; release mean={release_avg:.2f}ms")
    if release_avg < 0.7*debug_avg and debug_avg-release_avg>=30:
        print("CANDIDATE: release profile materially improves rAF callback scheduling. Needs independent focused repeat and physical frame verification.")
    else:
        print("NO QUALIFIED RELEASE IMPROVEMENT under predeclared 30ms / 30% effect rule.")
else:
    print("INCONCLUSIVE: four focus-valid A-B-B-A measurements not yet present.")
