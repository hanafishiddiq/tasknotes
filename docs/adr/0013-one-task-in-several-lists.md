# One Task can be shown in several Lists

Status: accepted (2026-11-18, batch 4)

The owner asked that a Task can appear in more than one List, identical in each: completing it in one completes it everywhere. In a List it shares, Remove offers "this List only" or "every List" (Move to Removed). It is added from the Task menu ("Also show in"), from the open Task (an "Also show in…" choice beside its List, with the others shown as chips that take it out again), or by dragging it onto a List in the sidebar while holding Option. A plain drag still only reorders.

A Task keeps one List of its own (`tasks.list_id`, or Inbox), which every existing rule keeps reading: moving, Removed Lists hiding their Tasks (ADR 0009), export. The other Lists are rows of `task_list_links` (schema v27), one per pair, never deleted: taking a Task out of a List it is also shown in marks its row, and showing it there again brings the row back with its place. It is one Task, so a status, progress or any other change shows in every List at once, and every interface reads `also_in` beside `list_id`.

Each List keeps its own Custom Order (ADR 0010) for a Task it shares (owner choice): its own List by `tasks.position`, another by the link's `position`, numbered as one sequence with that List's own Tasks, so a drag there moves it in that List alone. A List that shows no other List's Task keeps the plain query its index serves; only a List that does reads the combined one.

Taking a Task out of its own List makes the List it was first also shown in its own, at the place it had there. A Task in one List only is not taken out: Move to Removed is for that. Moving a Task into a List it is also shown in makes that its own List and keeps its place there. Showing and taking out change the Task: they name its revision, write a new one and are audited (`task.show_in`, `task.hide_from`). The sidebar counts a shared Task in each of its Lists and once in All tasks.

A List in Removed (or inside one) is left out of a Task's `also_in` until it is restored: the Task shows no chip for it, is never moved into it when it leaves its own List, and a Task whose only other List is in Removed counts as being in one List. Removing a List leaves its links as they are, so restoring it shows those Tasks there again; a List that only shows other Lists' Tasks counts as empty for the approval an AI interface needs to remove it, since nothing in it is hidden or lost.
