# Issue #19 — Safari Inspector export analysis: Standards / Spec review

**Fixed point:** `e52b054` (completed primary-source research and Safari Inspector readiness gate).
**Branch:** `analysis/issue19-safari-inspector-trace`.
**Method:** Two separate manual Standards/Spec checks, **not** an independent human/subagent approval.

## Standards

- **Pass — source ownership:** the user-provided Inspector export remains at its existing `~/Downloads/localhost-recording.json` location. No raw trace, screenshot image, source URL, page content, or personal data is committed.
- **Pass — deterministic analysis:** `scripts/qa/analyze_safari_inspector_trace.py` accepts an explicit JSON input path and validates schema v1, actual wheel events and source size. It emits only aggregate numeric timing, event counts and opaque image-hash equality statistics.
- **Pass — reproducibility:** original file SHA-256 and source byte count are recorded. A numeric-only copy of the output is kept under ignored `src-tauri/target`. Three small synthetic unit tests cover percentile computation, transition/composite classification, screenshot content exclusion and invalid/no-scroll captures.
- **Pass — interpretation controls:** screenshot sampling cadence is **not** equated with physical display FPS; WebKit `composite` event duration is **not** described as GPU presentation time. The exported `rendering-frame` records array is empty despite that instrument being selected, so no fabricated dropped-frame count is reported.
- **Pass — comparison to source:** the only app-specific performance hypothesis is the existing `.task-card` `border-color`/`box-shadow` hover transition in `src/App.css`. The analyzer does not claim Safari identified a selector (composite event records lack selector identifiers).
- **Pass — blast radius:** QA code/report and existing Issue #19 tracker only; no Tauri/WebKit settings, application CSS, Swift binaries, user Tasks/Notes or ChatGrid data are modified.
- **Observation:** an Inspector screenshot-enabled 99 MB capture may alter scrolling responsiveness. A follow-up A/B should compare matching capture settings and ideally a screenshots-disabled sensitivity run.
- **Observation:** temporal proximity between CSS transitions and compositing is observational and may be confounded by pointer and scroll activity. It does not establish independent causal proof.

## Spec — actual scrolling diagnosis

- **Pass:** export version 1, 14.073 seconds, 2,036 records and 396 wheel events; active wheel window spans 4.533 seconds.
- **Pass:** scroll interval has 600 transition-dispatch events, 44 compositing events, with 13 taking more than 16.7ms; compositing p95 28.97ms and total 514.5ms.
- **Pass:** JavaScript wheel-dispatch handlers total 3.4ms, style recalculations total 2.5ms, layout total 6.0ms and paint total 64.5ms during the same wheel window; the largest measured workload lies in compositing.
- **Pass:** within a 100ms lookback of a CSS transition, 25 compositor events average 17.00ms with 11 above 16.7ms; the other 19 average 4.71ms with two above 16.7ms. No causal claim is made.
- **Pass:** 44 screenshot samples during wheel activity, 33 distinct opaque payload hashes, approximate 9.69 screenshot **samples** per second, not physical screen FPS.
- **Open — causal fix:** disabling hover transitions has not yet been measured using matched true Safari rendering timelines; do not mark Issue #19 resolved.
- **Open — independent reviewer:** this report is manual analysis and not a substitute for the independent Standards/Spec reviewer gate on the stacked feature PRs.

**Decision:** Evidence is sufficiently strong to prioritize a *narrow, synthetic-QA-only* `.task-card` transition A/B before making a Tasknotes CSS change. Issue #19 remains OPEN / High.
