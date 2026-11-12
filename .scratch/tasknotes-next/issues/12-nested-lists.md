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

**2026-11-10 — owner decisions (Han), Q33/Q34, and review fixes (PR #32).**
- Q33: List names are unique, ignoring case, among siblings only, like Note Folders; the top-level Lists are siblings of each other, so "Adamaja IT › Reports" and "Personal › Reports" coexist. One core rule for create, rename, restore, move and drop; a move or drop beside a sibling of that name is refused with `name_conflict`, "A List with this name already exists here". Schema v21 swaps `lists_active_name` for `lists_sibling_name` (per parent); no row changes. Evidence: core `names_are_unique_among_siblings_ignoring_case`, the v20 → v21 upgrade test `upgrading_keeps_every_list_at_the_top_in_its_order_with_its_tasks`; CLI `list_names_are_unique_among_siblings_and_lists_are_named_by_id`; MCP `list_names_are_unique_among_siblings_and_tools_name_lists_by_id`; UI `says why a List cannot be dropped beside a List of the same name`.
- Name lookups: nothing resolves a List by name. The desktop, CLI `--list ID` / `--parent ID` and MCP `list_id` / `parent_id` take ids (a name given as an id is `not_found`, tested); quick capture never names a List. Where a person picks by name (the Task's List picker and label, the command palette) the window now shows the path; the palette also leaves out Lists inside a Removed List.
- Q34: Lists start collapsed, as Note Folders do; both remember the open ones the same way (`src/lib/expanded.ts`, ids under `tasknotes.lists.expanded` / `tasknotes.noteFolders.expanded`). Evidence: `start collapsed like Note Folders, and remember what the person opened after a restart`.
- New List inside: a row action on each List (none five levels deep; Inbox is no List row), a field under the row, Return creates through the new desktop command `create_sub_list`, Escape gives focus back. Four actions fit the overlay at 210 px (stylesheet test). Evidence: `creates a List inside another from its row, by pointer or keyboard`, `offers no List inside a List five levels deep`.
- Sidebar: the disclosure uses the core's `has_children`, so a List whose Lists are on the next page shows one, and opening it loads that page (tested); the rows filter reuses `listTree.up`; `moveSubtree` builds the tree once and keeps both parents' `hasChildren` current.
- Core: `reorder_list` is the one placement entry (`ReorderList.parent: ListParent`, `place_list` delegates); Lists and Note Folders read the row once in the write and return the decided audit action through `audited_as_decided`; `refused` is private. `ListParent::Current` is kept, with its reason in a comment: it keeps "no parent named means stay" in the core, resolved inside the write. Export: `Names` has its doc back, `Placed` its own.
- Bulk Change preview for a List removal lists and counts only what it would hide (nothing in Removed, nothing inside a Removed Sub-List); the sentence says "items" when Sub-Lists are among them. Evidence: core `the_approval_preview_counts_only_what_is_not_already_removed`, `src/approvals.test.tsx`.
- Verified as in `checks.yml`: core 295, CLI 56, MCP 41, bench 3, e2e 8, src-tauri 42 passed; `cargo fmt --check` and `cargo clippy --all-targets -D warnings` clean in all six; `qa-probe` check passes; `pnpm typecheck`, `pnpm build` and the probe-absent check pass. `pnpm test`: every file passes on its own, but full runs on a machine at load average 25–44 (other sessions compiling) failed 1–8 timing-bound tests, a different set each run (best: 1208 passed, 1 failed: the Notes heading after a lazy chunk load in `desktop-polish`); not compared against the base commit under the same load. Native GUI not checked.

**2026-11-12 — native check in the installed app** (the agent through Cua, owner-approved; dark theme; synthetic test Tasks and Lists in the owner's store, moved to Removed afterwards): "New List inside" on a List created a sub-List (CLI: its `parent_id` set, the parent's `has_children` true), shown indented under the parent with a collapse chevron.
