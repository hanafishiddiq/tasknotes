# Issue #19 — REAL trackpad A–B–B–A capture runbook

**Date:** 2026-10-12. **Status:** QA build and unit tests PASS; physical wheel and new Inspector trace **NOT RUN**. Only synthetic Tasks in `Tasknotes Issue19 Hover A B QA` are used. No normal Tasknotes data or CSS is modified.

## What changed from the first experiment?

The prior button **Start one A–B–B–A trace** still runs the original synthetic-wheel mode and is not sufficient for the user-reported physical scrolling lag.

New button **Start REAL trackpad A–B–B–A** runs four short *real wheel input* phases. The QA does **not** call `scrollBy` or synthesize `WheelEvent` during these physical phases; only trusted `wheel` events count. It switches hover transition styles A1/B1/B2/A2 within one native window, resetting scroll before each measured interval. A keeps original animations; B disables only card transitions. No production CSS change.

## Exact capture steps

1. QA app is started on the Mac with `bash scripts/qa/run-issue19-hover-ab.sh --reuse-bundle`. Use **Tasknotes Issue19 Hover A B QA**, not the older inspector-only build.
2. In Safari choose **Develop → This Mac → Tasknotes Issue19 Hover A B QA → localhost**, select **Timelines**. Use **Edit** above the timeline list to uncheck **Screenshots** while keeping **Layout & Rendering**, **JavaScript & Events**, and **Media & Animations** enabled; this avoids large screenshot payloads and reduces measurement interference. See [WebKit Timelines reference](https://webkit.org/web-inspector/timelines-tab/). Leave the Inspector attached.
3. Return focus to Tasknotes, place the pointer over a **Task card in the center of the list** (not on the small QA control panel). Click **Start REAL trackpad A–B–B–A** once; move the pointer back onto Task cards immediately.
4. Watch the QA's four labels: **A1, B1, B2, A2**. Each has a 2s preparation interval, then displays **SCROLL NOW with trackpad for 6 seconds**. During each SCROLL NOW interval, physically scroll repeatedly **down and up**, smoothly/consistently, to keep the page moving; **do not switch applications**. Keep real wheel input roughly comparable across all four phases. Timed phases advance automatically; no further button press needed.
5. When the QA status says **Completed physical A–B–B–A**, stop Safari Timelines and export **one new recording JSON** into Downloads. Do **not** upload the raw Safari file, which includes screenshots and page data. If it says **INCONCLUSIVE**, read the phase result first and repeat only after diagnosing the rejection reason.

## Validation and result reading (assistant)

```bash
cd ~/Desktop/Rumah/Playground/tasknotes
python3 scripts/qa/report-issue19-hover-ab.py
python3 scripts/qa/compare_issue19_hover_inspector.py \
  "$HOME/Downloads/<exact-new-recording>.json" \
  --require-physical-qa \
  --output src-tauri/target/.issue19-trusted-wheel-inspector-numeric.json
```

Validate latest `inputMethod=physical`, four focus-valid phases, >=35 real trusted wheel events, >=600px travel and >=12 rAF samples per phase, A transition runs >0 and B zero, and workload ratio <=2.5x. A valid Inspector capture must contain eight named phase markers, >=20 wheel events and >=5 composites per phase.

**Evidence-binding gate (updated 2026-10-13):** The required `--require-physical-qa` flag reads **only** the synthetic QA SQLite metrics, rejects any latest run not marked `physical` and valid, and cross-checks all four Inspector phase durations, real-wheel event counts and CSS transitions. It also rejects a Safari export older than the latest QA run. For this check, `Event.isTrusted` means **user-agent generated**, not an independent proof of the hardware source; a person must actually perform the trackpad scrolling ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/Event/isTrusted)). An old synthetic recording is explicitly rejected, not used as a substitute.

**Performance decision gate:** A1 and A2 must **both** reproduce at least one expensive composite event >16.7ms. Only then can B become a candidate if both paired compositor p95 and expensive-composite events/sec decrease by >=30%. The Inspector is not a direct physical GPU frame-present counter; user-perceived native smoothness must still be verified separately. Do not merge/close #19 on metrics alone.

## QA guardrails and reproducibility

- Use the QA-only bundle identifier `store.adamaja.tasknotes.issue19hoverab`, the existing private temporary QA SQLite (synthetic 32 Tasks), and a copied frontend under ignored `src-tauri/target`.
- `node --test qa/native/issue19-hover-ab.test.mjs` contains explicit unit checks rejecting fake `WheelEvent` objects and focus-lost/noncomparable phases.
- No OS-wide input automation, unsafe Safari/browser foreground control, private WebKit APIs, production `src/App.css` edits, user Task DB access, or raw Inspector exports committed.
