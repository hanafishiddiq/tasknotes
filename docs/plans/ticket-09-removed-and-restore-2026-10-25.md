# Ticket 09 — Removed and restore: plan (2026-10-25)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at 02b0d6c (Tickets 06, 07, 08 done; schema v8).

## Owner and coordinator decisions (from the ticket)

- Removed is a reversible disposition separate from work status (ADR-0002) for Tasks, Notes, Lists and Note Folders. Nothing is ever physically deleted and nothing is purged automatically.
- Removing a List or a Note Folder hides what is in it (a folder's whole subtree: subfolders and their Notes) without changing their own disposition. Restoring the container brings them back, except anything that had been moved to Removed on its own, which stays in Removed (owner, 2026-10-25).
- A Note's linked Tasks leave out Tasks hidden by Removed; the link row stays, so restoring brings the link back into view (Ticket 08).

## Decisions made by the implementer (owner may revisit)

- **A Task's linked Notes leave out Removed Notes** (their own Removed, or a Removed folder above them), symmetric with Ticket 08. The link row stays.
- **Restoring into a Removed container is refused**, never silent. A Task whose List is in Removed, or a Note or folder with a Removed folder above it, cannot be restored on its own: the core answers "restore the List / folder first". The Removed view shows such items with the reason and no Restore button. The alternative (restore into Inbox / top level) would change the item's place without asking.
- **Removed items are read-only.** Editing, moving, pinning, linking, or creating inside a Removed (or hidden) Task, Note, List or folder is refused until it is restored, so restore really brings back the state it was removed with, whatever other interface (CLI, MCP) tries meanwhile. Unlinking stays allowed: it only ends a relationship.
- **Revision checks:** remove and restore take the item's expected revision and bump it. For a Note the edit time (`updated_at`) is left alone, so restoring does not move it to the top of the list as if edited.
- **Folder names are unique among active siblings only,** as List names are among active Lists. Restoring a folder whose name is now taken by an active sibling is refused (rename the other first), like restoring a List.
- **One Removed view** (sidebar "Removed") lists every individually Removed Task, Note, List and folder, most recently removed first, in keyset pages of up to 100 (desktop asks for 50 and offers "Load more"). Items only hidden by a Removed container are not listed separately; they come back with it.

## Design

**Schema v9 (forward-only, one transaction):** `notes.removed_at`, `note_folders.removed_at`; the folder sibling-name index becomes partial (`WHERE removed_at IS NULL`); partial indexes on `removed_at` for tasks, notes, lists and folders feed the Removed view.

**Core interface (`removed.rs`, on the shared store):** `set_task_removed`, `set_note_removed`, `set_note_folder_removed` (plus the existing `set_list_removed`), each `{id, expected_revision, removed}`; `list_removed(limit, cursor)`. Hidden-folder checks use one recursive CTE over the (small) folder tree, so Note queries never walk ancestors per Note.

## Vertical TDD slices

1. Core: v8 → v9 migration (data intact, failed migration rolls back). Task remove/restore: hidden from views, searches and a Note's linked Tasks; restore recovers details, status, progress, links and history; revision check; restore under a Removed List refused; a Removed Task is read-only.
2. Core: Note remove/restore: hidden from All notes, folder views, search and a Task's linked Notes; restore recovers text and links; read-only while Removed.
3. Core: folder remove/restore: subtree hidden, individually Removed children stay Removed, restore under a Removed ancestor refused, active-sibling name rule.
4. Core: Removed view across kinds with bounded paging and restorability.
5. Tauri commands.
6. Desktop: Removed sidebar view with Restore; remove a Task (with Undo), the open Note, a folder.
7. Review and evidence: gates, owner click-checklist.
