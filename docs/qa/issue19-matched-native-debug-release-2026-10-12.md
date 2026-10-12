# Tasknotes Issue #19 — Matched native Debug vs Release scrolling A/B

**Date:** 2026-10-12
**Branch:** `test/issue19-release-debug-comparison`, stacked on Issue #19's diagnostic branch.
**Status:** **Completed experiment, negative for meaningful release improvement.** The user-reported slow scrolling remains unresolved.

## Question and prespecified decision rule

Given ChatGrid's smooth-scrolling evidence from an optimized macOS release binary, does compiling native Tasknotes in its true Cargo `release` profile (opt-level 3, LTO enabled, strip, codegen-units 1) materially improve scrolling compared with `debug`?

**Predefined candidate rule:** release must reduce scrolling `requestAnimationFrame` p95 by at least **30 ms and 30%**. A smaller delta is not sufficient evidence of a practical user-visible scrolling fix.

## How the comparison was conducted

- **Actual React Tasknotes frontend**, built once by `pnpm build` then copied into the ignored `src-tauri/target/issue19-native-ab/frontend` directory. A QA-only script was added *to this copy*, with SHA-256 of HTML `913d2febbc823eb41b8a5c318fe486b09ce108ef548be6bff0f6784dfc7f774b` and QA script `7965d17980376f7ab3e85d73317a642bc85f2c9840b8ab0a5961f7cb838a1fdc`. The hashes were identical before and after the release build.
- Same Tauri source and versions, macOS **27.0.1 ARM64** and main/top-level native WKWebView in both builds.
- Same **isolated QA identifier** `store.adamaja.tasknotes.issue19benchmark`, never `store.adamaja.tasknotes`. Identical SQLite dataset of **30 synthetic Ready Tasks plus one synthetic metrics Task**, reused for all four measurements. All titles start with `__ISSUE19_QA_AB__`, with an explicit guard refusing any other data.
- Explicit test-only Tauri overlay turns on `withGlobalTauri` and narrowly permits window show/focus queries. **No embedded WebDriver feature, no localhost test server, no global OS mouse injection, no personal Tasks**. The standard Tauri config is not altered.
- The benchmark triggers `window.scrollBy` on a 16-ms JavaScript timer, measures `requestAnimationFrame` callback timing and scroll movement, and persists only numeric results through the existing Rust TaskStore `update_task` command on the synthetic metrics row.
- Native bundles compiled with `pnpm tauri build --debug ...` and `pnpm tauri build ...` (real optimized release, including existing LTO). Debug QA bundle about **31 MiB**; release QA bundle about **5.6 MiB**.
- **A-B-B-A order:** Debug → Release → Release → Debug, same assets and database. Test rejects visibility/focus loss, insufficient frames and insufficient scroll movement. All four included samples were `valid=true`, `visibility=visible`, `windowFocusedAtEnd=true`, `focusLost=false`.
- One initial Release run lost focus and reached its 90-s timeout; it produced **no metrics row and was excluded**. The immediate bounded retry was valid.

## Actual results (p95 of rAF callback interval, milliseconds)

| Run order | Native profile | Idle p95 | Scrolling p95 | Scroll frame samples | Scroll travel |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 | Debug | 114 | **117** | 35 | 1,776 px |
| B1 | Release | 112 | **106** | 40 | 1,800 px |
| B2 | Release | 118 | **113** | 33 | 1,800 px |
| A2 | Debug | 113 | **118** | 35 | 1,752 px |
| **Average of each pair** | **Debug** | 113.5 | **117.5** | — | — |
| **Average of each pair** | **Release** | 115.0 | **109.5** | — | — |

**Measured difference:** release lowered pair-averaged scrolling rAF p95 by **8.0 ms (6.8%)**, far below the predeclared 30-ms / 30% improvement condition. Scrolling still showed callback intervals around 106–118 ms in *both* build profiles. The native release optimization is therefore **not an adequate demonstrated fix** for the user's severe scrolling lag.

## Constraints and interpretation

- The rAF metric is **browser callback scheduling**, not physical display presentation FPS or Mac trackpad inertia. The user's subjective “10–20 FPS” remains a distinct symptom; don't re-label rAF timing as measured screen FPS.
- A-B-B-A controls for some drift, but this is a **small exploratory sample** (two valid runs per mode), not a statistically powered performance certification.
- The Mac had other active workloads. Read-only `ps/lsof` observations confirmed multiple **ChatGrid-owned WebKit WebContent processes** with significant CPU activity; ~4.86 GiB host swap was in use during the release-build window. These may affect UI scheduling, but the measurements do not establish them as the *cause* of Tasknotes lag. **Do not terminate or reload user ChatGrid sessions** without separate authorization.
- The optimized release sample used the same *test-only* frontend instrumentation and QA identifier as debug; this was not an uninstrumented physical trackpad recording of the installed Tasknotes app.
- Existing independent Swift/AppKit WKWebView control (without Tauri/React/WebDriver) separately reproduced focused rAF p95 ~120 ms while native NSScreen display-link remained ~16.8 ms. This is consistent with an engine scheduling issue, but still does not isolate physical compositor presentation.

## Diagnostic outcome and next falsifiable experiment

**Reject:** “Switch to release build alone to fix scrolling.” Both A/B profiles are slow under the native timing test.

**Highest-value next question:** ChatGrid uses `WebviewBuilder` **child WKWebViews** with `background_throttling(Disabled)`, whereas Tasknotes uses a **main/top-level WKWebView**. Test both placements with the same page, same build, identical native host load, and a verified foreground window. Do not rearchitect Tasknotes into child views until a measurable improvement is demonstrated; previous **main-window** throttling-disabled QA experiment did not help.

**Issue #19 remains OPEN / High.** PRs #1–#5 remain Draft. No Tasknotes production logic, styling, database schema, user data, ChatGrid application, or macOS global settings were changed.

## Reproduce safely

From the repository root on macOS:

```bash
node scripts/qa/prepare-issue19-native-ab.mjs
pnpm tauri build --debug --config src-tauri/target/issue19-native-ab/issue19-ab.conf.json --bundles app --no-sign --ci
python3 scripts/qa/run-issue19-profile.py debug
pnpm tauri build --config src-tauri/target/issue19-native-ab/issue19-ab.conf.json --bundles app --no-sign --ci
python3 scripts/qa/run-issue19-profile.py release
python3 scripts/qa/run-issue19-profile.py release
python3 scripts/qa/run-issue19-profile.py debug
python3 scripts/qa/report-issue19-ab.py
```

The runner will **refuse non-synthetic data**; review its isolated bundle identifier before launching. The default normal `Tasknotes.app` remains separate and should not have QA capabilities. Restore the standard `target/debug/tasknotes` executable with a normal `pnpm tauri build --debug --bundles app --no-sign --ci` afterward if developers use that raw binary path.
