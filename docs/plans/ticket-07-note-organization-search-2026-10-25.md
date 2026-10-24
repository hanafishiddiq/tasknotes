# Ticket 07 — Note organization and searching: plan (2026-10-25)

Owner instruction 2026-10-25: start Ticket 07 on `integration/tasknotes-v1-07-18`. Owner decisions are recorded in the ticket ("Owner decisions — open items settled (2026-10-25)").

## Owner decisions (from the ticket)

- Nested Note Folders, at most 5 levels (a top-level folder is level 1).
- Opening a folder lists only the Notes directly in it; search finds Notes in every folder.
- Sibling folder names are unique under one parent, compared case-insensitively; the same name under different parents is allowed.
- Sidebar tree: expand/collapse per folder, expanded state remembered across restarts.
- Removed and restore for folders stay with Ticket 09.

## Design

**Schema v7 (forward-only, one transaction, self-contained in `note_folders::migrate_v7`).** Ticket 08 adds its own schema step on a sibling branch; whichever merges second renumbers its step (a two-line change: the `PRAGMA user_version` in the step and the runner's maximum).
- `note_folders(id, parent_id → note_folders, name, name_fold, revision, created_at, updated_at)`. `name_fold` is the Unicode lower-case name (SQLite `NOCASE` folds ASCII only), as Tags do. A unique index on `(coalesce(parent_id, ''), name_fold)` backs the sibling rule the core checks first. No `removed_at` yet: how Removed treats a subtree is still open (Ticket 09), so that ticket adds the column with its rules.
- `notes.folder_id` (NULL = not in a folder) and `notes.pinned` (0/1). Indexes `(pinned DESC, updated_at DESC, id DESC)` for All notes and `(folder_id, pinned DESC, updated_at DESC, id DESC)` for one folder; the v6 `notes_updated_order` index is replaced.
- `notes_fts`: FTS5 over the Markdown, external content (`content='notes'`), same tokenizer as Task search (`unicode61 remove_diacritics 2`), kept in sync by triggers so every future writer (CLI, MCP) maintains it; rebuilt from existing Notes in the migration.

**Core rules (all in Rust, shared by desktop, CLI and MCP):**
- Folder create/rename/move/list-children. A move into the folder itself or any of its subfolders is refused; a create or move that would put any folder below level 5 is refused; a sibling name clash (case-insensitive) is refused. Folder writes check the folder's revision.
- Siblings sort by folded name, then id: deterministic, nothing to persist.
- Notes: one query entry point `query_notes` (scope All or one folder, optional search text) returning bounded keyset pages of summaries: pinned first, then most recently edited, then id. The cursor is bound to the query that made it.
- Search: every word is a quoted prefix term (user text never reaches FTS syntax), all words must match, search spans every folder, results keep the list order (no relevance sort, as for Tasks), each hit carries a short plain-text snippet from FTS5 `snippet()` with matched words marked, so no Note body is sent to the list.
- Pin and move to folder change only placement. They do not bump the Note's content revision or edit time: the revision protects the Markdown, and bumping it would make the open editor's autosave conflict whenever its Note is pinned or moved. Both are idempotent target-state writes ("pinned = yes", "folder = X"), so concurrent placements cannot lose text.

**Desktop:**
- Sidebar Notes section: All notes plus the folder tree, loaded one bounded page of children per expanded folder; expand state kept in `localStorage` (`tasknotes.noteFolders.expanded`) like the theme, since it is a view preference rather than shared data. Row actions: new subfolder, rename, move (destination list from the loaded tree; the core judges cycles and depth).
- Notes workspace: list of the chosen scope with a Pinned group, a search field (all folders), pin/unpin and a folder picker for the open Note, a new Note is created in the folder being viewed. Arrow keys move between Notes in the list.
- Global search: the toolbar search (now "Search tasks and notes") also shows up to 5 matching Notes with snippets above the Tasks; a hit opens Notes with the same search; "Show all matching notes" opens the full, paged result.

## Vertical TDD slices

1. Core: v6 → v7 migration (Notes intact, failed migration rolls back), create and list nested folders, sibling names, depth limit on create.
2. Core: move a folder — cycle refusal, depth limit counting the moved subtree, name clash at the destination, revision check.
3. Core: Notes in folders and pins — create in a folder, move, pin/unpin, ordering and paging per scope, persisted across reopen.
4. Core: Note search — titles and body, edits reflected, empty Notes, every folder, snippets, foreign cursors; large synthetic fixture stays page-bounded.
5. Tauri commands.
6. Desktop: folder tree (create, nest, expand remembered), folder view, new Note in folder.
7. Desktop: pin/unpin and move the open Note; Pinned group.
8. Desktop: Notes search and arrow-key navigation.
9. Desktop: global search shows Notes beside Tasks.
10. Gates, ticket evidence, owner click-checklist.

## Risks

- Schema v7 is forward-only like v4–v6: no build may open the owner's database without a verified backup.
- `notes_fts` keys on `notes.rowid`, like the Task indexes key on `tasks.rowid`. `VACUUM` may renumber rowids of tables without an `INTEGER PRIMARY KEY`; Ticket 13 (backup/restore) and Ticket 17 should rebuild the FTS indexes after any `VACUUM` (`INSERT INTO notes_fts(notes_fts) VALUES('rebuild')`).
- Broad one-letter prefixes match many Notes; the page is bounded but SQLite sorts all matches. Ticket 17 measures this.
- Native GUI QA is not run in this ticket; a click-checklist for the owner is in the ticket.
