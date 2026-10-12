#!/usr/bin/env python3
"""Numeric-only analysis of a Safari Web Inspector timeline export.

Issue #19: summarize actual scroll input, style/layout/paint/composite timing,
CSS transition correlation, screenshot sampling cadence and CPU utilization.
Never emit screenshot bytes, URLs, source code, targets, personal page titles
or the input path. Never copy raw trace exports into Git.
"""
from __future__ import annotations

import argparse
import bisect
import collections
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys


def percentile(values: list[float], quantile: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    return round(ordered[math.ceil(len(ordered) * quantile) - 1], 4)


def timing_summary(items: list[dict]) -> dict:
    durations = [
        max(0.0, (entry["endTime"] - entry["startTime"]) * 1000)
        for entry in items
    ]
    return {
        "count": len(durations),
        "total_ms": round(sum(durations), 3),
        "mean_ms": round(statistics.mean(durations), 3) if durations else None,
        "median_ms": percentile(durations, 0.5),
        "p95_ms": percentile(durations, 0.95),
        "max_ms": round(max(durations), 3) if durations else None,
        "over_16_7_ms": sum(value > 16.7 for value in durations),
    }


def summary_from_recording(root: dict) -> dict:
    if root.get("version") != 1 or not isinstance(root.get("recording"), dict):
        raise ValueError("Unsupported Safari Web Inspector timeline schema")
    recording = root["recording"]
    records = recording.get("records", [])
    if not isinstance(records, list):
        raise ValueError("Expected an array of Web Inspector records")

    wheel = sorted(
        [r for r in records if r.get("details") == "wheel"],
        key=lambda record: record["startTime"],
    )
    if not wheel:
        raise ValueError("No wheel input present; this is not a scrolling capture")
    scroll_start = min(r["startTime"] for r in wheel)
    scroll_end = max(r["endTime"] for r in wheel)
    if scroll_end <= scroll_start:
        raise ValueError("Invalid scroll activity range")

    def during(records_: list[dict], key: str = "startTime") -> list[dict]:
        return [r for r in records_ if scroll_start <= r[key] <= scroll_end]

    def by_event(event_type: str) -> list[dict]:
        return [r for r in records if r.get("eventType") == event_type]

    def screenshot_summary(items: list[dict]) -> dict:
        times = sorted(r["timestamp"] for r in items)
        gaps = [(v - u) * 1000 for u, v in zip(times, times[1:])]
        # Hash opaque exported imageData, NOT decrypted/displayed pixel content.
        hashes = [
            hashlib.sha256(str(r.get("imageData", "")).encode("ascii")).hexdigest()
            for r in items
        ]
        return {
            "count": len(items),
            "unique_payload_hashes": len(set(hashes)),
            "consecutive_duplicate_samples": sum(
                a == b for a, b in zip(hashes, hashes[1:])
            ),
            "median_sample_gap_ms": percentile(gaps, 0.5),
            "p95_sample_gap_ms": percentile(gaps, 0.95),
            "sample_events_per_second": round(
                (len(times) - 1) / (times[-1] - times[0]), 3
            ) if len(times) > 1 and times[-1] > times[0] else None,
        }

    event_name_counts = collections.Counter(
        r["details"] for r in records
        if r.get("type") == "timeline-record-type-script"
        and isinstance(r.get("details"), str)
        and r["details"] in {
            "wheel", "transitionrun", "transitionstart", "transitionend",
            "transitioncancel", "pointerover", "pointerout", "mouseover", "mouseout"
        }
    )
    transition_types = ("transitionrun", "transitionstart", "transitionend", "transitioncancel")
    transition_times = sorted(
        r["startTime"] for r in records if r.get("details") in transition_types
    )
    active_transition_times = [
        t for t in transition_times if scroll_start <= t <= scroll_end
    ]
    composites = by_event("composite")
    active_composites = during(composites)

    def recent_transition(record: dict, window_ms: int) -> bool:
        earliest = record["startTime"] - window_ms / 1000
        i = bisect.bisect_left(transition_times, earliest)
        return i < len(transition_times) and transition_times[i] <= record["endTime"]

    near = [r for r in active_composites if recent_transition(r, 100)]
    far = [r for r in active_composites if not recent_transition(r, 100)]

    cpu = [r for r in records if r.get("type") == "timeline-record-type-cpu"]
    active_cpu = during(cpu, "timestamp")
    screenshots = [
        r for r in records if r.get("type") == "timeline-record-type-screenshots"
    ]

    return {
        "schema": "issue19-safari-inspector-numeric-v1",
        "recording_duration_s": round(
            recording["endTime"] - recording["startTime"], 3
        ),
        "timeline_record_count": len(records),
        "rendering_frame_record_count": sum(
            r.get("type") == "timeline-record-type-rendering-frame"
            for r in records
        ),
        "scroll": {
            "wheel_count": len(wheel),
            "active_duration_s": round(scroll_end - scroll_start, 3),
            "start_offset_s": round(scroll_start - recording["startTime"], 3),
            "end_offset_s": round(scroll_end - recording["startTime"], 3),
            "transition_events_during_scroll": len(active_transition_times),
            "transition_names_all": dict(event_name_counts),
        },
        "composite": {
            "whole_recording": timing_summary(composites),
            "during_scroll": timing_summary(active_composites),
            "outside_scroll": timing_summary(
                [r for r in composites if r not in active_composites]
            ),
            "near_transition_within_100ms": timing_summary(near),
            "other_composites_during_scroll": timing_summary(far),
        },
        "paint_during_scroll": timing_summary(during(by_event("paint"))),
        "layout_during_scroll": timing_summary(during(by_event("layout"))),
        "style_recalc_during_scroll": timing_summary(
            during(by_event("recalculate-styles"))
        ),
        "wheel_dispatch_during_scroll": timing_summary(during(wheel)),
        "transition_dispatch_during_scroll": timing_summary(during([
            r for r in records if r.get("details") in transition_types
        ])),
        "screenshots": {
            "whole_recording": screenshot_summary(screenshots),
            "during_scroll": screenshot_summary(during(screenshots, "timestamp")),
        },
        "cpu_samples": {
            "whole_recording_count": len(cpu),
            "during_scroll_count": len(active_cpu),
            "during_scroll_p95_reported_usage": percentile(
                [r["usage"] for r in active_cpu], 0.95
            ),
        },
        "interpretation_limitations": [
            "Safari screenshot sample rate is not physical display FPS.",
            "Compositing time is instrumented WebKit timeline work, not GPU presentation delay.",
            "CSS transition timing correlation does not by itself prove causation.",
            "The trace requests rendering-frame instrumentation but contains no rendering-frame records.",
            "Recording screenshots and Inspector itself may change performance.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="Explicitly selected Safari JSON export")
    parser.add_argument("--output", type=Path, help="Optional NUMERIC-ONLY JSON summary")
    args = parser.parse_args()
    if not args.input.is_file() or args.input.suffix.lower() != ".json":
        parser.error("An existing Safari .json export is required")
    size = args.input.stat().st_size
    if size > 250_000_000:
        parser.error("Refusing unexpectedly large export (>250MB)")
    raw = args.input.read_bytes()
    try:
        import orjson
        parsed = orjson.loads(raw)
    except ImportError:
        parsed = json.loads(raw)
    report = summary_from_recording(parsed)
    report["source_bytes"] = len(raw)
    report["source_sha256"] = hashlib.sha256(raw).hexdigest()
    output = json.dumps(report, sort_keys=True, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output)
        print("Wrote numeric-only report; screenshots, URLs and source code excluded.")
    else:
        sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
