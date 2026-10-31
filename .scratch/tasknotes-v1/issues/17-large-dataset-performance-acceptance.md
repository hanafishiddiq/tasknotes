# 17: Large-dataset performance acceptance

**What to build:** Synthetic 50,000 Task/10,000 Note dataset; profile indexed search p95 with agreed <250 ms goal, virtualize and measure dynamically expanded rows, measure scrolling near 60 FPS and AI paging/memory. Fix measured bottlenecks rather than guessing.

**Blocked by:** 05 — Indexed Task search and filters, 07 — Note organization and searching, 12 — MCP stdio integration and safe bulk mutations, 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** measured and code-reviewed; on `integration/tasknotes-v1-07-18` (Draft PR #20), awaiting the owner's trackpad check and merge decision. [Report](../../../docs/qa/ticket-17-performance-2026-10-30.md).

## Acceptance criteria

- [x] Create a deterministic, public-safe synthetic dataset containing 50,000 Tasks and 10,000 Notes without importing personal data. *(`crates/tasknotes-bench`; content is deterministic, ids and timestamps come from the core)*
- [x] Record query/search p95 and warm/cold behavior on the target Mac against the under-250 ms search goal using reproducible scenarios. *(through the core, CLI, MCP and the app; "cold" is a fresh process with a warm OS cache)*
- [x] Measure scroll performance close to 60 FPS with collapsed/expanded variable-height Task rows; record hardware, frame-time method and dataset. *(scripted, 60 FPS on a 60 Hz display; a real trackpad stays in the owner check)*
- [x] Verify UI/CLI/MCP paging and memory do not load all 60,000 records into a single process result or UI render.
- [x] Identify and address measured regressions, or explicitly report unmet acceptance targets rather than claiming benchmark success. *(one-letter Note search fixed; scrolling reported as not measured)*

## Incoming performance defect gate

- The user reported severe native GUI lag with only one synthetic Task on 2026-10-12. See [Bug 19](19-native-interaction-lag.md). Reproduce and measure it before asserting performance readiness. Do not mark this ticket complete while the bug is unresolved.

## Verification

- [x] Commit a repeatable synthetic benchmark harness and review the measured profile and result summary.
- [x] Relevant tests pass and evidence is recorded here.
- [x] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-30 — the Issue 19 gate.** Scripted scrolling on the real window, in front and unlocked, held 60 FPS with no frame over 20 ms at this dataset. Issue 19 is closed as `wontfix` on the owner's report that the ordinary app is smooth, and its root cause was never proven. The owner's trackpad check at this dataset settles the gate.
