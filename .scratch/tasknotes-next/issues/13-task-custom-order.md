# 13: Drag Tasks into a Custom Order

**What to build:** Drag to reorder Tasks in one List (spec stories 7–11).

**Blocked by:** 09 — Difficulty

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] In a single-List view, a Task can be dragged (and moved by keyboard) within its work-status group; dropping elsewhere does not change status. _Evidence: schema v20 `tasks.position` (one sequence per List or Inbox); core `reorder_task` after/before/first among the shown Tasks of the same group: `tests/custom_order.rs` `a_task_moves_after_another_before_another_or_first_within_its_group` (only the moved revision +1), `a_move_never_crosses_a_work_status_group_or_a_list` (anchor in another group, List or Removed = `revision_conflict`, status unchanged); UI `src/custom-order.test.tsx` "moves a Task with Alt+Up and Alt+Down…" (focus stays, live announcement, stops at the group edge) and "drags a Task within its group and refuses a drop in another group" (no `reorder_task`, no `change_task`). Native GUI not checked._
- [x] A drag that breaks the current sort switches that List to Custom, starting from the order on screen; changing priority through its control never switches to Custom. _Evidence: the desktop sends `shownSort`; the core seeds the whole List from that sort, moves, and remembers Custom in one transaction (a refused drop changes nothing): core `a_drag_under_another_sort_starts_custom_from_that_order_and_switches_the_list`; UI "switches the List to Custom when a drag breaks its sort…", "never switches to Custom when a priority changes through its control". Choosing Custom without a drag starts from the order on screen too (Q35): core `choosing_custom_starts_from_the_order_on_screen_then_keeps_that_order`; UI "starts Custom from the order on screen when chosen without a drag" (the page in Custom is asked for only after the core answered), "keeps the sort it had when Custom cannot be started". One definition of each sort's order serves the page and the rewrite: core `custom_starts_from_exactly_the_order_each_sort_lists` (every sort, by choosing and by a drag, with status, priority, Difficulty, tied creation times and a Removed Task)._
- [x] Each List remembers its sort and its Custom Order; switching away and back restores it; a new Task lands at the top of its group in Custom. _Evidence: `view_sorts` table (v20, desktop-only `set_view_sort`, not audited): core `each_view_remembers_its_sort_across_restarts_and_only_the_desktop_sets_it`, `a_new_task_lands_at_the_top_of_its_group_in_custom_order`, `a_task_keeps_its_place_through_status_changes_and_lands_on_top_of_another_list`, `a_restored_task_comes_back_to_its_place`; UI "remembers each List's sort, also after a restart". At launch the first page waits for the remembered sorts, so nothing reshuffles (PR #21): UI "opens a view by the sort it remembers, never by Priority first", "still opens, by Priority, when the remembered sorts cannot be read"._
- [x] Drag is off in search, tag filters and multi-List views. _Evidence: UI "keeps drag and Custom off in All tasks, and drag off in a search or tag filter" (no drag rows, Alt+Up does nothing, no Custom option in All tasks); core refuses Custom for All tasks (`custom_order_needs_one_list_or_inbox`). Drag is off under any filter, not only search and tag (Q36, ADR 0010); a try there (drag or Alt+Up/Down) shows "Turn off filters to reorder Tasks", or "Open one List to reorder Tasks" across Lists, for about 2 s in its own polite live region, with no focus change and nothing moving: UI "says quietly why a Task cannot move under a filter or across Lists", "the brief notice … shows about 2 seconds"._
- [x] CLI and MCP can list in Custom Order and move one Task after another with a revision check, without Bulk Change. _Evidence: CLI `task list --sort custom (--list ID | --inbox)`, `task reorder ID --revision N (--after ID | --first)`: `custom_order_lists_one_list_and_moves_a_task_after_another_with_a_revision`; MCP `task_list` sort `custom`, `task_reorder`: `tasks_read_and_move_in_custom_order_with_a_revision_check` (`task.reorder` Audit Entry, no Bulk Change); stale revisions audited: core `a_stale_revision_is_refused_and_recorded_and_an_applied_move_is_audited`. A move from either switches its List to Custom so the person sees it (Q37): core `a_move_from_any_interface_switches_its_list_to_custom` (a refused move switches nothing), the CLI and MCP tests read the List's sort after a move, the usage and the `task_reorder` description say so; the desktop follows on its next outside-change check: UI "shows the order an AI made when its move switches the List to Custom"._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q5/Q6/Q16/Q17/Q31.**

**2026-11-10 — owner decisions (Han), Q35/Q36/Q37, and review findings on PR #33.**
- Q35: choosing Custom from the sort menu, without a drag, starts the Custom Order from the order on screen, as a drag does (it showed Newest before). Applied to a List without a Custom Order of its own; a List that already has one gets it back, because spec story 9 promises that order back after looking by another sort. Recorded as a choice in ADR 0010; if Han wants every choice of Custom to start over from the screen, that is one condition in `set_view_sort`.
- Q36: drag stays off under any filter and across Lists; a try says why in a brief, quiet notice ("Turn off filters to reorder Tasks", "Open one List to reorder Tasks"), separate from the new Task toast.
- Q37: a move through the CLI or MCP switches its List to Custom, like a drag; core rule for every interface; CLI usage and MCP description updated.
- Findings: one ORDER BY definition (`search::ranked`, `search::order_by`) for the page and the rewrite, with a test per sort; the first page waits for the remembered sorts; `Sibling.active` is now `anchorable`; ADR 0010 says the upgrade's Newest-first positions are not the visible sort, and that sort changes and the rewrite are not audited and bump no revision (only the moved Task's); the rewrite of a 5,000-Task List takes 30 ms by choosing and 40 ms by a drag (debug; 16/20 ms release), so no index or batching was added.

**2026-11-12 — native check in the installed app** (the agent through Cua, owner-approved; dark theme; synthetic test Tasks and Lists in the owner's store, moved to Removed afterwards): dragging the lower Task by its title above the other (a real-pointer drag) reordered them within Ready and switched the sort from "Hardest first" to "Custom order".
