# 17: Large-dataset performance acceptance

**What to build:** Synthetic 50,000 Task/10,000 Note dataset; profile indexed search p95 with agreed <250 ms goal, virtualize and measure dynamically expanded rows, measure scrolling near 60 FPS and AI paging/memory. Fix measured bottlenecks rather than guessing.

**Blocked by:** 05 — Indexed Task search and filters, 07 — Note organization and searching, 12 — MCP stdio integration and safe bulk mutations, 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Create a deterministic, public-safe synthetic dataset containing 50,000 Tasks and 10,000 Notes without importing personal data.
- [ ] Record query/search p95 and warm/cold behavior on the target Mac against the under-250 ms search goal using reproducible scenarios.
- [ ] Measure scroll performance close to 60 FPS with collapsed/expanded variable-height Task rows; record hardware, frame-time method and dataset.
- [ ] Verify UI/CLI/MCP paging and memory do not load all 60,000 records into a single process result or UI render.
- [ ] Identify and address measured regressions, or explicitly report unmet acceptance targets rather than claiming benchmark success.

## Incoming performance defect gate

- The user reported severe native GUI lag with only one synthetic Task on 2026-10-12. See [Bug 19](19-native-interaction-lag.md). Reproduce and measure it before asserting performance readiness. Do not mark this ticket complete while the bug is unresolved.

## Verification

- [ ] Commit a repeatable synthetic benchmark harness and review the measured profile and result summary.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
