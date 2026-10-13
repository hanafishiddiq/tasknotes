#!/usr/bin/env python3
"""Compare Safari Inspector compositor timings for one marked Issue19 A-B-B-A QA run.

Accepts only an explicitly named Safari JSON file; emits numerical timing,
never screenshot bytes, URLs, DOM or user content. Does not equate Inspector
screenshot sampling or browser callback intervals with physical screen FPS.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import math
import re
import statistics
from pathlib import Path

MARK = re.compile(r"^ISSUE19_HOVER_(A1|B1|B2|A2)_(BEGIN|END)(?:_([0-9a-f]{32}))?$")
LABELS = ("A1","B1","B2","A2")
MAX_TRACE_BYTES = 256 * 1024 * 1024  # 256 MiB, bounded; real export is ~250.3 MB


def trace_within_size_limit(size: int) -> bool:
    return 0 < size <= MAX_TRACE_BYTES


def percentile(items: list[float], q: float) -> float | None:
    if not items:
        return None
    a = sorted(items)
    return round(a[math.ceil(q * len(a)) - 1], 3)


def trace_phases(records: list[dict]) -> list[dict]:
    """Find unique named events emitted by the QA WebView itself.

    Unlike opaque Safari timeline screenshot timestamps, named dispatched
    events provide an explicit, app-owned measurement boundary. Inconclusive
    if Web Inspector does not serialize these events.
    """
    match = {}
    identities = set()
    for event in records:
        name = event.get("details")
        if not isinstance(name,str):
            continue
        parsed = MARK.fullmatch(name)
        if not parsed:
            continue
        if not isinstance(event.get("startTime"),(int,float)):
            continue
        label,side,run_id=parsed.groups()
        identities.add(run_id)
        key=(label,side)
        if key in match:
            raise ValueError("Ambiguous duplicate A/B marker " + label + "-" + side)
        match[key] = event["startTime"]
    if len(match)!=8:
        missing = [f"{label}-{side}" for label in LABELS for side in ("BEGIN","END")
                   if (label,side) not in match]
        raise ValueError("INCONCLUSIVE: Inspector marker events absent: " + ", ".join(missing))
    if len(identities) != 1:
        raise ValueError("INCONCLUSIVE: mixed or missing Inspector trial identity")
    run_id = next(iter(identities))
    phases=[]
    for label in LABELS:
        start=match[(label,"BEGIN")]
        end=match[(label,"END")]
        if start>=end:
            raise ValueError("INCONCLUSIVE: phase timing interval reversed")
        phases.append({"label":label,"mode":label[0],"start":start,"end":end,
                       "durationSeconds":round(end-start,3)})
        if run_id is not None:
            phases[-1]["runId"] = run_id
    if any(a["end"]>=b["start"] for a,b in zip(phases,phases[1:])):
        raise ValueError("INCONCLUSIVE: A-B-B-A phase windows overlap/out of order")
    return phases


def summarize(records: list[dict], phases: list[dict]) -> dict:
    out = {"phases":[]}
    for phase in phases:
        a,b=phase["start"],phase["end"]
        subset=[r for r in records if
                isinstance(r.get("startTime"),(int,float)) and a<=r["startTime"]<=b]
        comps=[r for r in subset if r.get("eventType")=="composite"
               and isinstance(r.get("endTime"),(int,float))]
        durations=[max(0,(r["endTime"]-r["startTime"])*1000) for r in comps]
        transitions=[r for r in subset if r.get("details") in
                     ("transitionrun","transitionstart","transitionend","transitioncancel")]
        wheels=[r for r in subset if r.get("details")=="wheel"]
        sample={
            "label":phase["label"],"mode":phase["mode"],
            "durationSeconds":phase["durationSeconds"],
            "compositeCount":len(comps),
            "compositeP95Ms":percentile(durations,.95),
            "compositeMedianMs":percentile(durations,.5),
            "compositeSlowOver16_7":sum(d>16.7 for d in durations),
            "compositeSlowPerSecond":round(sum(d>16.7 for d in durations)/max(b-a,.001),3),
            "compositeTotalMs":round(sum(durations),3),
            "transitionEvents":len(transitions),
            "transitionRuns":sum(r.get("details")=="transitionrun" for r in transitions),
            "syntheticWheelEvents":len(wheels),
        }
        if "runId" in phase:
            sample["runId"] = phase["runId"]
        out["phases"].append(sample)
    bymode = {mode:[p for p in out["phases"] if p["mode"]==mode] for mode in ("A","B")}
    if all(p["compositeP95Ms"] is not None for p in out["phases"]):
        a_p95=statistics.mean(p["compositeP95Ms"] for p in bymode["A"])
        b_p95=statistics.mean(p["compositeP95Ms"] for p in bymode["B"])
        a_slow=statistics.mean(p["compositeSlowPerSecond"] for p in bymode["A"])
        b_slow=statistics.mean(p["compositeSlowPerSecond"] for p in bymode["B"])
        p95_reduction=(a_p95-b_p95)/a_p95 if a_p95>0 else 0
        slow_reduction=(a_slow-b_slow)/a_slow if a_slow>0 else 0
        out["pairedMean"]={
            "A_p95Ms":round(a_p95,3),"B_p95Ms":round(b_p95,3),
            "A_slowPerSecond":round(a_slow,3),"B_slowPerSecond":round(b_slow,3),
            "p95ReductionPercent":round(p95_reduction*100,1),
            "slowReductionPercent":round(slow_reduction*100,1),
            "baselineExpensiveCompositingPresent":a_slow>0,
            "candidate":a_slow>0 and p95_reduction>=.30 and slow_reduction>=.30,
        }
    out["limitations"]=[
        "A/B is synthetic timer-driven scroll plus a forced hover-like class, not physical trackpad.",
        "Web Inspector composite durations are not GPU frame presentation times.",
        "Small A-B-B-A sample and Inspector recording overhead prohibit causal or product-FPS claims.",
    ]
    return out



def verify_physical_qa(result: dict, run: dict) -> dict:
    """Bind the Inspector summary to the latest isolated QA measurement.

    isTrusted proves browser-side generation, NOT physical hardware origin.
    That last assertion comes from the human-operated test protocol.
    """
    if (run.get("inputMethod") != "physical"
            or run.get("valid") is not True
            or run.get("inputComparable") is not True
            or not isinstance(run.get("completedAt"), (int, float))):
        raise ValueError("INCONCLUSIVE: latest QA run is not a completed valid physical-input run")
    expected = result.get("phases")
    actual = run.get("phases")
    if not isinstance(expected, list) or not isinstance(actual, list) or len(expected) != 4 or len(actual) != 4:
        raise ValueError("INCONCLUSIVE: expected exactly four Inspector and QA phases")
    run_id = run.get("runId")
    if (not isinstance(run_id, str) or not re.fullmatch(r"[0-9a-f]{32}", run_id)
            or any(phase.get("runId") != run_id for phase in expected)):
        raise ValueError("INCONCLUSIVE: Inspector trial identity does not match the latest QA run")
    wheels = []
    distances = []
    for label, inspector, qa in zip(LABELS, expected, actual):
        mode = label[0]
        if (inspector.get("label") != label or inspector.get("mode") != mode
                or qa.get("phase") != label or qa.get("mode") != mode):
            raise ValueError("INCONCLUSIVE: phase identity/order differs between Inspector and QA")
        count = qa.get("trustedWheelEvents")
        if (qa.get("valid") is not True or qa.get("focusLost") is not False
                or qa.get("timedOut") is not False
                or not isinstance(count, int) or count < 35
                or qa.get("wheelEvents") != count
                or not isinstance(qa.get("travelPx"), (int, float)) or qa["travelPx"] < 600
                or not isinstance(qa.get("rAFSamples"), int) or qa["rAFSamples"] < 12):
            raise ValueError("INCONCLUSIVE: QA physical phase lacks focus/trusted-wheel/travel validity")
        css_runs = qa.get("transitions", {}).get("run")
        inspected_runs = inspector.get("transitionRuns")
        if not isinstance(css_runs, int) or not isinstance(inspected_runs, int) or (
            (css_runs <= 0 or inspected_runs <= 0) if mode == "A" else
            (css_runs != 0 or inspected_runs != 0)
        ):
            raise ValueError("INCONCLUSIVE: A/B transition events do not match QA CSS variant")
        inspector_wheels = inspector.get("syntheticWheelEvents")
        if (not isinstance(inspector_wheels, int) or inspector_wheels < 20
                or abs(inspector_wheels - count) > max(5, count * 0.20)):
            raise ValueError("INCONCLUSIVE: Inspector wheel count does not match this QA phase")
        start = qa.get("start", {}).get("pageSeconds")
        end = qa.get("end", {}).get("pageSeconds")
        duration = inspector.get("durationSeconds")
        if (not isinstance(start, (int, float)) or not isinstance(end, (int, float))
                or not isinstance(duration, (int, float)) or end <= start
                or not 5 <= duration <= 9 or abs((end-start)-duration) > 1.5):
            raise ValueError("INCONCLUSIVE: Inspector phase timing does not match the QA run")
        if inspector.get("compositeCount", 0) < 5:
            raise ValueError("INCONCLUSIVE: too few Inspector compositing operations")
        wheels.append(count)
        distances.append(qa["travelPx"])
    if (max(wheels)/min(wheels) > 2.5 or max(distances)/min(distances) > 2.5):
        raise ValueError("INCONCLUSIVE: physical wheel workload differs too much between phases")
    paired = result.get("pairedMean")
    if not isinstance(paired, dict):
        raise ValueError("INCONCLUSIVE: absent Inspector compositor summary")
    # Both untouched A windows must reproduce the high-cost symptom.
    red_baseline = all(result["phases"][i].get("compositeSlowOver16_7", 0) > 0 for i in (0, 3))
    return {
        "qaTrustedWheelVerified": True,
        "baselineRepeatedSlowComposites": red_baseline,
        "candidate": red_baseline and paired.get("candidate") is True,
        "physicalHardwareProvenByIsTrusted": False,
    }


def latest_qa_run(marker: Path) -> dict:
    """Read only the pre-existing synthetic QA Task DB, never user Tasks."""
    import sqlite3
    import tempfile
    if not marker.is_file() or marker.is_symlink():
        raise ValueError("INCONCLUSIVE: missing synthetic QA data marker")
    try:
        folder = Path(marker.read_text().strip()).resolve(strict=True)
    except (OSError, ValueError) as exc:
        raise ValueError("INCONCLUSIVE: invalid QA data directory") from exc
    if (folder.is_symlink() or folder.parent != Path(tempfile.gettempdir()).resolve()
            or not folder.name.startswith("tasknotes-hover-ab.")):
        raise ValueError("INCONCLUSIVE: refusing non-QA data directory")
    db = folder / "tasknotes.db"
    if not db.is_file() or db.is_symlink():
        raise ValueError("INCONCLUSIVE: missing or non-regular QA database")
    with closing(sqlite3.connect(db.as_uri()+"?mode=ro", uri=True)) as conn:
        if conn.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
            raise ValueError("INCONCLUSIVE: QA database integrity check failed")
        titles = [row[0] for row in conn.execute("SELECT title FROM tasks")]
        prefix = "__ISSUE19_HOVER_QA__"
        if len(titles) != 32 or any(not title.startswith(prefix) for title in titles):
            raise ValueError("INCONCLUSIVE: refusing data not exclusively in the QA fixture")
        metrics = conn.execute(
            "SELECT description FROM tasks WHERE title=?", (prefix+"METRICS",)
        ).fetchall()
        if len(metrics) != 1:
            raise ValueError("INCONCLUSIVE: expected exactly one synthetic QA metrics row")
        data = json.loads(metrics[0][0])
    runs = data.get("runs")
    if not isinstance(runs, list) or not runs:
        raise ValueError("INCONCLUSIVE: no completed QA trial recorded")
    return runs[-1]



def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace",type=Path)
    parser.add_argument("--output",type=Path)
    parser.add_argument("--require-physical-qa",action="store_true",
        help="Bind this Inspector export to the latest valid real-wheel QA SQLite run")
    args=parser.parse_args()
    if not args.trace.is_file() or args.trace.suffix.lower()!=".json":
        parser.error("Explicit existing Safari .json export required")
    size=args.trace.stat().st_size
    if not trace_within_size_limit(size):
        parser.error("Refusing empty trace or trace larger than 256 MiB")
    raw=args.trace.read_bytes()
    try:
        import orjson
        trace=orjson.loads(raw)
    except ImportError:
        trace=json.loads(raw)
    recording=trace.get("recording",{})
    records=recording.get("records")
    if trace.get("version")!=1 or not isinstance(records,list):
        parser.error("Unsupported Web Inspector timeline schema")
    phases=trace_phases(records)
    result=summarize(records,phases)
    # Reject materially incomplete / noncomparable profiles.
    if any(p["compositeCount"]<5 or p["syntheticWheelEvents"]<20
           for p in result["phases"]):
        raise ValueError("INCONCLUSIVE: insufficient wheel/composite events in one or more phases")
    # Without a verified physical-input QA run, a numeric win is not a fix candidate.
    result["pairedMean"]["numericCandidate"] = result["pairedMean"]["candidate"]
    result["pairedMean"]["candidate"] = False
    result["qaBinding"] = {"qaTrustedWheelVerified":False}
    if args.require_physical_qa:
        run=latest_qa_run(Path("src-tauri/target/.issue19-hover-data-dir"))
        if not isinstance(run.get("completedAt"), (int, float)):
            raise ValueError("INCONCLUSIVE: QA run has no completion timestamp")
        if args.trace.stat().st_mtime*1000 < run["completedAt"]-5000:
            raise ValueError("INCONCLUSIVE: Inspector export predates the latest QA trial")
        # Numeric method uses the original prospective thresholds; the QA gate
        # additionally insists that both A repetitions reproduce the symptom.
        result["pairedMean"]["candidate"] = result["pairedMean"]["numericCandidate"]
        binding=verify_physical_qa(result,run)
        result["qaBinding"]=binding
        result["pairedMean"]["candidate"]=binding["candidate"]
        result["limitations"][0] = (
            "QA accepted trusted browser wheel events with human-operated trackpad protocol; "
            "Event.isTrusted alone does not prove physical hardware origin."
        )
    result["sourceSha256"]=hashlib.sha256(raw).hexdigest()
    result["sourceBytes"]=size
    serialized=json.dumps(result,indent=2,sort_keys=True)+"\n"
    if args.output:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(serialized)
        print("Stored numeric-only A-B-B-A compositor comparison; raw screenshots omitted.")
    else:
        print(serialized)
    if result.get("pairedMean",{}).get("candidate"):
        print("CANDIDATE ONLY: verify independent trace and real native scrolling before changing product CSS.")
    else:
        print("NOT PROVEN: no qualified compositor improvement; Issue19 remains open.")
    return 0


if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (ValueError, KeyError) as error:
        # An invalid/missing marker is a missing measurement, not a CSS regression.
        print("INCONCLUSIVE: " + str(error).removeprefix("INCONCLUSIVE: "))
        raise SystemExit(2)
