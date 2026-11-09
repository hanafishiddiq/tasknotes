# 12: Lists inside Lists

**What to build:** Lists can contain Lists, five levels deep (ADR 0009; spec story 12).

**Blocked by:** 04 (manual sidebar order, PR #24)

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] A List can be dragged into or out of another List in the sidebar (and moved by keyboard as in PR #24); depth above 5 is refused.
  Evidence: `src/nested-lists.test.tsx` (drop into / out, Alt+→/←, never inside itself, refusal shown); core `tests/nested_lists.rs` `lists_nest_five_levels_at_most_by_the_folder_rule`, `a_list_cannot_land_inside_itself`.
- [x] A List shows only its own Tasks; a new sub-List starts empty.
  Evidence: core `a_sub_list_starts_empty_and_each_list_shows_only_its_own_tasks`.
- [x] Inbox cannot be moved into a List and cannot hold Lists.
  Evidence: core `inbox_stays_on_top_and_holds_no_lists` (Inbox is no List row; `inbox`, a folder id refused as `not_found`).
- [x] Removing a List hides the Lists and Tasks inside it until it is restored; through AI it needs Bulk Change approval when it holds anything.
  Evidence: core `removing_a_list_hides_the_lists_and_tasks_inside_until_it_comes_back`, `a_list_removed_inside_a_removed_list_waits_for_it`, `through_ai_removing_a_list_that_holds_a_list_needs_approval` (preview lists Sub-Lists and nested Tasks); CLI/MCP `needs_approval`; `src/approvals.test.tsx`.
- [x] CLI and MCP can create a List inside another, move a List and list the tree; exports keep the nesting.
  Evidence: `tasknotes-cli/tests/nested_lists.rs`, `tasknotes-mcp/tests/crud.rs::lists_nest_move_and_read_as_a_tree`, core `exports_keep_the_nesting` (JSON `parent_id`, Markdown `Tasks/Work/Reports.md`).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q7/Q8/Q18/Q24.**
