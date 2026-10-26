# 09: Removed and restore

**What to build:** Move Tasks, Notes, and Lists into Removed instead of deleting; restore prior state. Removing a List hides child Tasks, and restoring it preserves any independently Removed children. Links remain valid. Normal search/list excludes Removed.

**Blocked by:** 04 — Inbox, Lists, tags, deadlines, 06 — Standalone Markdown Notes, 08 — Reversible Task–Note links

**Status:** ready-for-agent

**Implementation:** in progress — implemented and code-reviewed, awaiting owner review (branch `feat/ticket-09-removed-and-restore`, plan `docs/plans/ticket-09-removed-and-restore-2026-10-25.md`).

## Acceptance criteria

- [x] Removing Tasks, Notes or Lists sets reversible Removed disposition rather than physically deleting database rows.
- [x] Restoring a Task or Note recovers previous details, status, progress, links and associated history.
- [x] Removing a List hides its child Tasks from active views without independently removing those child Tasks.
- [x] Restoring that List does not restore a child that had been individually moved to Removed.
- [x] Normal views and searches exclude Removed; a dedicated Removed view shows restorable records.
- [x] No ordinary UI or core CRUD route provides permanent deletion or an automatic purge.

## Verification

- [x] Exercise nested List/Task Removed and restore sequences in core and UI, including individually Removed children and links.
- [x] Relevant tests pass and evidence is recorded here.
- [x] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-22:** Note Folders can be nested (owner decision, see Ticket 07). Define and test how removing and restoring a folder treats its subfolders and Notes. The List rule (children hidden without changing their own Removed disposition) is the natural default, to be confirmed with the owner.

**2026-10-25 — owner decision (Han):** Removing a Note Folder follows the List rule. Its subfolders and Notes are hidden with it without changing their own Removed disposition. Restoring the folder brings them back, except any subfolder or Note that had been moved to Removed on its own, which stays in Removed.

**2026-10-25 — from Ticket 08 (coordinator decision):** A Note's linked Tasks leave out any Task hidden by Removed (its own `removed_at`, or a Removed List); the link row is kept. Restoring the Task or its List must bring these links back into view on the Note (covered today for Lists by `a_note_lists_no_task_hidden_by_removed_and_shows_it_again_on_restore`). Decide here how a Task's linked Notes treat Removed Notes.


**2026-10-25 — implementation (Ticket 09 implementer):** Implemented on `feat/ticket-09-removed-and-restore`, not pushed, not merged. `/code-review` and native GUI QA still open.

- **Schema v9** (forward-only, one transaction): `notes.removed_at`, `note_folders.removed_at`; folder sibling names unique among active siblings only; partial `removed_at` indexes on tasks, notes, lists and folders. Tests: a v8 database keeps its Notes and folders; a failed v9 migration rolls back to v8 and the next open succeeds.
- **Core** (`crates/tasknotes-core/src/removed.rs`): `set_task_removed`, `set_note_removed`, `set_note_folder_removed` (with `set_list_removed` from Ticket 04), each with an expected revision; `list_removed` (keyset pages of 1–100, newest first, every kind). There is no delete or purge function anywhere.
- **Decisions made here (owner may revisit):**
  1. A Task's linked Notes leave out Removed Notes and Notes under a Removed folder; the link row stays (symmetric with Ticket 08).
  2. Restoring into a Removed container is refused: a Task whose List is Removed, or a Note or folder with a Removed folder above it, says "restore the List / folder first". The Removed view shows that reason in place of the Restore button. The alternative was to restore into Inbox / top level.
  3. Removed (or hidden) items are read-only: edit, move, pin, link, and create-inside are refused until restored. Unlinking stays allowed.
  4. A removed folder frees its name; restoring it over an active sibling with the same name is refused, as for Lists.
  5. Restoring a Note does not change its edit time, so it does not jump to the top of the list.
  6. The sidebar's "Show removed lists" toggle is replaced by one sidebar entry, "Removed", for every kind.
