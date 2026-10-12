# Issue #19 — Trusted physical-wheel A–B–B–A red-reproduction plan

**Status:** Planned *before* implementation, 2026-10-12. **Branch:** `test/issue19-trusted-wheel-ab`, stacked on closed-result diagnostic prototype PR #9; QA-only, not production fix.

## Problem and root-gap

User-perceived native Tasknotes trackpad scrolling is severely janky. First human-driven Safari trace: 13/44 composites >16.7ms, composite p95 28.97ms. The completed four-phase synthetic A/B trace has *zero* expensive compositing in baseline A despite focus, script markers and CSS transitions. Synthetic `new WheelEvent("wheel")` and `scrollBy` do not run through real trusted wheel default scrolling; see [MDN WheelEvent()](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent/WheelEvent). Thus synthetic A/B cannot be promoted to a physical-scrolling fix.

## Decision: one QA-only real-input mode (cheapest valid seam)

Do **not** change production UI or build a second renderer. Add a separate **Start REAL trackpad A–B–B–A** control to the existing isolated Tasknotes QA app; keep the original synthetic mode unchanged. Human scroll input is necessary because system-wide mouse/keyboard injection is disallowed.

- Each of A1/B1/B2/A2 uses the existing original Tasknotes UI, synthetic database, same window and CSS. A preserves card hover transitions; B disables *only* the same transitions. Reset scroll to top **before** each timed phase; hold a brief pause/countdown for the human.
- During each timed window (6 seconds), collect only WebKit-generated `wheel` events with `event.isTrusted === true`; **do not** dispatch synthetic wheel events or call `scrollBy` during measurement. Natural CSS :hover should occur while the pointer remains stationary over Task cards.
- Require window `visible`, `document.hasFocus()`, >=35 trusted wheel events and >=600px accumulated real scroll. Require at least 12 rAF gaps, A transitionruns >0, B transitionruns =0; reject otherwise as INCONCLUSIVE.
- To avoid an apples/oranges A/B, compare phase wheel-count and travel range (largest/smallest at most 2.5x). Report the actual four per-phase values: human input cannot be perfectly fixed.
- Reuse unique `ISSUE19_HOVER_A1_BEGIN` / END timeline markers and the existing numeric-only Safari comparator. Require >=5 composites/phase and >=20 Inspector wheel events/phase.
- **RED prerequisite:** For a change to count as a candidate fix for the reported bug, the physical A baseline must itself show *nonzero composites above 16.7ms*. Only then apply the already predeclared >=30% improvements in both compositor p95 and slow-composite events/sec. rAF and Inspector screenshots are not delivered physical FPS.
- Keep raw Safari export, screenshots and user data outside Git; use only QA's synthetic SQLite. No backgroundThrottling changes, WebKit private APIs, global input injection or user-facing production CSS patch.

## Pre-trace evidence binding (2026-10-13 QA hardening)

Source correction: `Event.isTrusted` establishes user-agent generation rather than proof of physical hardware; a human physically performing the trackpad action remains part of the test protocol. Source: https://developer.mozilla.org/en-US/docs/Web/API/Event/isTrusted.

Before treating an exported Safari timeline as physical-wheel evidence, the numeric comparator must also read the latest **QA-only** SQLite metrics row in **read-only mode**, verify `inputMethod=physical`, all four valid/focused/comparable phases, and compare the Inspector's phase windows, wheel counts and CSS transitions against the stored QA measurements. Reject an older scripted trace, mismatched export, missing markers, invalid QA run, or missing per-phase samples as **INCONCLUSIVE**, not as a performance failure. Check that the exported file timestamp is after the QA trial.

Preserve the current script's numeric-only synthetic analysis for historical experiments. Make the stricter check opt-in through `--require-physical-qa` and mandate this flag for the physical experiment. No raw screenshot/DOM/private Task data will be copied or committed. For a CSS candidate, also require expensive composites in **both** A1 and A2 baselines before evaluating the unchanged 30% criteria. Run TDD before implementation.

## Execution phases and acceptance

1. **TDD:** Extract a small pure input/gate helper; test that untrusted events are rejected, physical validity enforces focus/distance/event thresholds, synthetic mode still behaves the same. Tests must fail before helper implementation.
2. **QA implementation:** Add separate physical-start control, real input listener, fixed timed phases and explicit mode in the synthetic run record. Build only opt-in QA bundle; never enable physical QA code in normal Tasknotes.
3. **Automation:** Run Node/Python comparator tests, typecheck, frontend/Rust tests, and native debug QA build. Ensure normal mode still works and no unexpected side effect. Human phase run is not automated by bypassing macOS input restrictions.
4. **One human Safari capture:** Attach Web Inspector to the QA app. Select physical mode; keep the pointer over Task cards and continuously scroll up/down with trackpad during each of the four 6-second labelled phases. Export ONE trace when complete. Reject incomplete/focus-lost or imbalanced input, don't massage thresholds post-hoc.
5. **Review:** Compare real-input A/B Inspector trace, independent Standards/Spec review, then decide `supported / rejected / inconclusive`. Keep stacked Draft, no merge and Issue #19 OPEN until actual user-visible improvement is proven.

## Alternatives considered and rejected

- More synthetic wheel replay: previous baseline never reproduced expensive work.
- macOS Quartz/global mouse injection: outside permitted controller scope.
- Native renderer rewrite/child WKWebView/Release-only changes: previously failed matched or relevant tests.
- Promoting CSS change from 32.3% *synthetic* p95 alone: fails slow-event criterion and user-visible reproduction.
