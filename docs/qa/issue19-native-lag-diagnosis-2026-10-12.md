# Issue 19 — Native Tasknotes lag: first diagnosis checkpoint

**Date:** 2026-10-12
**Branch:** `fix/issue-19-native-lag-diagnosis` (stacked above the draft native E2E harness)
**Method:** Matt Pocock `/diagnosing-bugs`: targeted red-capable measurement, repeated baseline, minimal reduction, falsifiable differential probes
**Status:** **Unresolved. No production Tasknotes performance fix has been made.**

## The symptom being investigated

The user reports severe lag **specifically when scrolling** Tasknotes on macOS, subjectively comparable to a demanding 3D game at **10–20 FPS**, even with one synthetic Task. This is a perceived rather than measured FPS value. The scroll path must now be the primary reproduction; prior Task expand/collapse instrumentation alone is insufficient. This is the *user-reported bug*, distinct from the experimental native WKWebView frame-scheduling anomaly below.

## Red-capable diagnostic loop

On a macOS development machine:

```bash
pnpm qa:perf:build
pnpm qa:perf
# Control only (adds 200ms artificial stall in synthetic WKWebView):
QA_INJECT_JANK_MS=200 pnpm qa:perf
# After profiling, ALWAYS restore normal Tasknotes.app:
pnpm tauri build --debug --bundles app --no-sign --ci
```

The probe uses the opt-in, debug-only embedded WDIO driver with a *new synthetic SQLite database in TMPDIR for every run*. The visible QA window may be shown/focused only through Tauri's narrow test-only permission; the ordinary app and any personal SQLite DB remain out of scope.

**Threshold:** next `requestAnimationFrame` callback after a task toggle p95 below 50 ms is an **experimental diagnostic threshold**, not a product acceptance SLO. The benchmark intentionally fails when it detects a substantially delayed callback. The synthetic 200-ms stall yielded approximately 202 ms and caused the test to fail, proving it is red-capable.

## Observed baselines — original test-only WKWebView

| Probe | Observed p95 | Interpretation |
| --- | ---: | --- |
| Task expand: DOM mutation | 5–15 ms | React applied the visible DOM change rapidly |
| Task collapse: DOM mutation | 2–3 ms | Same |
| Next rAF callback after expansion | ~93–99 ms | Delayed scheduling in these QA conditions |
| Next rAF callback after collapse | ~78–110 ms | Delayed scheduling, variable |
| 12 direct toggles inside one WKWebView script | ~114–122 ms | Repeatable red against 50-ms diagnostic threshold |
| Idle rAF callback interval with Tasknotes rendered | ~115–121 ms | The slow callback was not specific to toggling |
| Empty body, remaining styles | ~117–124 ms | Deleting the Tasknotes UI did not resolve the slow callback |
| Empty body, CSS links and styles removed | ~120–121 ms | Removing Tasknotes styles did not resolve it |
| JavaScript `setTimeout(0)` | ~8 ms | General JavaScript timer scheduling was much faster |
| UI save wall time measured by native WDIO | approximately 26–115 ms across runs | Different metric; includes driver/IPC overhead, **not** display-frame latency |

**Critical limitation:** rAF is the browser's *frame-callback scheduling*, **not proof that a physical GPU frame took this long to paint or became visible that late**. The numbers were captured while an embedded WebDriver app had a `document.visibilityState` of `visible`, but Tauri native window state at the end showed `is_visible=true, is_focused=false`, and macOS reported a different app as frontmost (`com.openai.codex`). Browser window focus changed during automation. This supports a foreground/background scheduling or native QA harness effect, and does **not** establish a defect in Tasknotes' React components or SQLite.

Initial system load during Rust compile was high (around 20–29). Repeated samples with load around 4–8 remained delayed in the same QA conditions. The Mac subsequently restarted/reconnected; immediate post-startup load was high and should not be used for clean performance comparison. Host was on AC power when queried.

## Minimization and hypotheses

The slow callback persists with no Task DOM and no CSS, so Task row layout, progress history and SQL queries are *not supported as the primary cause of this particular rAF anomaly*.

1. **WKWebView background-frame throttling or QA focus handling.** Prediction: running the same rAF test with Tasknotes confirmed as the foreground native app changes the interval materially.
2. **macOS display/compositor or background system activity.** Prediction: an unrelated minimal native WebKit page, under matched focus and load, exhibits similar scheduling.
3. **Build configuration/debug-only WebDriver instrumentation.** Prediction: a comparable focused normal/release build without WDIO behaves differently.
4. **Tasknotes application DOM/CSS/history work.** Prediction would be a large improvement on removing the DOM or CSS; **not observed** in this minimized probe.

An authorized Mac app activation attempt through the UI controller timed out; afterward the macOS frontmost app was `SecurityAgent`, indicating an operating-system interaction/permission boundary. Do **not** bypass or emulate UI permission confirmation through shell. The required focused-app comparison therefore remains incomplete.

## Current conclusion

