# Issue 19 — Controlled hover-transition A/B prototype

**State:** Planned before execution; implementation stays isolated in an opt-in QA app. **Question:** Is the scroll-window spike in WebKit compositing partly caused by `.task-card` 150ms hover transitions (`border-color` and `box-shadow`), as suggested by the user-provided Safari trace?

## Source evidence and competing hypotheses

- Actual user's Safari Web Inspector export (99.2MB, kept outside Git) captured 396 wheel events over 4.533s, 600 CSS-transition events and 44 compositor operations. Compositor p95 28.97ms; 13/44 >16.7ms. Composites within 100ms of transitions average 17.00ms vs 4.71ms otherwise, **correlation, not proof**. Source: `docs/qa/issue19-safari-inspector-trace-analysis-2026-10-12.md`.
- H1 **animated card hover is expensive**: transition-disabling only `.task-card` should reduce that element's transition events and the compositing tail.
- H2 **static shadow is expensive**: transition event count drops but expensive compositor events remain.
- H3 **WKWebView scheduler dominates**: transition events drop while rAF timing remains slow, which does not disprove localized compositing savings.
- H4 **Inspector/host noise dominates**: no consistent improvement over A–B–B–A; repeat with matching screenshots/CPU settings and foreground state.

## Hold constant

1. One real native Tauri/Wry **debug** WKWebView, one QA-only bundle ID, same compiled React UI, 31 synthetic Tasks, viewport, position, app focus and scroll action cadence.
2. A1–B1–B2–A2 **within the same process/session**, total ~25s. Baseline A does not alter `.task-card` transition; B sets only `.task-card { transition: none !important; }`, preserving border/shadow hover appearance.
3. Use a synthetic hover-like class on the card crossing the same fixed viewport point in **all four phases** to produce repeatable hover property changes without global mouse automation. Mark this as a proxy, not hardware pointer/trackpad fidelity.
4. Timestamp each phase with `console.timeStamp()` (documented in WebKit Inspector timelines) plus unique dispatched events. Store numerical phase boundaries and trial outcomes through **Tauri IPC in the synthetic QA SQLite** outside the timed phase.
5. Safari Web Inspector can start/stop via `console.profile()`/`console.profileEnd()` (per WebKit official Timelines reference). Human should, at most, attach one Safari Inspector and export one A–B–B–A recording. Control valid vs invalid focus and Inspector overhead; no claims of measured screen FPS.

## RED / GREEN criteria

- Instrumentation correctness: in A, the computed `.task-card` transition must include `box-shadow` and `border-color`; in B it must be `none`. Matched 4-phase dataset, enough scroll events/travel, and at least 10 collected rAF gaps each phase; repeated transitions occur in A and are suppressed on B.
- Main performance decision **must use Inspector compositing**, not screenshots/sec or only rAF. Candidate requires in the B pair at least **30% fewer composites >16.7ms per active scroll-second** and **30% lower compositor p95** compared with A pair; both pass on focus-valid A–B–B–A. This is experimental threshold, **not** a product FPS SLA.
- If one Inspector capture cannot be obtained safely, stop at 'QA prototype prepared, profiler proof pending' rather than claiming a CSS fix.

## Execution order

1. Prepare versioned QA script plus ephemeral copied frontend + unique QA bundle/config/database; ensure normal app/data untouched.
2. Unit-test variant switching, calculated-style guards, phase markers, and metadata contract; smoke-check via native isolated QA app once.
3. If autonomous rAF/transition A/B suggests a compositor candidate, record one Safari Inspector trace containing all four phases; analyze before/after compositor cost using the existing privacy-safe numeric parser extended for explicit markers.
4. Review Standards/Spec, document one of **supported / rejected / inconclusive**, and only then propose a minimal production change. Restore normal debug `Tasknotes.app` if QA rebuild overwrote raw debug target. Keep all PRs Draft; no merge.

**Out of scope:** any normal Tasknotes CSS or DB changes, webkit private APIs, changing global Safari/macOS preferences, force-moving macOS mouse focus, or uploading the raw Safari screenshots.
