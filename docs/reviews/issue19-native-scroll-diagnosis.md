# Tasknotes Issue #19 — Two-axis diagnostic review

**Date:** 2026-10-12
**Branch:** `fix/issue-19-native-lag-diagnosis`, stacked above `test/native-wdio-e2e`.
**Method:** Manual Standards and Spec reviews, separate passes. This is **not** an independent reviewer approval and **not** a product-code fix.

## Standards / engineering safety

- The issue is now precisely **user-observed slow scrolling**, subjectively described as 10–20 FPS. The native test uses synthetic data, not personal Tasks.
- The native QA benchmark uses opt-in `native-e2e` feature and a visible but isolated WKWebView; global Tauri window show/focus permissions live exclusively in the QA-only capability. The release compile error prevents shipping the driver in a release build.
- `qa/native/perf-scroll.spec.ts` drives scoped W3C wheel actions; Tauri's embedded driver implements these through the WKWebView's `WheelEvent` and `window.scrollBy`. No global mouse/keyboard injection is used.
- Baseline and visual-effects-ablated scenarios use the same test build and driver; the blank-page control removes React DOM and styles *inside that synthetic test process only*. No product frontend module, React component, CSS, SQLite migration or app API is altered.
- The comparator tests the bare executable and packaged `.app` executable. An official Tauri `backgroundThrottling=disabled` QA-only variant was also tested but showed no benefit; the override was reverted and is not in standard app configuration.
- Deliberately failing the 50-ms diagnostic threshold is **expected** while the issue is unresolved. The existing functional native E2E suite and 19 React/20 Rust unit+integration tests remained green.
- A private temporary SQLite database is created and deleted by the runner; no actual user data, QA database, secrets or debug app bundles are committed. After QA, the normal app was rebuilt (31 MiB) with no WebDriver listener.
- External upstream issue [wry#1848](https://github.com/tauri-apps/wry/issues/1848) describes compositor stalls on recent macOS versions, but its mechanism and symptoms are *not confirmed* to match this Tasknotes problem. Do not ship a private-WebKit-API workaround from an unverified upstream hint.

## Spec / symptom verification

- **PASS — reproducible symptom at the test seam:** 160 W3C wheel actions and 33–36 measured rAF intervals per scenario. At default Tauri policy the p95 callback intervals are 117 ms (1 Task), 121 ms (30 Tasks), 120 ms (30 Tasks without visual effects), and 119 ms (blank scrolling HTML).
- **PASS — falsifiable threshold:** the 30-Task case reliably violates an explicit diagnostic `rAF p95 <50 ms` gate. A separate synthetic 200-ms main-thread stall also correctly triggered a red result, proving the timing loop can detect a stall.
- **PASS — targeted ablations:** visual effects off and full body/CSS removal did not improve frame scheduling. Launching within the `.app` bundle and QA-only disabling of background throttling also failed to improve materially.
- **PASS — no regression of Task CRUD/progress:** two-process native WKWebView E2E (including exact Progress Note and SQLite persistence) still passes; existing React and Rust tests are green.
- **LIMIT — not actual physical FPS:** the metric is WKWebView `requestAnimationFrame` callback scheduling, not compositor frame presentation or physical trackpad inertia. The user-described 10–20 FPS remains a subjective estimate.
- **LIMIT — root cause not established:** UI simplification not helping *this QA measurement* does not prove a macOS/Wry platform defect. Normal uninstrumented release-app scrolling and compositor presentation are still unmeasured under matched foreground and system load.
- **LIMIT — no production fix:** this PR is diagnostic infrastructure only. Issue #19 remains high severity and blocks final Ticket 17 performance verification and Ticket 18 release readiness.

## Merge and follow-up

**One-way risk:** No product storage schema changes in this diagnosis branch. The diagnostic controls must never be merged into an app release without QA feature gating and standard-build validation.

**Scope:** Tasknotes local macOS toolchain and reproducible test evidence; existing feature PRs #1–#4 remain Draft.

**Verdict:** Ready for a *stacked Draft diagnosis PR* that records the red repro, no more. Continue the Matt Pocock `/diagnosing-bugs` loop with authorized normal-build frame presentation measurement before modifying product UI. Independent code review remains a separate outstanding gate.
