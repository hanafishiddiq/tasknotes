# 07: Note organization and searching

**What to build:** Folders that can be nested (folders inside folders), pinned Notes, notes list/search, and global query coverage of Note content, with keyboard-ready navigation and reasonable page sizes. Verify Markdown content indexing and ordering.

**Blocked by:** 06 — Standalone Markdown Notes

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Users can organize Notes into folders, pin/unpin Notes, and navigate a dedicated Notes workspace.
- [ ] Folders can contain subfolders; a Note can live in any folder; the folder tree survives restart; moving a folder into itself or one of its own subfolders is refused.
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

## Owner decision — nested Note Folders (2026-10-22)

Han asked for folders inside folders and chose to build it here, with the folder feature, rather than as a later ticket. This applies to **Note Folders only**; Task Lists stay flat. Recorded in the [spec](../spec.md) (user story 29 and "Nested Note Folders") and `GLOSSARY.md`.

Decided: a folder contains subfolders and Notes; the tree is independent of Task Lists; no folder may become its own ancestor; removing a folder is reversible.

Open, to settle with the owner when this ticket starts:
- Maximum nesting depth.
- Whether opening a folder also lists Notes from its subfolders.
- Name rules among sibling folders (unique or not, case handling).
- How Removed and restore treat a folder's subfolders and Notes (Ticket 09).
- How the sidebar shows the tree (expand/collapse, remembered state) within bounded rendering.

## Owner decisions — open items settled (2026-10-25)

Han chose, when this ticket started:
- **Maximum nesting depth:** 5 levels (a top-level folder is level 1). Deeper creates/moves are refused by the core.
- **Opening a folder:** lists only the Notes directly in that folder, not Notes from its subfolders. Search still finds Notes in every folder.
- **Sibling names:** unique among siblings of the same parent, compared case-insensitively ("Rapat" and "rapat" cannot share a parent). Same name under different parents is allowed.
- **Sidebar tree:** expand/collapse per folder, and the expanded state is remembered across restarts.
- Still deferred to Ticket 09: how Removed and restore treat a folder's subfolders and Notes.
