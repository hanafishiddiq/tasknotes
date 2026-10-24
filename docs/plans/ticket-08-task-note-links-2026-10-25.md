# Ticket 08 — Reversible Task–Note links: plan (2026-10-25)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` after Ticket 06.

## Decisions made by the implementer (owner may revisit)

- **Unlink keeps the row.** A Note Link is a row in `task_note_links` with a stable ID. Unlinking sets `unlinked_at`; linking the same Task and Note again (including Undo) brings back the same row and ID. So there is one row per Task–Note pair, ever, and nothing is physically deleted.
- **Linking is idempotent.** Linking a pair that is already linked returns the existing link unchanged; no duplicate is possible (unique `(task_id, note_id)`).
- **Linking writes neither side.** A Task's or Note's revision and edit time do not change when it is linked or unlinked, so a link never causes a revision conflict in an open editor.
- **Order:** linked Notes and linked Tasks list most recently linked first, in keyset pages of up to 100 (desktop asks for 50 and offers "Load more").
- **Removed (Ticket 09) is not decided here.** Links survive whatever happens to either side; the lists return every linked item. Ticket 09 decides whether Removed items are hidden in these lists.
- **Desktop:** the expanded Task page gets a "Notes" section (open, unlink with Undo, link a Note from the recent Notes list); an open Note gets a "Tasks" strip (open, unlink with Undo, link a Task found by title search, which reuses Ticket 05 search). Opening a linked Task shows that one Task, expanded, with "Show all tasks" to leave.

## Design

**Schema v7 (forward-only, one transaction):**
`task_note_links(id, task_id → tasks, note_id → notes, linked_at, unlinked_at)`, `UNIQUE(task_id, note_id)`, both foreign keys `ON DELETE RESTRICT`, indexes for "Notes of a Task" and "Tasks of a Note" in link order. Notes folders, pins and search stay out (Ticket 07 takes v8).

**Core interface (`links.rs`, on the shared store):** `link_note(task_id, note_id)`, `unlink_note(link_id)`, `list_task_notes(task_id, limit, cursor)`, `list_note_tasks(note_id, limit, cursor)`. A missing Task or Note is refused (`Task not found`, `Note not found`); a foreign key backs the check for every other writer of the file.

## Vertical TDD slices

1. **Core links:** v6 → v7 migration (data intact, failed migration rolls back), link both ways many-to-many, no duplicates, unlink leaves Task and Note untouched, relink restores the same ID, missing references refused, foreign keys refuse dangling rows, survives reopen.
2. **Task → Note in the desktop:** Tauri commands, linked Notes on the Task page, link from recent Notes, open the Note, unlink with Undo.
3. **Note → Task in the desktop:** linked Tasks on an open Note, link by search, open the Task, unlink with Undo.
4. **Review and evidence:** `/code-review`, gates, owner click-checklist.