- **Automated functional native acceptance (PR #4):** passed twice.
- **Performance/visible lag (Issue #19):** **not fixed or closed**.
- A tight red-capable frame-callback test exists, but its reproduction is **not yet proven to be the exact user-observed lag**, because foreground focus and build configuration differ.
- No speculative changes to Tasknotes product UI or SQLite are justified yet. Keep all feature PRs Draft.
- The user has now clarified that **scrolling**, not Task creation or editing, is the severe symptom. A targeted WKWebView wheel-scroll test reproduces slow frame callbacks even with Tasknotes DOM/CSS removed. Comparing ordinary non-WebDriver app frame presentation and approved physical trackpad behavior is the remaining differentiator; do not request full manual CRUD QA again.

## Next eligible actions

1. With foreground permission, confirm Tasknotes is truly focused (`is_focused=true`, and macOS frontmost process is Tasknotes) before measuring rAF and frame pacing under low system load.
2. Compare the same scenario with a minimal native WKWebView / normal debug versus release build; record exact host/load and frame timings.
3. If the user-reported lag is reproduced, capture a failing regression at its true seam, implement a *measured* fix, and rerun both the minimized and full native E2E suites.
4. Obtain separate independent Standards/Spec review before merging stacked PRs. This diagnostic report does not substitute for a reviewer.

## User-confirmed scroll focus and measured reproduction (2026-10-12)

The user clarified that **scrolling** is the primary slow operation; it subjectively feels like 10–20 FPS, even with one Task. This does not establish a measured display FPS.

The dedicated native WKWebView diagnostic `qa/native/perf-scroll.spec.ts` drives W3C wheel actions *only inside the isolated Tasknotes WKWebView*. On this embedded driver, wheel actions are implemented by scoped JavaScript `WheelEvent` / `window.scrollBy`, so results measure callback scheduling **but not physical trackpad inertia or GPU frame presentation**. The test runs on newly created synthetic SQLite databases, never personal Tasks or Notes.

**Final high-sample baseline, default Tauri policy; 160 wheel actions per case (p95):**

| Scenario | Sampled frame intervals | rAF interval p95 | Travel |
| --- | ---: | ---: | ---: |
| One Task | 35 | **117 ms** | 111 px |
| 30 Tasks | 35 | **121 ms** | 1,253 px |
| 30 Tasks with CSS shadow/gradient/filter disabled (QA only) | 33 | **120 ms** | 1,280 px |
| Blank scrolling HTML page, all Tasknotes DOM and styles removed | 36 | **119 ms** | 1,280 px |

The pre-specified **diagnostic**, not release, threshold of p95 below 50 ms failed in the 30-Task case. Earlier shorter 25-wheel-action runs with about 5–8 frame samples also failed (30 Tasks roughly 111–122 ms; blank page roughly 114–120 ms), but the longer run provides more reliable sampling.

**A/B comparisons that did not help:**

- Running the same QA executable via its actual bundled macOS `Tasknotes.app/Contents/MacOS/tasknotes` path instead of the bare executable did not resolve callback delays (30 Tasks ~118 ms, blank page ~114 ms, short-sample run).
- Official Tauri macOS `backgroundThrottling: "disabled"` was tested **only in the QA overlay**; 30 Tasks still ~111 ms, blank page ~120 ms. It was **reverted** and was **not deployed** to normal Tasknotes. Documentation: https://v2.tauri.app/reference/config/#backgroundthrottlingpolicy.
- Removing all Tasknotes DOM and CSS, or limiting the list to one Task, did not meaningfully improve frame callbacks. Do **not** claim a React, SQLite, gradient, or shadow fix without new contrary evidence.

**Relevant upstream signal, not proof of the same root cause:** the Wry project has an active macOS 26+ compositor issue where the DOM updates despite stale/partial frames: https://github.com/tauri-apps/wry/issues/1848. Tasknotes uses Wry 0.57.0/Tauri 2.12.3 on macOS 27.0.1; our symptom is slow scroll callback cadence rather than proven stale/partial composited frames. No assumption that the two bugs are identical.

**Conclusion:** a red-capable *scroll-specific WKWebView diagnostic* is established; a native app product fix is **not** established. The user's perceived 10–20 FPS is consistent in broad magnitude with long frame-callback intervals, but the measured rAF p95 values are **not directly measurable display FPS**. Before implementing a workaround, compare a truly foreground **normal, non-WebDriver** app with matched release/debug builds and/or Core Animation presentation instrumentation. **Tool availability (2026-10-12):** The authorized Mac currently has only Xcode Command Line Tools (`/Library/Developer/CommandLineTools`); `xcrun --find xctrace` returned `unable to find utility xctrace`. Native Core Animation/Instruments traces are therefore unavailable in this environment without a separate developer-tool installation. Do not claim that a compositor trace was captured. An approved method of native foreground focus and visual frame collection is needed. Avoid dangerous macOS accessibility/permission bypasses or shipping the test WebDriver into normal app builds.

The performance bug remains **High, unresolved**, blocking Ticket 17's final performance verification and Ticket 18's release readiness. The full functional native E2E suite (two real app processes plus SQLite) still passes after the scroll diagnosis.

## Safety and cleanup

The QA-only Rust capability contains `core:window:allow-show` and `core:window:allow-set-focus`; the normal Tauri app does not load these permissions or the embedded WebDriver. One isolated diagnostic process was stopped, and its TMPDIR SQLite directory was removed. The ordinary debug `Tasknotes.app` must be rebuilt after the profiling build and verified to be driver-free. Do not ship the QA binary.
