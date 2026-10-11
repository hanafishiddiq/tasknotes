# 04: Inbox, Lists, tags, deadlines

**What to build:** Organize Tasks in Lists/Inbox, add tags and optional deadlines, show overdue indicators, and persist CRUD changes across restart. Test List ownership and relative date display.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Tasks can be created in Inbox or moved to a named List and still appear in All Tasks without duplication.
- [ ] Users can add/remove tags and optional deadlines and see overdue indicators without violating the fixed status order.
- [ ] List/Task metadata and tag associations survive restart; nonexistent List references cannot orphan a Task.
- [ ] Overdue behavior is deterministic with a controlled local clock/time zone in tests; no forced reminder is introduced.
- [ ] Large Lists are browsed through a bounded query/viewport, not an unbounded render.

## Verification

- [ ] Exercise UI CRUD and restart on Inbox, Lists, tags, deadlines, with Rust referential-integrity and clock-boundary tests.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
