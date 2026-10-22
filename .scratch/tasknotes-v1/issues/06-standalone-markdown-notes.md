# 06: Standalone Markdown Notes

**What to build:** Notes view and first Note end-to-end: rich-text editor creates/updates canonical Markdown, autosave persists and reloads; basic headings, lists, checklists, formatting and links round-trip safely. Uses a focused core Note interface.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** in progress — owner started Ticket 06 on 2026-10-22 after Ticket 05 merged. [Plan](../../../docs/plans/ticket-06-markdown-notes-2026-10-22.md).

## Acceptance criteria

- [ ] Users can create, edit and reopen a standalone Note without any associated Task.
- [ ] Headings, bold, italic, bullet lists, checklists and links round-trip through WYSIWYG editing and canonical Markdown without loss in supported cases.
- [ ] Autosave persists typing without manual Save and does not silently overwrite a more recent conflicting edit.
- [ ] Note creation and updates use the same Rust core and local SQLite persistence as desktop Tasks.
- [ ] Invalid content or interrupted writes leave the last consistent saved version recoverable.

## Verification

- [ ] Run Markdown round-trip cases, forced autosave/restart scenarios and core persistence tests on synthetic Notes.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
