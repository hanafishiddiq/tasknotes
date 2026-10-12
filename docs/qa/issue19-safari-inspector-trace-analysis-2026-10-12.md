# Issue #19 — Actual Safari Web Inspector scrolling trace analysis

**Date:** 2026-10-12
**Evidence:** The user's latest exported `localhost-recording.json` from Safari Web Inspector, located in their Mac Downloads directory.
**Raw file:** 99,192,197 bytes, SHA-256 `836fc013d1e6693b78217c83cfb6c074c5c3ff2d938a4df72d75dc7f12fa3165`.
**Recorded duration:** **14.073 seconds**, Safari timeline export format version 1.
**Privacy:** The original ~99 MB trace, its 93 screenshot payloads and any page/URL data are **not committed**. The versioned analysis script exports only bounded, nonidentifying numeric aggregates.

## Question

The user experiences extremely laggy native Tasknotes scrolling, subjectively like 10–20 FPS. Does the **actual Inspector capture during scrolling** point to heavy JavaScript, style/layout, paint or compositing? Does it identify a concrete code-level hypothesis worth testing?

## What the trace really contains

- **2,036 top-level records:** 1,317 JavaScript event dispatch, 599 layout/rendering events, 93 screenshots, and 27 CPU samples.
- Recording has Rendering Frames listed among enabled instruments and selected in the Inspector view, **but no `timeline-record-type-rendering-frame` records appear in the exported `records` array**. Therefore a true rendered-frame duration histogram cannot be reconstructed from this export.
- **396 wheel events from 2.917 to 7.450 seconds** into the trace: **4.533 seconds of identifiable scrolling activity**.
- Other scroll-window events include **600 CSS transition events** (start/run/end/cancel). Total transition events across the recording: **660**.
- During scrolling, Inspector emitted **44 compositing records and 44 screenshot samples**. There are **33 unique exported screenshot payloads** in those 44 captures, with **11 consecutive duplicate samples**. Screenshots occurred at approximately **9.69 samples/second**. **This is the Inspector screenshot sampling rate, not the physical FPS or GPU presentation rate.**

## Work recorded during the actual scroll interval

| Web Inspector activity | Count | Cumulative duration | Median | p95 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Wheel-event JavaScript dispatch | 396 | **3.4 ms** | ~0.01 ms | ~0.03 ms | ~0.06 ms |
| CSS transition-event dispatch | 600 | **1.7 ms** | ~0 ms | ~0.01 ms | ~0.04 ms |
| Style recalculation | 39 | **2.5 ms** | ~0.06 ms | ~0.11 ms | ~0.11 ms |
| Layout | 37 | **6.0 ms** | ~0.16 ms | ~0.24 ms | ~0.25 ms |
| Paint | 282 | **64.5 ms** | ~0.08 ms | ~0.87 ms | ~2.40 ms |
| **Compositing** | **44** | **514.5 ms** | **10.57 ms** | **28.97 ms** | **35.20 ms** |

**13 of the 44 scroll-window compositing operations exceeded 16.7 ms**, the time budget of one 60-Hz display interval. Inspector's measurements attribute considerably more measured work to compositing than JavaScript, style calculation or layout. The 13 slow compositing records are **not** equivalent to 13 actually dropped screen frames.

The Inspector shows CPU usage samples during scrolling (9 sampled points; reported usage p95 ~35.4), but this coarse CPU sampling cannot assign GPU/compositor ownership or prove a specific platform bug.

## Concrete correlation with card hover transitions

Within the scrolling interval:

| Compositing records | Records | Mean duration | Exceeded 16.7 ms |
| --- | ---: | ---: | ---: |
| **Within 100 ms of a CSS transition event** | 25 | **17.00 ms** | **11** |
| **Not within 100 ms of a CSS transition event** | 19 | **4.71 ms** | **2** |

