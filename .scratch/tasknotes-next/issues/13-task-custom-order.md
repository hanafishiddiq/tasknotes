# 13: Drag Tasks into a Custom Order

**What to build:** Drag to reorder Tasks in one List (spec stories 7–11).

**Blocked by:** 09 — Difficulty

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] In a single-List view, a Task can be dragged (and moved by keyboard) within its work-status group; dropping elsewhere does not change status. _Evidence: schema v20 `tasks.position` (one sequence per List or Inbox); core `reorder_task` after/before/first among the shown Tasks of the same group: `tests/custom_order.rs` `a_task_moves_after_another_before_another_or_first_within_its_group` (only the moved revision +1), `a_move_never_crosses_a_work_status_group_or_a_list` (anchor in another group, List or Removed = `revision_conflict`, status unchanged); UI `src/custom-order.test.tsx` "moves a Task with Alt+Up and Alt+Down…" (focus stays, live announcement, stops at the group edge) and "drags a Task within its group and refuses a drop in another group" (no `reorder_task`, no `change_task`). Native GUI not checked._
- [x] A drag that breaks the current sort switches that List to Custom, starting from the order on screen; changing priority through its control never switches to Custom. _Evidence: the desktop sends `shownSort`; the core seeds the whole List from that sort, moves, and remembers Custom in one transaction (a refused drop changes nothing): core `a_drag_under_another_sort_starts_custom_from_that_order_and_switches_the_list`; UI "switches the List to Custom when a drag breaks its sort…", "never switches to Custom when a priority changes through its control"._
- [x] Each List remembers its sort and its Custom Order; switching away and back restores it; a new Task lands at the top of its group in Custom. _Evidence: `view_sorts` table (v20, desktop-only `set_view_sort`, not audited): core `each_view_remembers_its_sort_across_restarts_and_only_the_desktop_sets_it`, `a_new_task_lands_at_the_top_of_its_group_in_custom_order`, `a_task_keeps_its_place_through_status_changes_and_lands_on_top_of_another_list`, `a_restored_task_comes_back_to_its_place`; UI "remembers each List's sort, also after a restart"._
- [x] Drag is off in search, tag filters and multi-List views. _Evidence: UI "keeps drag and Custom off in All tasks, and drag off in a search or tag filter" (no drag rows, Alt+Up does nothing, no Custom option in All tasks); core refuses Custom for All tasks (`custom_order_needs_one_list_or_inbox`). Choice: drag is off under any filter, not only search and tag (ADR 0010)._
- [x] CLI and MCP can list in Custom Order and move one Task after another with a revision check, without Bulk Change. _Evidence: CLI `task list --sort custom (--list ID | --inbox)`, `task reorder ID --revision N (--after ID | --first)`: `custom_order_lists_one_list_and_moves_a_task_after_another_with_a_revision`; MCP `task_list` sort `custom`, `task_reorder`: `tasks_read_and_move_in_custom_order_with_a_revision_check` (`task.reorder` Audit Entry, no Bulk Change); stale revisions audited: core `a_stale_revision_is_refused_and_recorded_and_an_applied_move_is_audited`. Neither changes the List's sort (ADR 0010)._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q5/Q6/Q16/Q17/Q31.**
