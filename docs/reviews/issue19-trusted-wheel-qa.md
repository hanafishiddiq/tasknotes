# Issue #19 — Trusted-wheel QA two-axis review (manual)

**Date:** 2026-10-12. **Fixed point:** parent `prototype/issue19-hover-transition-ab` at `0c1ef9a`. **Branch:** `test/issue19-trusted-wheel-ab`. This review is performed by the implementing assistant against source and tests and **is NOT an independent reviewer approval**. Keep Draft.

## Standards axis (engineering and safety)

- **PASS:** The new physical wheel mode lives exclusively in the copied, opt-in QA script under `qa/native`; the normal `src/App.css`, React user UI and Rust TaskStore are not changed.
- **PASS:** The QA retains its separate bundle identifier `store.adamaja.tasknotes.issue19hoverab`, explicit synthetic fixture title prefix, private temporary QA SQLite and no WebDriver listener. No user-data read or global macOS input injection added.
- **PASS:** Real `WheelEvent.isTrusted===true` is required for every counted physical wheel event; physical phases return from the tick callback **before** synthetic WheelEvent/scrollBy/force-hover branch. Failure of focus, too few events, insufficient scroll travel, missing expected CSS transitions or imbalanced workload yields INCONCLUSIVE.
- **PASS:** Existing scripted mode remains available; 7 Node tests, 6 Python comparator tests and 19 React tests passed, TypeScript typecheck and dedicated native unsigned debug QA bundle built.
- **NOTE:** The QA script's multiple experiment modes are deliberately narrow test infrastructure, not architecture for the shipping application. The per-phase DOM status update itself is minor unmeasured QA work; one real capture is needed to assess its effect.
- **LIMIT:** Actual real-wheel interaction, Inspector marker quality and physical screen presentation are not yet verified, and QA CLI cannot autonomously operate the native Safari Inspector.

## Spec axis (real bug reproduction)

- **PASS:** The updated test targets the gap in the prior synthetic A/B: real trusted trackpad scrolling instead of dispatched untrusted `WheelEvent` with explicit `scrollBy`. Fixed same-process A–B–B–A design, unchanged Task data and CSS variants, prephase scroll reset and phase labels are present.
- **PASS:** Four 6-second physical phases store `inputMethod`, event counts, trusted event counts, scroll travel, rAF samples, CSS transitions, phase markers and validity in QA-only SQLite. Ratios up to 2.5x are predeclared as the minimal comparability gate; no thresholds were selected from new physical results.
- **PASS:** Inspector comparator explicitly rejects candidate qualification if baseline has **zero** composites over 16.7ms; only eligible baseline tests can apply both predeclared 30% criteria.
- **PENDING:** A focus-valid real-user scrolling A–B–B–A, with natural pointer/card hover transitions and a correctly segmented Safari Inspector recording. No claim of GPU-present FPS or user-visible lag improvement can be made without it.
- **BLOCKED:** Issue #19 product fix, regression test at a physical frame-presentation seam, independent review and merge remain open.

**Follow-up review — 2026-10-13 (not independent):** A new opt-in `--require-physical-qa` gate binds the Inspector record to the latest read-only, fixed-prefix QA SQLite row, validates phase identity, timing, trusted browser wheel counts, CSS A/B transition state, focus and comparable workload, and requires both A repetitions to be slow for any fix candidacy. Fourteen Python comparator tests pass, including isolated SQLite acceptance/refusal and the historical scripted-trace rejection. A `candidate` numeric result without the physical QA flag is explicitly suppressed. Source-based qualification: `isTrusted` does not guarantee actual trackpad hardware; that remains a supervised human action. The normal production app/database are unchanged. No independent human/subagent review or new physical capture has occurred.

**Verdict:** QA feature is test/build-ready, not validated with trusted hardware input. Retain Draft PR, no production CSS change, no Issue #19 closure. Next is one human Safari Inspector capture according to `docs/qa/issue19-trusted-wheel-qa-runbook.md`.
