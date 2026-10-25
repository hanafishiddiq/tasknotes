# 07: Note organization and searching

**What to build:** Folders that can be nested (folders inside folders), pinned Notes, notes list/search, and global query coverage of Note content, with keyboard-ready navigation and reasonable page sizes. Verify Markdown content indexing and ordering.

**Blocked by:** 06 — Standalone Markdown Notes

**Status:** ready-for-agent

**Implementation:** in progress — implemented, awaiting independent review

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

## Implementation checkpoint — slices 1–9 (2026-10-25)

Plan: `docs/plans/ticket-07-note-organization-search-2026-10-25.md`. All tests use synthetic fixtures; the owner's database was never opened.

**Schema v7** (`note_folders::migrate_v7`, one transaction, self-contained so it can be renumbered when Ticket 08's schema step merges): `note_folders` (parent, name, Unicode-folded name, revision; unique `(parent, folded name)`), `notes.folder_id`, `notes.pinned`, list indexes `(pinned, updated_at, id)` overall and per folder, and `notes_fts` (FTS5, external content, triggers on insert/update/delete, rebuilt from existing Notes). No folder `removed_at` yet: Ticket 09 decides how Removed treats a subtree.

**Core (Rust, shared by desktop, CLI and MCP):**
- Folders — `tests/note_folders.rs` (9): `a_v6_database_keeps_its_notes_and_gains_folders` (also: old Notes searchable), `a_failed_v7_migration_rolls_back_and_keeps_the_v6_database_usable`, `folders_nest_and_the_tree_survives_reopening_the_store`, `sibling_names_are_unique_ignoring_case_but_may_repeat_under_other_parents` (incl. "Één"/"één"), `a_sixth_level_folder_is_refused`, `folder_pages_are_bounded_and_never_repeat_or_skip`, `a_folder_cannot_move_into_itself_or_any_of_its_subfolders`, `a_move_is_refused_when_the_moved_subtree_would_pass_five_levels`, `moving_or_renaming_respects_names_at_the_destination_and_the_revision`.
- Placement and order — `tests/note_placement.rs` (4): a folder lists only its own Notes, All notes lists every Note; moving and pinning keep the text, revision and edit time (an editor holding the old revision still saves); pinned first, each group most recently edited first, kept across reopening; keyset pages over pinned and unpinned Notes never repeat or skip; foreign cursors refused.
- Search — `tests/note_search.rs` (3): titles and Markdown in every folder (prefix, accents, case; every word must match; plain-text snippet with marked words, no `**`); edits reflected, empty Notes never match the placeholder title "New Note", a Note emptied by an edit leaves the results; results keep the list order in bounded pages, cursor bound to the search text, more than 16 words refused, optional folder narrowing.
- `tests/large_note_fixture.rs` (1): 5,000 synthetic Notes, every page ≤ 100, listing, one folder and a 2,500-hit search walk in exact order without repeats (timings printed, not asserted; Ticket 17 benchmarks).
- Mutation checks, each making a test fail: depth `>=`→`>`, ASCII-only name fold, cycle check removed, subtree height ignored, pins not in the order, cursor not bound to the scope, no FTS rebuild in the migration, no update trigger, raw snippet without mark stripping.

**Tauri:** `list_notes` (folder scope, text), `create_note` (folder), `move_note`, `set_note_pinned`, `list_note_folders`, `create_note_folder`, `rename_note_folder`, `move_note_folder`.

**Desktop (mocked Tauri seam, shared stand-in `src/test/note-backend.ts`):**
- `src/note-folders.test.tsx` (6): nested folders created from the sidebar and their open/closed state remembered across a restart; a folder lists only its own Notes and a new Note is created inside it; rename and move a folder, with the core's refusal shown and the tree unchanged; pin/unpin the open Note (Pinned group, no autosave conflict afterwards); an edited Note stays below pinned ones; move the open Note to another folder and out of every folder.
- `src/note-search.test.tsx` (6): Notes search across every folder from inside a folder, marked snippet, opening a hit, clearing back to the folder; "No matching notes"; arrow keys between Notes; global search shows up to 5 matching Notes beside the Tasks with no `get_note` call; a hit opens Notes with the same search; "Show all matching notes" opens the full paged result.
- Mutation checks, each making a test fail: expanded state not persisted, new Note not created in the folder, search limited to the current folder, edited Note placed above pinned ones.
- Existing tests: the toolbar search is now "Search tasks and notes" (label updated in `search-filters.test.tsx`); `notes.test.tsx` uses the shared stand-in; its paging test was locale-dependent (failed under `en_NZ`, where the date reads "1 Jan" right after the title "Note 1") and now matches the title text.

**Self-made decisions (owner may revisit):**
- Pinning and moving a Note change placement only: no content revision bump and no new edit time, so the open editor never conflicts after a pin or move. Concurrent placements are last-writer-wins on a target state ("pinned", "in folder X"), which cannot lose text.
- Sibling folders sort by name (case-insensitive); there is no manual folder order.
- The folder tree loads when Notes are first opened, like the editor, so Task startup does not wait for it; until then the sidebar shows only All notes.
- Expanded folders are remembered in this window's `localStorage` (`tasknotes.noteFolders.expanded`), like the theme, not in the database.
- Destination lists (move a folder, move a Note) offer every folder loaded in the sidebar; a folder inside a collapsed, never-opened parent appears after opening that parent. The desktop does not pre-filter impossible destinations; the core refuses them with its reason.
- Opening a folder ends a Notes search; global search results open in All notes with the same search.
- Global search shows Notes above the Tasks (at most 5) and searches Notes only when the text contains a letter or digit.
- Search results keep the list order (pinned, then most recently edited), not relevance, as Task search does.

**Checks (2026-10-25, before merging the integration tip):** `pnpm test` 140 Vitest tests in 12 files; `pnpm typecheck`; `pnpm build` (main chunk 277 kB, Notes chunk 323 kB); `cargo test --manifest-path crates/tasknotes-core/Cargo.toml` 77 tests; `cargo test --manifest-path src-tauri/Cargo.toml` 1 test; core and Tauri `cargo clippy --all-targets -- -D warnings`; both `cargo fmt --all -- --check`. Run in a copy of this worktree outside `~/Desktop` (see the environment note in the agent report), same sources.

**Not done here:** removing/restoring folders (Ticket 09); AI-facing Note search and Exclude from AI (Ticket 11); native GUI QA (owner click-checklist below); drag and drop.

### Owner click-checklist (native app, isolated synthetic data)

Run a build with `TASKNOTES_DEV_DATA_DIR` pointing at a fresh folder (never the real database).

1. Click All notes. Type "Work" in New Folder and press Return: "Work" appears under All notes.
2. Hover Work, click the folder-plus icon, type "Meetings", Return: Work opens with Meetings inside. Repeat inside Meetings down to level 5; a sixth level shows "Folders can be nested at most 5 levels deep".
3. Quit (Cmd+Q) and reopen, then click All notes: Work is still open showing Meetings. Collapse Work, quit, reopen: it stays collapsed.
4. Create "rapat" and then "Rapat" at the top level: the second shows "A folder with this name already exists here".
5. Hover Work, click the move icon, pick "Work / Meetings": "A folder can't be moved into itself or one of its subfolders"; nothing moves.
6. Click Work, click New note, type "Budget review": the Note appears in Work only; All notes lists it too; Meetings does not.
7. With the Note open, click the pin icon: it moves to the top under "Pinned". Keep typing: it saves without a conflict banner. Quit and reopen: still pinned. Unpin: it returns among the others.
8. Use the Folder picker above the editor to move the Note to Meetings: it leaves Work's list and appears in Meetings.
9. In the Notes search field type a word from the Note body: matches from every folder appear with the word highlighted. Clear it: the folder's Notes return. Type nonsense: "No matching notes".
10. Click a Note in the list, then use the Up/Down arrow keys: focus moves between Notes; Return opens one.
11. Go to All tasks and type the same word in the toolbar search: a "Notes" section shows at most 5 matches above the Tasks. Click one: Notes opens on it with the search filled in. With more than 5 matches, "Show all matching notes" opens the full list.
12. Check both themes (sidebar bottom): folder rows, the Pinned heading, highlighted snippet words and the editor toolbar stay readable, and the selected Note's snippet is readable on the gold selection.
