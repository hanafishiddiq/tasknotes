# 09: Difficulty on Tasks

**What to build:** Optional Difficulty beside Priority (spec stories 5–6).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] A Task has Difficulty Very Easy … Very Hard or No Difficulty; existing and new Tasks start with No Difficulty. _Evidence: schema v17 nullable `tasks.difficulty` with a CHECK; `tests/difficulty.rs` `a_new_task_has_no_difficulty…`, `upgrading_from_v16_gives_every_task_no_difficulty…`._
- [x] The row shows a Difficulty pill only when set; the editor can set and clear it. _Evidence: `src/difficulty.test.tsx` (pill only when set; Difficulty select with "No difficulty" sends `difficulty: null`). Native GUI not checked._
- [x] Filter by Difficulty and sort by Difficulty (Very Hard first, No Difficulty last) work in the UI, CLI and MCP. _Evidence: core `sorting_by_difficulty…` (pages of 1/2/100), `filtering_by_difficulty…`; CLI `difficulty_is_set_cleared_filtered_and_sorted_through_the_cli`; MCP `difficulty_is_set_cleared_filtered_and_sorted_through_mcp`; UI "Hardest first" and "Filter by difficulty"._
- [x] Changing Difficulty is a revisioned change recorded in audit history; exports include it. _Evidence: `setting_and_clearing_difficulty_is_a_revisioned_audited_change` (revision +1, `task.change` Audit Entries, stale refusal audited), `exports_include_difficulty` (JSON `difficulty`, Markdown "Difficulty: Very Hard")._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q4/Q23.**
