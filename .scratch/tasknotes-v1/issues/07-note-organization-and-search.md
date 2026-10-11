# 07: Note organization and searching

**What to build:** Folders, pinned Notes, notes list/search, and global query coverage of Note content, with keyboard-ready navigation and reasonable page sizes. Verify Markdown content indexing and ordering.

**Blocked by:** 06 — Standalone Markdown Notes

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Users can organize Notes into folders, pin/unpin Notes, and navigate a dedicated Notes workspace.
- [ ] Search finds Note titles and Markdown content; results reflect edits and behave correctly for empty Notes.
- [ ] Global Search can surface authorized Notes as well as Tasks, without pulling all Note bodies into the initial viewport.
- [ ] Pin and folder sorting is deterministic and persisted across restarts.
- [ ] Large Note collections are queried and displayed through bounded pages/virtualized lists.

## Verification

- [ ] Run Notes UI organization/search tests and SQLite indexing tests with synthetic Notes.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
