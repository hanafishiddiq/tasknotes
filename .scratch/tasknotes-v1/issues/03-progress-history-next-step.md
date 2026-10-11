# 03: Progress history and Next Step

**What to build:** Record 0–100% progress with dated append-only Progress Entries and Next Step; view recent history on expanded Tasks; manually reopen Completed Tasks at the previous incomplete percentage. Test time ordering and transitions.

**Blocked by:** 02 — Work status, priority, and fixed ordering

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] A Task exposes editable progress 0–100%, Next Step, and dated progress notes without leaving the expanded Task.
- [ ] Every substantive progress/status change creates an append-only dated Progress Entry; earlier history remains readable.
- [ ] Completing a Task forces 100%, while an explicit reopen restores its previous incomplete percentage and returns to In Progress.
- [ ] Temporarily inactive Tasks retain In Progress status, progress, Next Step and history across application restarts.
- [ ] Invalid percentages or updates that conflict with current status do not corrupt stored values or history.

## Verification

- [ ] Run core behavior tests for 0/1/99/100 transitions, reopen and history, plus one desktop persistence scenario.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
