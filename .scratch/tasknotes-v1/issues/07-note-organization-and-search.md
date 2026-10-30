# 07: Note organization and searching

**What to build:** Folders that can be nested (folders inside folders), pinned Notes, notes list/search, and global query coverage of Note content, with keyboard-ready navigation and reasonable page sizes. Verify Markdown content indexing and ordering.

**Blocked by:** 06 — Standalone Markdown Notes

**Status:** ready-for-agent

**Implementation:** review findings fixed; re-review done on 2026-10-30 and its findings fixed; awaiting the owner's native check and merge decision; on `integration/tasknotes-v1-07-18` (Draft PR #20).

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
- [x] Run `/code-review` and address findings before marking complete. *(see the review section below)*

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

**Schema v8** (`note_folders::migrate_v8`, one transaction; written as v7 and renumbered to v8 when the integration branch brought Ticket 08's v7 `task_note_links`, which runs first): `note_folders` (parent, name, Unicode-folded name, revision; unique `(parent, folded name)`), `notes.folder_id`, `notes.pinned`, list indexes `(pinned, updated_at, id)` overall and per folder, and `notes_fts` (FTS5, external content, triggers on insert/update/delete, rebuilt from existing Notes). No folder `removed_at` yet: Ticket 09 decides how Removed treats a subtree.

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

## Merge of `integration/tasknotes-v1-07-18` (c10e536, Ticket 08) — 2026-10-25

- Schema renumbered: Ticket 08's `task_note_links` stays v7; this ticket's step is now v8 (`migrate_v8`, runner maximum 8). A failed v8 step leaves v7 committed (`a_failed_v8_migration_rolls_back_and_keeps_the_database_usable`). Ticket 08's v6 fixture in `tests/links.rs` now also rebuilds `notes` in its v6 shape and drops the v8 objects; current-version asserts are 8.
- One request into Notes for both tickets: `NotesRequest { seq, search, openId }`. Global search sends its text; a Task's linked Note sends an empty search and the Note to open, so both go through the same list-race guard. Opening a linked Note shows All notes.
- Test stand-ins: the shared Note stand-in answers `list_note_tasks`; `links.test.tsx` gained `folderId`, `pinned`, `snippet` and `list_note_folders`; the locale-safe paging assertion keeps Ticket 08's `noteButton` helper.

**Checks after the merge, in this worktree:** `pnpm test` 148 Vitest tests in 13 files (two runs); `pnpm typecheck`; `pnpm build` (main chunk 283 kB, Notes chunk 325 kB); `cargo test` core 90 and Tauri 1; core and Tauri `cargo clippy --all-targets -- -D warnings`; both `cargo fmt --all -- --check`.

## Independent review (2026-10-25): REQUEST CHANGES — fixes

| Finding | Fix | Evidence |
| --- | --- | --- |
| MAJOR: `snippet()` ran for every match before ORDER BY/LIMIT (reviewer: 3,000 × 100 KB Notes, 88 s vs 61 ms) | The page's rows are chosen first; snippets are cut only for them (`f0627d0`) | `search_snippets_cost_the_page_not_every_match` (150 Notes of ~100 KB): page of 1 ≈ 3 ms vs page of 100 ≈ 50 ms; on the old code both ≈ 71 ms and the test fails. Reviewer's bench, release, 500 × ~100 KB with the word ~3,000 times per Note: first page of 50 in 1.3 s (about 13 s before). The rest is FTS5 choosing a fragment among many hits per Note; Ticket 17 should measure it |
| Minor: names folded with `to_lowercase()` only | One shared fold `text_fold::fold` (NFD, full case folding via `caseless`, NFC) for Tags, the Tag filter and sibling Folder names; the v8 step re-folds stored Tags and merges Tags that now fold alike (first spelling kept, every Task keeps the Tag once; only the duplicate Tag row goes) (`e0ebe27`). Folder folds need no recompute: v8 creates the table | `tags_match_with_full_case_folding_in_any_unicode_form`, `opening_a_v7_database_merges_tags_that_now_fold_alike` (fails with the re-fold skipped), Folder cases Straße/STRASSE, ΣΑΣ/σας, é NFC/NFD in `sibling_names_are_unique_ignoring_case_but_may_repeat_under_other_parents`. New deps: `caseless` 0.2.2, `unicode-normalization` 0.1.26 |
| Minor: "!!!" listed every Note; client regex repeated the word rule | The core returns an empty page for non-blank text without words; the App regex is gone (`daec63b`) | `search_finds_titles_and_markdown_content_in_every_folder` (now RED before the fix), UI `asks the core even for text without words, which matches no Notes` |
| Nit: v8 Note parts lived in `note_folders.rs` | v8 split by owner: `note_folders::migrate_v8` (tree), `notes::migrate_v8` (columns, indexes, FTS), `organization::refold_tags`; the runner sets `user_version = 8` in the same transaction (`e0ebe27`) | Migration and rollback tests unchanged and green |
| Nit: Tauri `create_note` took `folder_id` beside its input | `CreateNote { markdown, folder_id }`; `create_note_in_folder` removed (`a25f02e`) | Core and UI tests assert the new shape |
| Nit: misplaced/unclear comments in `NotesWorkspace.tsx`; glossary "Folder" | Fixed; glossary term is now Note Folder (`a25f02e`) | — |
| Spec gap: keyboard navigation in the folder tree | Up/Down move, Right opens or steps in, Left closes or steps out to the parent (`3b6f153`) | `moves through the folder tree with the arrow keys` (fails with Left-to-parent removed) |

**Extra scope kept (owner may decide):** renaming a folder was not asked for in the ticket; it stays, with the same name rules and revision check as moves.

**Owner click-checklist addition:** in the sidebar tree, use Up/Down/Right/Left on folders as above; create "Straße" and then "STRASSE" in one parent and expect the name clash.

**Checks after the fixes, in this worktree:** `pnpm test` 150 Vitest tests in 13 files (two runs); `pnpm typecheck`; `pnpm build` (main chunk 283 kB, Notes chunk 325 kB); `cargo test` core 93 and Tauri 1; core and Tauri `cargo clippy --all-targets -- -D warnings`; both `cargo fmt --all -- --check`.

**2026-10-25 — owner decision (Han):** Merging Tags whose names are equal under full Unicode case folding during the v8 upgrade is accepted. Every Task keeps the Tag; only the second spelling of the Tag name disappears.

**2026-10-30 — re-review outcome (implementer).** One finding touched this ticket's search: `query_notes` chose its page in one statement and cut snippets in others, so a snippet could come from text newer than the page. Page and snippets now read one deferred read transaction (`a49afeb`); ordering, paging and the snippet cost are unchanged. The privacy consequence and its test are recorded in Ticket 11.

Evidence (implementer's own runs, synthetic data; these fixes have had no further independent review): `cargo test` core 204, CLI 45, MCP 34, src-tauri 42, all passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean in all four; `pnpm test` 261 passed in 25 files; `pnpm typecheck` and `pnpm build` clean. Native checks in the click-checklist above are still the owner's.
