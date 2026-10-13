# Ticket 04 — implementation plan before execution

**Date:** 2026-10-13. **Fixed source base:** `118e4240389fc7c96addf26241bab8a3dd5356b8` (`main` feature PR #1–3 merged). **Branch:** `feat/ticket-04-inbox-lists-tags-deadlines`, separate Mac worktree at `~/Desktop/Rumah/Playground/tasknotes-ticket04`. **Owner authorization:** user said "oke lanjutkan, tetap berdasarkan workflow /ask-matt" following recommendation for **Ticket 04**. Source implementation ONLY. No deploy, no live SQLite access, no merge until separately authorized.

## Problem definition

The existing Tasknotes v3 has local Tasks, status/priority/progress/history, but no Inbox/List grouping, tags or deadlines. Approved Ticket04 in `.scratch/tasknotes-v1/issues/04-inbox-lists-tags-deadlines.md` explicitly requires: Inbox or named List, All Tasks without duplicate, tag/due changes, overdue indicator, persistence/restart, referential integrity, deterministic local-date/timezone, bounded rows and /code-review. Q33 defaults to All Tasks with sidebar; Q36 specifies reversible List removal, hiding child Tasks without removing them. Ticket 05 (search) and Ticket 09 (Removed) are OUT OF SCOPE except for necessary referential/data foundations.

## Decisions and boundaries confirmed from approved product docs

- **Inbox is nullable `list_id`**, not a fabricated List or status. All Tasks is a query view, not a duplicate data collection. Named Lists have stable UUID and title; core rejects references to nonexistent/Removed Lists. Move-in or move-out is a single revision-checked Task mutation.
- **List removal is reversible** (`removed_at`), as approved Q36; removing a List hides its children from normal active views but does not alter Task work status/progress/history or delete any rows. A simple restore mechanism can be supported in the Ticket04 List manager; the full unified Removed experience belongs to Ticket09.
- **Tags are normalized** in SQLite `tags` + `task_tags` with composite key and FK. Task metadata changes replace the selected set transactionally; validate/de-duplicate values, no orphan associations. Ignore unassigned tags in task read results.
- **Deadline is a local calendar date** `YYYY-MM-DD`, not midnight UTC timestamp. `null` clears it. Overdue iff valid deadline date is strictly before the user's **local** date AND Task is not Completed. Today is **not** overdue. Do not add reminders or notifications. For deterministic tests, inject an explicit local-day value at the presentation seam (UTC offset/clock tests).
- **Scoped cursor queries** filter at SQLite before limit, never page All Tasks and then filter in JS. Status order In Progress→Ready→Completed is invariant. Bind pagination cursors to scope+sort to reject reuse across views; maintain backward compatibility for existing v2 all-task cursor.
- **No user data or QA experiments in Git.** Fresh temp SQLite v3→v4 migration must preserve existing records. Old binaries reject newer schema; **do not install/restart ordinary Tasknotes** until separate, explicit backup/deploy approval.

## Agreed public test seams (from approved Q48, prior discussion, user's continuation)

1. `TaskStore` Rust public interface against temporary SQLite: migration v3→v4, List create/rename/remove/restore, Task move/query, tag/due mutation, revision conflicts, invalid references, pagination, restart. This catches business/persistence failures, not native WebView rendering.
2. Typed Tauri IPC contract in React tests: All Tasks/Inbox/List navigation, List CRUD, tag/deadline edit, overdue states with injected local clock, pagination. This catches UI→core wiring, not OS-native click behavior.
3. **Opt-in native Tauri QA** on dedicated bundle/data paths if feasible without user-app interference, with synthetic fixture only. This catches launch/basic window behavior; no claim to real hardware FPS. No real data, browser controller focus stealing or Issue19 QA reintroduction.

## Tracer-bullet plan (sequential /tdd red→green, not speculative abstraction)

**S1 — List ownership foundation.** Write failing Rust contract test: migrate synthetic v3 DB to v4 and create two Lists, find them after restart, rename with optimistic revision guard and reject duplicate/blank names. Add minimal forward-only migration, `List` and `TaskStore` public methods. Run target test red then green. Preserve v1→v4 migration chain; ensure no changes to active user's DB.

**S2 — Inbox/All/selected List.** Failing public Rust test: new Task is in Inbox and All, moving to List under expected revision changes only List membership and does not duplicate; nonexistent/removed List rejected; scoped pages/ties stable and in fixed status order. Add `list_id` to Task, guarded move, SQL predicate before pagination, list-aware cursors. Add simple reversible List remove/restore (no physical deletes, children hidden but unchanged).

**S3 — Tags and deadlines.** Failing Rust contract: associate/dissociate tags, normalized uniqueness, invalid tags/date rejected atomically, clear deadline, restart and stale revision behavior. Add normalized tag tables, Task exposed `tags` and `due_date`, bounded batch hydration; list query pagination remains bounded.

**S4 — UI via typed Tauri.** Failing React test for sidebar view selection/create/move, List management and scoped create. Add Tauri thin adapters and typed taskApi methods. No duplicated Rust policy in React. Incremental TDD for due/tag edit, navigation state/paging, local calendar overdue/today/completed (deterministic time zone), and descriptive error handling. Maintain scroll/layout/accessibility and current component behavior.

**S5 — Verification and review.** Full React/Rust tests, typecheck, build, rustfmt/clippy (and Tauri Clippy if available), synthetic migration/negative controls; optional isolated unsigned native bundle and smoke without touching normal Tasknotes. Two-axis independent AGY Standards/Spec review at exact `main` SHA; factual cross-check and remediation only on demonstrated issues. Summarize evidence in ticket and docs; open Draft stacked on `main`, DO NOT merge/release. Write a /retro if tooling exposes a systemic oversight.

### Stop conditions

If a fundamental product choice is NOT covered by Q1–Q49, ask a single question instead of inventing it. If a test seam cannot observe the required behavior, do not declare acceptance from mocks. Preserve owner-running ordinary Tasknotes process, installed bundle and current data. Do not modify Issue19 QA or other repos.
