# 09: Removed and restore

**What to build:** Move Tasks, Notes, and Lists into Removed instead of deleting; restore prior state. Removing a List hides child Tasks, and restoring it preserves any independently Removed children. Links remain valid. Normal search/list excludes Removed.

**Blocked by:** 04 — Inbox, Lists, tags, deadlines, 06 — Standalone Markdown Notes, 08 — Reversible Task–Note links

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Removing Tasks, Notes or Lists sets reversible Removed disposition rather than physically deleting database rows.
- [ ] Restoring a Task or Note recovers previous details, status, progress, links and associated history.
- [ ] Removing a List hides its child Tasks from active views without independently removing those child Tasks.
- [ ] Restoring that List does not restore a child that had been individually moved to Removed.
- [ ] Normal views and searches exclude Removed; a dedicated Removed view shows restorable records.
- [ ] No ordinary UI or core CRUD route provides permanent deletion or an automatic purge.

## Verification

- [ ] Exercise nested List/Task Removed and restore sequences in core and UI, including individually Removed children and links.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
