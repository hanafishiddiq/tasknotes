# Issue #19 — Card-hover animation A/B prototype

**Date:** 2026-10-12
**Branch:** prototype/issue19-hover-transition-ab, stacked on the Safari Inspector analysis.
**State:** Native QA app and reproducible harness ready. **Compositor A/B trace PENDING; no Tasknotes fix.**

## Question and evidence

The previous actual Safari Inspector capture contained 396 wheel events and 600 CSS-transition events during 4.533 seconds. Of 44 compositor records, 13 took longer than 16.7 ms; compositor p95 was 28.97 ms. Events within 100 ms of CSS transitions averaged 17.00 ms versus 4.71 ms otherwise. This is an association, not proof that card-hover animations cause all perceived scrolling lag.

Tasknotes currently transitions card border-color and box-shadow during hover for 150 ms. The prototype tests the effect of disabling only these card transitions while preserving the hover border/shadow itself.

## Built prototype

**Tasknotes Issue19 Hover A B QA** is a standalone 31 MiB debug Tauri/Wry app using the actual compiled Tasknotes UI and Rust/SQLite core. It has a dedicated bundle identifier (store.adamaja.tasknotes.issue19hoverab), private temporary SQLite (31 synthetic Tasks plus one numeric QA metrics row), and an overlay near the bottom-left.

The overlay contains **Start one A–B–B–A trace**. One click runs four identical timed-scroll phases, approximately 20–30 seconds total:

- A1/A2: original card hover animations unchanged.
- B1/B2: only card transition animations disabled; hover appearance unchanged.
- All phases: same synthetic wheel-equivalent scroll, same fixed-point forced hover class (not a real trackpad), same native window/React assets and focus checks.
- For each phase, the QA records scroll distance, synthetic wheel events, card-transition events, rAF p50/p95, and beginning/end timestamps through existing Rust commands into synthetic QA SQLite.
- WebKit console.timeStamp markers, named custom events, console.profile and console.profileEnd allow one Safari Inspector recording for all phases. The marker format is ISSUE19_HOVER_A1_BEGIN and so on.

## Actual verification so far

- **PASS:** Native QA app built with separate identifier, no WebDriver listener, no production Tasknotes CSS change. Screenshot confirms all 32 synthetic Tasks and a ready Start button.
- **PASS:** Three Node tests cover A–B–B–A ordering, CSS mode switching and percentile math.
- **PASS:** Four Python tests cover Inspector phase parsing, no-improvement rejection and refusal of absent/duplicate markers. The original unmarked Safari export was correctly rejected as inconclusive, not treated as an A/B result.
- **PASS:** The prototype safely rejected the initial automatic background execution as **INCONCLUSIVE** because the QA window was not focused. No data from that attempt is counted as a performance measurement.
- **PENDING:** A four-phase foreground trial and a new Safari Inspector recording. Therefore no compositor delta or production improvement can be reported yet.

## One Safari recording, not four

1. Leave the open **Tasknotes Issue19 Hover A B QA** app running (do not choose the older Tasknotes Issue19 Inspector QA app).
2. In Safari, open Develop → this Mac → Tasknotes Issue19 Hover A B QA → localhost, then open Timelines. The WebKit console.profile call can start the recording; if necessary, press Safari's Record button just before the next step.
3. Return to the QA app and click **Start one A–B–B–A trace** exactly once. **No manual scrolling** is required. When the status says **Completed A–B–B–A**, stop/export ONE Safari Timelines recording to Downloads.
4. Tell the assistant the export is saved. It will read the latest file from Downloads and run the numeric-only tools. It will not upload the trace or its screenshots to GitHub.

Read the QA's own SQLite results using the command: python3 scripts/qa/report-issue19-hover-ab.py

Analyze the new Safari export with: python3 scripts/qa/compare_issue19_hover_inspector.py PATH_TO_NEW_JSON

Only if both mean compositor p95 and the number of composites over 16.7 ms per scrolling second improve by at least 30% across valid A–B–B–A will B qualify as a candidate. Even then, separate native user-visible validation is required. Physical screen FPS and real trackpad behavior are **not** measured by the current synthetic scroll.

## Product safety and next gate

This prototype does not change src/App.css, the Tasknotes production app, the user's Task DB, or ChatGrid. The script/QA frontend remain test-only; raw Safari recordings remain on the Mac. After the experiment, restore the ordinary raw debug app through the standard normal Tauri build if that executable path is used.

**Decision:** PENDING, not fixed; Issue #19 stays OPEN/High. Do not merge or claim a performance cure without the corresponding Inspector recording and independent review.
