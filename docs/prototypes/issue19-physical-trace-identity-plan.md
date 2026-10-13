# Issue 19 — continuation and exact physical-trial identity

Date: 2026-10-13. Base/fixed review point: `d9121c0da27ce4081cc46ea3d03933991908b424` (Draft PR #10). Branch: `fix/issue19-physical-trace-identity`.

## Problem definition

The product bug is severe human-perceived native scrolling lag, not slow rAF alone. The latest read-only synthetic QA report still has three scripted runs (two invalid, one valid); no physical trial exists. Downloads contains the two already analyzed recordings. Do not repeat debug/release, main/child, throttling, blank-DOM, scripted CSS or capture experiments without new evidence. No product fix is established.

A separate, deterministic QA defect was reproduced during continuation: `verify_physical_qa` accepts a trace from a different physical trial if counts/durations/CSS states match. A file modification timestamp is not identity. This can invalidate the next decision about a CSS candidate; fixing this narrow defect is justified before asking the owner to capture again.

## Planning before implementation

1. RED/GREEN at the existing public numeric-comparator seam: reject missing/different trial identity while retaining matched physical acceptance and historical numeric-only analysis.
2. RED/GREEN at the QA trace protocol seam: generate one random non-secret ID per execution, persist it in the QA metrics row, append that ID to the eight existing phase BEGIN/END markers. Require the same ID on all eight; reject mixed/incomplete marker sets. This reuses the already captured protocol rather than adding a new timing envelope or more measured events. Preserve old scripted recordings for numeric-only analysis.
3. Add these already-existing QA unit suites to CI, rebuild only the opt-in synthetic QA bundle, verify production CSS/Rust/data paths unchanged. No new benchmark or changed performance threshold.
4. Review the diff against the fixed point along Standards and Spec. Independent `agy` not found on PATH or its standard app CLI path; do not claim self-review is independent. Commit/push and open stacked Draft PR, never merge/release.
5. Prepare the verified QA app for one human physical A-B-B-A capture. The physical-input and Safari Inspector interaction gate remains owner-only under current controller restrictions. End at that real blocker, not by inventing a product fix.

## Evidence and acceptance

Actual pre-change command: `PYTHONPATH=scripts/qa python3` importing `HoverTraceComparisonTests.valid_physical_pair` and calling `verify_physical_qa` with trace ID `11111111111111111111111111111111` vs latest QA ID `22222222222222222222222222222222`.
Observed exit 1: `RED: unrelated physical recording accepted when duration/counts match; no run identity check`.

ID matching is protection against accidentally mixing recordings, not cryptographic attestation of hardware input or defense against fabricated evidence. Keep the predeclared focus/trust/workload/timing checks, both-A expensive-composite prerequisite and both 30% criteria unchanged. Product acceptance still requires actual user-visible improvement, no functional regressions and independent review before any approved merge/release.

## Primary-source research (checked 2026-10-13)

- DOM Standard: `isTrusted` distinguishes user-agent dispatch from script dispatch; it does not attest that a physical trackpad was used. https://dom.spec.whatwg.org/#dom-event-istrusted
- WebKit Timelines: recordings may be resumed/appended; scripts can emit timestamp/profile markers. Screenshots are collected after composites, not a physical present counter. Media & Animations instrumentation can itself affect performance. https://webkit.org/web-inspector/timelines-tab/

Therefore use a fresh, explicitly bounded recording and exact trial identity, disable Screenshots as the existing runbook requires, keep instrument choices equal across phases, and treat any Inspector candidate as provisional pending uninstrumented native acceptance. No claim that instrumentation caused the original lag.

## Safety

Use only existing synthetic QA fixtures. Never publish raw Safari exports, screenshots, personal Task data, SQLite or secrets. Preserve the unrelated untracked `MEMORY.md`. Do not manipulate Safari through shell, inject system input, use private WebKit APIs, install Xcode, or modify global settings.
