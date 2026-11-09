# Tasks keep a Custom Order per List

Status: accepted (2026-11-09, Ticket 13, owner decisions Q5/Q6/Q16/Q17/Q31)

A person drags Tasks within a work-status group of one List (Inbox included) to give them a **Custom Order**. It reuses ADR 0007's sibling positions and its "after a named sibling, or first" write, so the desktop, the CLI and MCP share one rule.

**Storage (schema v20).** Each Task has an integer `position`, one sequence per List (or Inbox), whatever its status. Sorted by Custom, a List shows In Progress → Ready → Completed as always, each group in that sequence. So a status change never needs a write of its own: the Task shows in its new group at the place it already had in the List. Upgrading starts every List Newest first, where a new Task lands, and changes no revision. A new Task, or one moved in from another List, takes a position below every other in its List, so it lands at the top of its group.

**The write.** `reorder_task` places one Task directly after a Task, directly before one, or first, always among the shown Tasks of its own group. An anchor that is gone, in Removed, in another group or another List is `revision_conflict`; nothing then moves and no status changes. Only the moved Task's revision is checked and bumped, and it leaves a `task.reorder` Audit Entry. Renumbering starts at the lowest position the List already has, so a move rewrites only the Tasks it passes. It is one Task, so it never goes through Bulk Change approval.

**Switching to Custom.** The desktop says which sort it was showing. When that is Priority, Newest or Difficulty, the core first rewrites the List's Custom Order to that order, all of it and not only the loaded page, then moves the Task and remembers Custom for the List, in one transaction. A drop that would change nothing is refused and changes nothing, not even the sort. Changing a priority never goes through this write, so it only re-sorts.

**Remembered sorts.** Each view (All tasks, Inbox, each List) remembers its sort in the core (`view_sorts`), so it survives restarts. Only the desktop sets it: it is how the person looks at their Tasks, so it is not audited. All tasks cannot be Custom, since positions mean nothing across Lists.

**Surprising choices.**
- The CLI and MCP read the Custom Order (`--sort custom`, sort `custom`) and move within it, but they never change a List's sort. The person keeps the sort they chose; if a List is not on Custom, an AI's move shows when the person switches to Custom.
- The desktop also places a Task *before* a row, because its pages replace one another: on a later page the Task before the top row is not loaded. The CLI and MCP offer after and first only, like the sidebar.
- Dragging is off whenever any search or filter is active, not only search and tag filters: a filtered page hides part of a group, and the person should see the whole group they arrange.
- Exports carry each Task's `position`; the format version stays.

**Known limits.** A move happens within the page on screen; a Task cannot be dragged to another page of the same List in one step. Switching to Custom from a later page shows the first page again, since the earlier sort's cursor means nothing in the new order. A move rewrites the positions of the Tasks it passes, and switching from another sort rewrites every Task of the List, in one transaction. Neither was measured on a very large List.
