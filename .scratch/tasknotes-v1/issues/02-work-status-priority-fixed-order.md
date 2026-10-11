# 02: Work status, priority, and fixed ordering

**What to build:** Change Task status and all five priority levels from UI, see intuitive symbols/colors and predictable In Progress → Ready → Completed grouping. Enforce 100%-is-Completed and progress-driven work status behavior in the core, not only React.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Users can choose Ready, In Progress, or Completed and all five priorities; every priority has its agreed color, icon and accessible text label.
- [ ] All Tasks always renders In Progress above Ready above Completed, regardless of chosen secondary sort.
- [ ] Ready is for not-yet-started Tasks; once a Task has started, becoming idle does not change status or its saved progress.
- [ ] Progress 1–99% on a Ready Task moves it to In Progress; 100% moves it to Completed; contradictory status/progress writes are rejected, not silently coerced.
- [ ] Switching a partly completed Task back to Ready may not silently discard progress; such a request requires an explicit reset/validation path, not an inactivity trigger.
- [ ] UI and Rust core tests agree on ordering and transitions, including Tasks sharing the same priority.

## Verification

- [ ] Test state-transition table via the Rust core and verify ordering, icon labels, and validation through the Tauri UI.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
