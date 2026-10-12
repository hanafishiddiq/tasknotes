# Tasknotes Issue #19 — Main WKWebView vs child WKWebView

**Date:** 2026-10-12
**Branch:** `test/issue19-main-vs-child-webview`, stacked above [PR #6](https://github.com/hanafishiddiq/tasknotes/pull/6).
**Status:** **Diagnostic completed; no meaningful child-WebView benefit. Issue #19 remains open.**

## Falsifiable question

The user recalls that ChatGrid's native macOS browser became smooth after a fix by Astra. Source archaeology showed ChatGrid's smooth implementation uses **child WKWebViews**, while Tasknotes uses a top-level/main WKWebView. Does **moving the same Tasknotes frontend to a child WKWebView** materially reduce frame-callback latency? Does ChatGrid's extra disabled-background-throttling policy help?

**Prespecified candidate threshold:** a child mode must improve the scrolling `requestAnimationFrame` p95 by **at least 30 ms AND 30%** versus the top-level control under the **same binary and focus-valid measurements**. A tiny delta or absolute low-frame-rate improvement is not a production fix.

## Design and safety

- Used the **actual Tasknotes UI**, 30 synthetic Tasks plus 1 metrics Task, identical SQL store and frontend resources across all variants. `index.html` SHA-256: `a9c3406d343822282fc022b91f23b09188643184f0baf92c3f4c091a537b4a71`; injected synthetic-only QA JS SHA-256: `8213db7ee5a8e41e6905f8015427165a5a2f2f828d48e98dd1185d59b6bb6602`.
- A **single test-only Tauri binary per profile** has an explicit `issue19-placement-qa` Cargo feature, enabling `tauri/unstable` for the same Wry multi-WebView API used by ChatGrid. Changing the environment variable `TASKNOTES_QA_PLACEMENT` selects a mode **without rebuilding**.
- `top`: `WebviewWindowBuilder::new(app,"main",WebviewUrl::App("index.html"))`. `child`: a native `WindowBuilder::new(app,"main")`, with exactly one `window.add_child(WebviewBuilder::new("qa-content", same local URL))` and `auto_resize()`. `child-no-throttle`: identical child with `BackgroundThrottlingPolicy::Disabled` as in ChatGrid.
- The host Tauri window is `main` in every mode, same client viewport **1120×708**, scroll height **3039**, and synthetic scrolling movement around **1,680–1,800 px**. Page UI confirms `webviewLabel=main` for top and `qa-content` for child.
- Test-only app identifier is **`store.adamaja.tasknotes.issue19placement`**, not `store.adamaja.tasknotes`. A Rust assertion checks the identifier **before opening the TaskStore**, and again before building the window. The runner refuses non-QA data and symlinked database paths.
- **No WebDriver server**, no Mac-wide input injection, no destructive user-data operation, no ChatGrid sessions touched. Both builds omit `native-e2e` and use regular Tauri IPC.
- The same in-WebView QA script drives timer-paced `window.scrollBy`, records scroll distance and rAF callback intervals, rejects focus-loss, checks visibility, and persists numeric data to the synthetic metrics Task with revision-checked Rust commands.
- Initial debug pilots experienced focus loss (two **invalid** samples), excluded. The QA-only setup then explicitly focused the WebView first responder; **all six debug samples** and **all six release samples** below are valid and were logged with `focusLost=false`, `focusAtStart=true`, and native `isFocused=true` at the end.
- Source scripts: `qa/native/main-child-scroll-probe.js`, `src-tauri/src/issue19_placement_qa.rs`, `scripts/qa/prepare-issue19-placement.mjs`, `scripts/qa/run-issue19-placement.py`, and `scripts/qa/report-issue19-placement.py`.

## Observed native results — scrolling rAF callback p95, milliseconds

| Build | Top-level main | Child, default policy | Child, ChatGrid-like background throttling disabled |
| --- | --- | --- | --- |
| **Debug** | **220, 215** (mean **217.5**) | **215, 224** (mean **219.5**) | **220, 224** (mean **222.0**) |
| **Release (optimized)** | **217, 220** (mean **218.5**) | **216, 219** (mean **217.5**) | **222, 219** (mean **220.5**) |

Main vs default child used **A–B–B–A order** in each build to partially control drift. The optional disabled-throttling child control was measured twice after the A–B–B–A run.

**Effect relative to main:** Debug default child was **2 ms slower** (~0.9%); optimized Release default child was **1 ms faster** (~0.5%). Child plus disabled throttling was ~4.5 ms worse in Debug and ~2 ms worse in Release. **None meets the 30-ms / 30% improvement condition.**

Each included trial reported the same viewport, page scroll range, 150 timer ticks (~3.15 s), and at least **12 rAF interval samples**. Source bundle/JS hash did not change between mode changes; the platform wrapper and background policy were the controlled variables.

## Important interpretation boundaries

1. **No architecture migration:** A child WebView does **not** improve Tasknotes scrolling under this matched native QA comparison. Copying ChatGrid's child-WebView topology or disabled-background-throttling policy into Tasknotes would create complexity without measured benefit.
2. **Absolute latency differs across harnesses:** this QA build (including the `tauri/unstable` multi-WebView feature and a programmatically constructed window in both modes) yielded ~215–224-ms rAF p95. Earlier declarative main-window QA on the same machine showed ~106–118 ms. Those harnesses differ in window creation, feature selection and initial focus handling; the difference **cannot be causally attributed to any one factor**. The *within-binary main-vs-child* comparison is the defensible result.
3. **Not physical FPS:** rAF callback intervals are browser scheduling measurements, not composited screen presentation or the user's physical trackpad inertia. The user's perceived ~10–20 FPS remains the reported symptom; do not convert p95 intervals directly to FPS.
4. **Small sample and host workloads:** two focus-valid samples per mode/profile; no statistical significance claims. Other WebKit/ChatGrid/Brave workload on the user's Mac is a possible confounder, not a proven cause. None was terminated.
5. **No product fix:** user-owned Tasknotes app, its normal bundle identifier, SQLite schema/data, styling, window topology, and ChatGrid are unmodified. The QA compiler feature is opt-in; a bundled QA binary must never be shipped as the normal app.

## Next highest-value diagnostic hypothesis

The sharp ~110-ms vs ~220-ms difference between **declaratively configured main WKWebView** (earlier release/debug A–B report) and the **programmatically built top-level WebviewWindowBuilder** in the new multi-WebView QA binary warrants an isolated, matched *declarative-vs-programmatic startup* comparison. That test must control the `tauri/unstable` compile feature, frontend, focus and native process launch; otherwise it only reproduces the same confounding.

If that yields no usable remedy, focus on WKWebView scheduling / macOS compositor instrumentation or reproducing a genuine foreground physical-trackpad frame-presentation trace with authorized tooling. `xctrace` is unavailable from the installed Xcode Command Line Tools. **Issue #19 remains OPEN / High** and blocks final Ticket 17/18 performance/release acceptance.

## Reproducing safely

```bash
node scripts/qa/prepare-issue19-placement.mjs
pnpm tauri build --debug --features issue19-placement-qa --config src-tauri/target/issue19-placement/issue19-placement.conf.json --bundles app --no-sign --ci
python3 scripts/qa/run-issue19-placement.py top --profile debug
python3 scripts/qa/run-issue19-placement.py child --profile debug
python3 scripts/qa/run-issue19-placement.py child --profile debug
python3 scripts/qa/run-issue19-placement.py top --profile debug
# An optimized QA build uses the SAME generated assets:
pnpm tauri build --features issue19-placement-qa --config src-tauri/target/issue19-placement/issue19-placement.conf.json --bundles app --no-sign --ci
python3 scripts/qa/run-issue19-placement.py top --profile release
python3 scripts/qa/run-issue19-placement.py child --profile release
python3 scripts/qa/run-issue19-placement.py child --profile release
python3 scripts/qa/run-issue19-placement.py top --profile release
python3 scripts/qa/run-issue19-placement.py child-no-throttle --profile release
python3 scripts/qa/run-issue19-placement.py child-no-throttle --profile release
python3 scripts/qa/report-issue19-placement.py
```

The page's safety limit stores at most 10 numeric QA runs; archive the synthetic results and recreate **only the verified QA fixture** if you need another batch. The normal Tasknotes debug bundle should be rebuilt after QA when its raw `target/debug/tasknotes` output has been repurposed. No physical screenshot/video or private data is needed.
