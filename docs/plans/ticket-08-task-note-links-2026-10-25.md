# Ticket 08 — Reversible Task–Note links: plan (2026-10-25)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` after Ticket 06.

## Decisions made by the implementer (owner may revisit)

- **Unlink keeps the row.** A Note Link is a row in `task_note_links` with a stable ID. Unlinking sets `unlinked_at`; linking the same Task and Note again (including Undo) brings back the same row and ID. So there is one row per Task–Note pair, ever, and nothing is physically deleted.
- **Linking is idempotent.** Linking a pair that is already linked returns the existing link unchanged; no duplicate is possible (unique `(task_id, note_id)`). Linking an unlinked pair again reuses its row with a fresh stamp, so it lists first; Undo (`restore_link`) keeps the original stamp and place (review fix).
- **Linking writes neither side.** A Task's or Note's revision and edit time do not change when it is linked or unlinked, so a link never causes a revision conflict in an open editor.
- **Order:** linked Notes and linked Tasks list most recently linked first, in keyset pages of up to 100 (desktop asks for 50 and offers "Load more").
- **Removed:** links survive whatever happens to either side. Coordinator decision at review: a Note's linked Tasks leave out Tasks hidden by Removed (their own or their List's), and the link row stays. The rest is Ticket 09.
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

## Follow-ups

- **Stale Undo (Tickets 10/12):** Undo after an unlink calls `restore_link` on that link ID. If another interface (CLI, MCP) ended or re-made the link meanwhile, a late Undo in the desktop can bring back a link that interface ended. Acceptable for a single desktop today; when CLI/MCP writers arrive, give links a revision (or check `unlinked_at` against the value the Undo saw) and record it in the Audit Entry.