- **Desktop:** sidebar "Removed" view (Restore, Load more, no delete), "Move to Removed" in an expanded Task (with Undo), on the open Note's toolbar, and on each folder row.
- **Evidence (run in the worktree, 2026-10-25, by the implementer):** core `cargo test` 105 passed (12 in `tests/removed.rs`); Tauri `cargo test` 1 passed; `cargo clippy --all-targets -D warnings` and `cargo fmt --check` clean in core and `src-tauri`; frontend 157 tests in 15 files passed (7 new in `src/removed.test.tsx`, `src/removed-notes.test.tsx`); typecheck and build passed. Environment note: during this session the main checkout became unreadable to the sandbox (`EPERM` on `tasknotes/package.json` and `.git`), so `pnpm` could not start; the same commands ran through `npx` (`vitest run`, `tsc --noEmit` for both tsconfigs, `tsc && vite build`) with an uncommitted local config that only skips the PostCSS search above the worktree. Re-run `pnpm test` and `pnpm build` once access is back.

**Owner click-checklist (native app, synthetic data):**
1. Expand a Task with progress, a Next Step and a linked Note; click "Move to Removed". It disappears from All tasks and search; the Note's Tasks strip no longer lists it. Click Undo: it is back with the same progress.
2. Remove it again, open sidebar "Removed", click Restore: the same details, progress history and link return.
3. In a List, remove one Task on its own, then remove the List. In Removed, the List shows Restore, the Task says "Restore its List first". Restore the List: its other Tasks return, the removed Task stays in Removed and can now be restored.
4. In Notes, remove the open Note (trash icon on the toolbar): the next Note opens; the Note is gone from search and from its Task's Notes section. Restore it from Removed.
5. Make folder A with subfolder B holding a Note; remove one Note in B on its own, then remove A from its row. A, B and their Notes vanish from the tree, All notes and search. In Removed, the Note says "Restore its folder first". Restore A: B and its other Notes return; the Note removed on its own stays in Removed.
6. Remove a folder "Ideas", create a new "Ideas", then try to restore the old one: it is refused with the name message.
7. Confirm nothing anywhere offers Delete, Erase or Empty, and Removed says items stay until restored.

**2026-10-26 — review fixes (Ticket 09 implementer):** `/code-review` findings addressed on the same branch, not pushed, not merged.

- **Owner approval (Han, 2026-10-26):** decisions 2, 3 and 4 above are approved and no longer open: Removed (or hidden) items are read-only until restored, including `restore_link` while its Task or Note is Removed; restoring into a container still in Removed is refused; a Removed folder frees its name and restoring over an active same-name sibling is refused. Recorded in ADR-0002. Decision 5 (restore does not bump a Note's edit time) is unchanged.
- **Bug:** an item hidden by a Removed container can no longer be removed on its own (a Task in a Removed List: `TaskListRemoved`; a Note or folder below a Removed folder: `NoteFolderRemoved`), so restoring the container always brings back what it hid (spec: "except those separately Removed").
- **Story 36:** the Removed view is searchable. `list_removed(text, limit, cursor)` matches every word against Task fields and Progress Entries and Note text (FTS word prefixes) and List/folder names; still keyset-paged, cursor bound to its search. Tauri `list_removed` takes `text`; RemovedView has a "Search Removed" field. `TaskScope::Removed` (Ticket 05) is left as is.
- **One rule for "hidden by Removed":** `task_visible!` / `note_visible!` (with `task_in_removed_list!` / `note_in_hidden_folder!`) in `lib.rs`, used by search, Notes, links and the Removed view; shared `time|id` cursor parser for Removed and links. No behaviour change.
- **Desktop:** App no longer walks folder ancestry; when the core refuses to list the open folder (`NoteFolderRemoved`) the Notes view returns to All notes. A failed folder-tree reload after a restore is now shown instead of swallowed. `removedApi` has one shape, `remove/restore(kind, {id, revision})`.
- **New tests:** core `an_item_hidden_by_a_removed_container_cannot_be_removed_on_its_own`, `restoring_a_folder_brings_its_note_back_into_a_tasks_linked_notes`, `the_removed_view_can_be_searched_across_every_kind`; UI "searches Removed…", "leaves a subfolder's view when a folder above it is moved to Removed".
- **Evidence (run in the worktree, 2026-10-26, by the implementer):** core `cargo test` 109 passed (16 in `tests/removed.rs`); Tauri `cargo test` 1 passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean in both; `pnpm test` 159 passed in 15 files; `pnpm build` passed.
- **Out of scope here:** Removed through the CLI and MCP (stories 48–49) belongs to Tickets 10–12. Native GUI click-checklist above still open for the owner.
