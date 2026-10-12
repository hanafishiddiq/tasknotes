#!/usr/bin/env python3
"""Compare Safari Inspector compositor timings for one marked Issue19 A-B-B-A QA run.

Accepts only an explicitly named Safari JSON file; emits numerical timing,
never screenshot bytes, URLs, DOM or user content. Does not equate Inspector
screenshot sampling or browser callback intervals with physical screen FPS.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import statistics
from pathlib import Path

MARK = re.compile(r"^ISSUE19_HOVER_(A1|B1|B2|A2)_(BEGIN|END)$")
LABELS = ("A1","B1","B2","A2")


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
    for event in records:
        name = event.get("details")
        if not isinstance(name,str):
            continue
        parsed = MARK.fullmatch(name)
        if not parsed:
            continue
        if not isinstance(event.get("startTime"),(int,float)):
            continue
        label,side=parsed.groups()
        key=(label,side)
        if key in match:
            raise ValueError("Ambiguous duplicate A/B marker " + label + "-" + side)
        match[key] = event["startTime"]
    if len(match)!=8:
        missing = [f"{label}-{side}" for label in LABELS for side in ("BEGIN","END")
                   if (label,side) not in match]
        raise ValueError("INCONCLUSIVE: Inspector marker events absent: " + ", ".join(missing))
    phases=[]
    for label in LABELS:
        start=match[(label,"BEGIN")]
        end=match[(label,"END")]
        if start>=end:
            raise ValueError("INCONCLUSIVE: phase timing interval reversed")
        phases.append({"label":label,"mode":label[0],"start":start,"end":end,
                       "durationSeconds":round(end-start,3)})
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
            "candidate":p95_reduction>=.30 and slow_reduction>=.30,
        }
    out["limitations"]=[
        "A/B is synthetic timer-driven scroll plus a forced hover-like class, not physical trackpad.",
        "Web Inspector composite durations are not GPU frame presentation times.",
        "Small A-B-B-A sample and Inspector recording overhead prohibit causal or product-FPS claims.",
    ]
    return out


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace",type=Path)
    parser.add_argument("--output",type=Path)
    args=parser.parse_args()
    if not args.trace.is_file() or args.trace.suffix.lower()!=".json":
        parser.error("Explicit existing Safari .json export required")
    size=args.trace.stat().st_size
    if size>250_000_000:
        parser.error("Refusing trace larger than 250MB")
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
