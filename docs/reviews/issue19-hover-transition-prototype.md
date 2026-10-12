# Issue #19 — Hover transition prototype Standards / Spec review

**Branch:** prototype/issue19-hover-transition-ab, stacked on actual Safari Inspector trace analysis.
**Method:** Two manual review axes. This is NOT independent human/subagent approval.

## Standards and safety

- **PASS:** Real Tasknotes compiled frontend is copied to an ignored QA output and instrumented there; the tracked production stylesheet and Tasknotes normal app behavior are unchanged.
- **PASS:** Only the card transition animation is altered by variant B. Original border-color, shadow values and hover appearance are preserved. Variant A restores the original transition. No global animation disabling, requestAnimationFrame monkeypatch, private WebKit APIs or OS-level input injection.
- **PASS:** Dedicated bundle identifier, private temporary database, exact QA-only Task title prefix and read-only metrics reporter; no normal user Tasks or ChatGrid sessions are accessed. Source CSS and injected QA script hashes are recorded.
- **PASS:** A–B–B–A in a single real native WebView controls build, viewport, CSS assets and synthetic input algorithm. Focus loss, insufficient scroll distance or missing expected CSS transitions cause an INCONCLUSIVE result.
- **PASS:** Named timeStamp/CustomEvent markers and WebKit console.profile provide the required boundaries for ONE Safari Inspector recording. An explicit comparator refuses missing/duplicate markers and does not invent physical FPS.
- **PASS:** Three Node tests and four Python comparator tests, plus the existing three Safari parser tests. An old unmarked Safari trace is rejected as INCONCLUSIVE. The native debug QA bundle (31 MiB) boots and presents the Start control.
- **NOTE:** The first unattended pilot launched in the background and correctly refused focus-invalid measurement. It saved one invalid synthetic metrics record with no measured phases. It is NOT an observation of A/B performance.
- **NOTE:** Product builds must never inherit the QA capability or copied assets. Restore the raw ordinary debug app after test-build work when necessary. Do not stage raw Safari screenshot recordings, QA SQLite or bundles.

## Spec / measured acceptance

- **PASS:** The test asks a precise question: do hover border/shadow transition events contribute substantially to observed compositing spikes?
- **PASS:** Prototype preserves the original design in A and static hover appearance in B, modifying just animation.
- **PASS (synthetic scope):** One valid, focus-preserved native A1-B1-B2-A2 run; QA card transition runs A=828, B=0; identical 200 scripted wheel events and 4,810px movement per phase.
- **PASS (measurement), FAIL (candidate):** New Safari Inspector trace contains all eight markers, 60–65 compositor records per phase, p95 reduction 32.3%, but zero >16.7ms composites in both modes. This does not meet the two-part 30% threshold or reproduce the physical-scrolling baseline.
- **PENDING:** Trusted physical wheel reproduction and display smoothness; synthetic WheelEvent/scrollBy, rAF and Inspector screenshot samples are not delivered display FPS.
- **BLOCKED:** No performance CSS production patch until measured A/B result passes both predefined >=30% criteria, and no Issue #19 resolution or merge without independent review.

**Review verdict (updated):** Scope/safety and one successful synthetic capture verified. **Do not implement production CSS change:** the current trace lacks slow compositor events even at baseline and rAF timing is unchanged. Independent reviewer approval remains outstanding. User-reported lag remains unresolved; next target is trusted physical wheel input. See [recorded analysis](../qa/issue19-hover-ab-safari-comparison-2026-10-12.md).
