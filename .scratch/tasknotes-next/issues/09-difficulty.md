# 09: Difficulty on Tasks

**What to build:** Optional Difficulty beside Priority (spec stories 5–6).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — PR #29 merged 2026-11-09; the native check waits for the single batch-2 build (Ticket 15)

## Acceptance criteria

- [x] A Task has Difficulty Very Easy … Very Hard or No Difficulty; existing and new Tasks start with No Difficulty. _Evidence: schema v17 nullable `tasks.difficulty` with a CHECK; `tests/difficulty.rs` `a_new_task_has_no_difficulty…`, `upgrading_from_v16_gives_every_task_no_difficulty…`._
- [x] The row shows a Difficulty pill only when set; the editor can set and clear it. _Evidence: `src/difficulty.test.tsx` (pill only when set; Difficulty select with "No difficulty" sends `difficulty: null`). Native GUI not checked._
- [x] Filter by Difficulty and sort by Difficulty (Very Hard first, No Difficulty last) work in the UI, CLI and MCP. _Evidence: core `sorting_by_difficulty…` (pages of 1/2/100), `filtering_by_difficulty…`; CLI `difficulty_is_set_cleared_filtered_and_sorted_through_the_cli`; MCP `difficulty_is_set_cleared_filtered_and_sorted_through_mcp`; UI "Hardest first" and "Filter by difficulty". No Difficulty is filterable too (`DifficultyFilter::NoDifficulty`, CLI/MCP `none`, UI "No difficulty"; "Any difficulty" stays no filter): core `filtering_by_no_difficulty_keeps_only_tasks_without_one_and_any_keeps_all`, CLI and MCP difficulty tests, UI "filters to Tasks with No Difficulty…"._
- [x] Changing Difficulty is a revisioned change recorded in audit history; exports include it. _Evidence: `setting_and_clearing_difficulty_is_a_revisioned_audited_change` (revision +1, `task.change` Audit Entries, stale refusal audited), `exports_include_difficulty` (JSON `difficulty`, Markdown "Difficulty: Very Hard")._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q4/Q23.**

**2026-11-09 — review fixes on PR #29.**
- No Difficulty filter in core, CLI (`task list --difficulty none`), MCP (`task_list` `difficulty: "none"`) and the window ("No difficulty"); usage and tool text updated (`1864c35`).
- One public `tasknotes_core::tasks::present`; MCP re-exports it (`5e53f3f`).
- CLI `--difficulty` checks `none`, then uses `p.choice` (`fac0752`).
- Unit test pins `DIFFICULTY_RANK` to `Difficulty::rank` and the v17 index text, and EXPLAIN QUERY PLAN shows the Difficulty sort uses `tasks_status_difficulty_order` with no temp B-tree; a one-value drift fails it (`caa70f2`).
- `read_task` reads `difficulty` and the trailing column through `DIFFICULTY_COLUMN` / `COLUMN_COUNT`, guarded by a unit test against `COLUMNS` (no name-based pattern exists in the codebase) (`7918e5b`).
- No Difficulty in the editor shows a dashed square, not Very easy's `SignalZero`; DESIGN.md updated (`6fd8981`).
- Upgrade tests in `tests/search.rs` and `tests/organization.rs` assert upgraded Tasks have NULL Difficulty (`b26694e`).
- Verified: `pnpm typecheck`; `pnpm test` 1047 passed + 4 todo (34 files); core 252, CLI 47, MCP 36, src-tauri 42 passed; `cargo fmt --check` clean in all four. Native GUI not checked.

**2026-11-12 — native check in the installed app** (the agent through Cua, owner-approved; dark theme; synthetic test Tasks and Lists in the owner's store, moved to Removed afterwards): Hard and Easy pills showed on the rows; "Hardest first" put the Hard Task above the Easy one, and swapping their Difficulty swapped the order (the CLI's `--sort difficulty` agreed).
