# 10: Won't Do

**What to build:** Mark a Task Won't Do (ADR 0008; spec story 4).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] A Task can be marked Won't Do from its status control; it moves to the Completed group with a distinct mark and keeps its progress. _Evidence: schema v18 `tasks.wont_do` (CHECK: only on Completed); core `marking_wont_do_completes_the_task_keeps_its_progress_and_records_a_progress_entry` (40% kept, revision +1, `task.change` audit), `a_query_filters_by_wont_do_and_keeps_it_in_the_completed_group`; UI `src/wont-do.test.tsx` ("Won't do" option sends `wontDo: true`; row gets struck title, "Won't do" label, cross in the neutral ring). Native GUI not checked._
- [x] Reopening clears Won't Do and returns it to Ready or In Progress by the usual progress rules. _Evidence: core `reopening_through_the_status_clears_wont_do_by_the_usual_progress_rules` (In Progress restores the kept %; Ready only at 0%), `clearing_wont_do_alone_reopens_to_ready_or_in_progress_by_progress`, `changing_progress_reopens_and_a_normal_complete_clears_wont_do`; UI "reopens a Won't Do Task through the status control"._
- [x] The transition is a Progress Entry; CLI and MCP can set, clear and filter Won't Do; Bulk Change status requests can include it. _Evidence: `progress_entries.previous_wont_do` / `wont_do` (v18); CLI `wont_do_is_set_cleared_and_filtered_through_the_cli` (`--wont-do true|false` on `task change` and `task list`, history fields), `a_bulk_status_request_can_mark_tasks_wont_do`; MCP `wont_do_is_set_cleared_and_filtered_through_mcp`, `a_bulk_status_request_can_mark_tasks_wont_do`; core `a_bulk_status_request_can_mark_wont_do_after_approval` (pending until approved; old requests without the field still read); exports: `exports_include_wont_do`._
- [x] Glossary and ADR 0008 agree with the behaviour. _Evidence: Completed keeps its progress only when Won't Do; Won't Do is a Completed resolution, not a fourth status (status enum unchanged, group order unchanged); reopening follows the usual progress rules. No edit to either was needed._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q1/Q15.**

**2026-11-09 — implementation (Draft PR).**
- Schema v18, forward-only and self-contained in `migrate_v18` (renumber if Ticket 01 lands first).
- Rules chosen where the ADR is silent: a finished Task (Completed at 100%) cannot be marked Won't Do until reopened (core refuses; the option is disabled in the window); marking an already Won't Do Task again is an empty change; a Won't Do Task at 0% may go back to Ready; clearing Won't Do alone reopens to Ready at 0% and In Progress otherwise; setting a Task aside plays no completion sound.
- Verified: `pnpm typecheck`; `pnpm test` 1086 passed + 4 todo (37 files); core 267, CLI 49, MCP 38, src-tauri 42 passed; e2e passed; `cargo fmt --check` clean in all four. Native GUI not checked.