That is a **~3.6× association in mean compositing duration**. Event timing alone does **not** establish causation; transition-rich periods may also correlate with pointer location, scroll velocity and other state.

**Grounded code lead:** In `src/App.css`, `.task-card` animates `border-color` and `box-shadow` over **150 ms**, and `.task-card:hover` changes both. During scrolling, task cards move under the stationary pointer; hover changes could repeatedly trigger costly transitions and compositing. The capture includes 46 `pointerover`, 44 `pointerout`, and many transition events.

**Important counterevidence:** Earlier **test-only** WebDriver/WKWebView experiments showed sparse rAF callbacks even with all Tasknotes DOM/CSS removed. Disabling shadows/gradients without removing *all transitions* did not improve that synthetic callback metric. Therefore **do not claim the hover transition is the single root cause** of the user's physical scrolling lag merely from this recording. The Inspector indicates a plausible **additional app-specific rendering cost** that should be tested independently.

## Correct interpretation

- **Supported:** No evidence of a long JavaScript event handler or expensive layout causing this 4.53-second scroll capture. The largest measured per-operation durations are in **compositing**, with spikes temporally associated with CSS transitions.
- **Not proven:** Physical display FPS, root cause in the macOS compositor, specific selectors attached to the composite events (they carry no selector), whether removing the task-card transition fixes *user-perceived* scrolling, or the exact effect of Inspector screenshot collection overhead.
- **Measurement caveat:** The trace included screenshots (93 high-volume image payloads) and Inspector profiling itself; recording can affect performance. The screenshot count/cadence must not be presented as actual delivered frames.

## Most efficient next prototype / diagnostic gate

**Targeted, reversible, synthetic-QA-only CSS A/B—not an immediate production rewrite.**

1. **Baseline:** preserve the existing trace and a synthetic Tasknotes QA build with real Task cards and the same scroll workload.
2. **Only change one variable:** add a QA-only style to disable **`.task-card` `border-color`/`box-shadow` transitions** during scrolling while leaving other product styling, TaskStore, and WebView configuration unchanged. Do not disable all animations globally or ship a new `requestAnimationFrame` shim.
3. Compare **A–B–B–A** focused scrolling with an identical number of synthetic cards, same viewport, matched system load and fresh Safari Inspector timing traces. Record whether the **transition event count** collapses and whether **compositing p95 and >16.7-ms event count** improve materially. Use an Inspector run without screenshot capture if possible to reduce measurement overhead, then confirm with the existing screenshot mode for comparability.
4. If a substantial improvement is measured, implement the **smallest** task-card CSS change with tests, then compare visible native scrolling and guard reduced-motion/accessibility behavior. If not, reject the hover hypothesis and investigate WebKit/platform frame scheduling with separate compositor evidence.

This is a properly bounded `/prototype` candidate if the interaction/design trade-off matters: preserving a useful hover affordance without expensive animation. Otherwise a small `/diagnosing-bugs` A/B is sufficient. Both require test evidence before changing production defaults.

## Reproduce numerical analysis

From the Tasknotes repository on the user's Mac:

```bash
python3 -m unittest discover -s scripts/qa -p 'test_analyze_safari_inspector_trace.py' -v
python3 scripts/qa/analyze_safari_inspector_trace.py \
  "$HOME/Downloads/localhost-recording.json" \
  --output src-tauri/target/.issue19-safari-inspector-numeric-summary.json
```

The script reads only the selected export, checks for actual `wheel` events, and outputs only counts, durations, opaque screenshot-hash equality statistics and the input file checksum. It does **not** export screenshots, JavaScript sources, DOM content, remote URLs or the user's download path. The numeric JSON is kept under ignored `src-tauri/target` for QA reproducibility.

**Verdict:** **First actual Safari Inspector trace analyzed; app-specific compositing/hover transition hypothesis identified; Issue #19 remains OPEN / High.** No Tasknotes production code or user database has been modified, and no PR should be merged solely on this evidence.
