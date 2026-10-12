# Issue #19 — Focus-valid hover A–B–B–A Inspector analysis

**Date:** 2026-10-12. **Scope:** QA-only, synthetic 31 Task cards + one metrics Task. **Verdict:** Valid as a *synthetic CSS/compositing A/B*, **not qualified as a fix** for the user's native trackpad scrolling lag. No change to production CSS.

## Provenance and gates

- Human-attached Safari Inspector export: local `~/Downloads/localhost-recording2.json`, 250,304,955 bytes, SHA-256 `6a68b54d43177dba0bba05311d41b288f336b031159a5c61bb9daf3babda8e46`. Raw export and screenshot payloads remain privately on the Mac, not committed.
- Parsed with `python3 scripts/qa/compare_issue19_hover_inspector.py ~/Downloads/localhost-recording2.json --output src-tauri/target/.issue19-hover-ab-compositor-comparison.json`. The output is **numeric-only** and ignored by Git.
- Original parser had a 250,000,000-byte budget; this export exceeds it by 304,955 bytes. A **TDD-tested 256 MiB** input-size ceiling enables analysis while retaining an explicit bound (test still rejects larger or empty input). `orjson` is present on the Mac.
- One valid synthetic SQLite QA run, after two earlier discarded focus-invalid attempts. Four phases **A1 → B1 → B2 → A2**: each has focus=true, 200 scripted wheel events and **4,810px** scrolling distance; original Tasknotes UI and Rust core. The Inspector trace contains eight unique serialized phase markers and at least 60 composite records per phase. These gates pass.
- QA's **card-targeted** transitionrun counts: A1 415, A2 413, B1 0, B2 0. The Inspector's event-level counts include other timeline events and need not match the QA's per-card counts.

## Measured Inspector results

| Phase | CSS mode | Composites | Inspector compositor p95 | Composites >16.7 ms | Inspector transitionrun events |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 | Animated | 64 | 3.564 ms | 0 | 1,245 |
| B1 | Disabled | 65 | 2.003 ms | 0 | 0 |
| B2 | Disabled | 60 | 2.095 ms | 0 | 0 |
| A2 | Animated | 62 | 2.485 ms | 0 | 1,239 |

Paired mean compositor p95: **A 3.024 ms → B 2.049 ms**, **32.3% reduction** under this *scripted* workload. Composites >16.7ms/scroll-second: **A 0 → B 0**, **no reduction**. The original two-part threshold (**>=30%** reduction of both compositor p95 **and** expensive compositor events/sec) is **not met**; the comparator correctly marks `candidate=false`.

Independent browser callback measurements from the same QA run: rAF p95 **A1 119 ms / A2 122 ms → B1 121 ms / B2 121 ms**. Mean **A 120.5 ms, B 121.0 ms**, no benefit. These are **not physical display FPS**.

## Why this does not solve the reported scrolling lag

The earlier *human wheel-scrolling* Inspector recording (`localhost-recording.json`, SHA-256 `836fc013d1e6693b78217c83cfb6c074c5c3ff2d938a4df72d75dc7f12fa3165`) showed **44 compositing records during 4.533s; p95 28.97 ms; 13 over 16.7ms**. The new synthetic A/B baseline has **zero** >16.7ms composites in both A repetitions. The new test therefore does **not** reproduce the slow compositing work that motivated the hypothesis, despite valid focus and abundant phase markers.

Crucially, `qa/native/issue19-hover-ab.mjs` dispatches `new WheelEvent("wheel")` (untrusted) and then scrolls using `window.scrollBy`, plus a forced hover-like class. It does **not** replay a real macOS trackpad's trusted input/default scrolling/gesture pathway. Primary source: [MDN WheelEvent constructor](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent/WheelEvent) (synthetic wheel events are untrusted and do not trigger default action). Inspector activity is **not** physical GPU presentation FPS: [WebKit Timelines](https://webkit.org/web-inspector/timelines-tab/).

A1 and A2 p95 values also differ noticeably (3.564 vs 2.485ms), so the paired average alone does not establish a stable causal effect under physical scrolling.

## Decision and next step (/diagnosing-bugs)

- **Reject B as a standalone Issue #19 fix under the predeclared gate.** Disabling card transitions reduces one synthetic compositing metric, but no *expensive* composites existed in the synthetic baseline and the rAF issue remained. Do **not** modify normal `src/App.css`, mark Issue #19 resolved or merge the diagnostic PR as a fix.
- **Next red-capable loop:** re-test with actual trusted physical trackpad wheel input on the QA app, A–B–B–A within the same session. Preserve identical UI/data/viewport; require baseline A to reproduce slow >16.7ms composites *before* claiming a candidate. Gate each phase on focus, enough **trusted** wheel events and scrolling distance. Add phase markers and numerical-only Inspector export. Human hardware input may be required; don't simulate system-wide events or bypass Mac controller policy.
- If a true wheel run also fails to recreate lag, separate compositor cost from browser rAF scheduling and seek a different user-visible frame-presentation measurement. Neither a changed CSS animation nor a rAF shortcut is a verified performance fix.
