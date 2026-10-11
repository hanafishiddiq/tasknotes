# 08: Reversible Task–Note links

**What to build:** Link one or more Notes to a Task and one Note to multiple Tasks, navigate the relationship from each view and unlink without destroying either resource. Test link integrity after edits.

**Blocked by:** 04 — Inbox, Lists, tags, deadlines, 06 — Standalone Markdown Notes

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] One Note can be linked to several Tasks, and one Task can link to several Notes.
- [ ] Users can navigate relationships from both Task and Note views, with stable IDs and no duplicate associations.
- [ ] Unlinking never removes or modifies either linked Task or Note.
- [ ] Deleting a relationship is recoverable where Undo is offered, and invalid references cannot introduce dangling links.
- [ ] Relationships survive app restart and work across different Task Lists.

## Verification

- [ ] Run core link-integrity tests and desktop cross-navigation/link/unlink scenarios.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
