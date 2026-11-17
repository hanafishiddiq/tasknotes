# Subtasks are a checklist inside a Task, not Tasks

Status: accepted (2026-11-18, batch 4)

The owner asked for Subtasks and chose a simple checklist: each item is a text that is done or not, with a very generous length (10,000 characters) so a thought is never cut short. We considered making each Subtask a full Task (status, priority, notes, progress, indented under its parent), but that adds a hierarchy to every Task query, ordering rule and interface for little gain, and crowds the list.

Subtasks therefore live in their own table, `task_subtasks` (schema v25), owned by one Task and ordered by `position`; a Task holds at most 500. The Task carries `subtasks_done` and `subtasks_total`, counted when it is read, and the window shows them as "2/5" on its row. Ticking items never changes the Task's Progress or work status (owner decision: the percentage stays manual), so the rules of ADR 0006 are untouched.

Every interface writes through the core (ADR 0001): add, edit, tick, move, remove and restore, each audited (`subtask.*`) with a revision check. Removing marks the item, as unlinking a Note Link does; nothing is deleted, restoring returns it to its place, and it never appears in the Removed view. While its Task or the Task's List is Removed, the checklist is kept and takes no changes. Subtasks are Task data, never Note text, so Exclude from AI does not apply to them. JSON export includes every item (Removed ones marked); Markdown lists the shown ones under "Subtasks".
