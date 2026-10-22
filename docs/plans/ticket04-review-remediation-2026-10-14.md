# Ticket 04 — independent review remediation plan

**Date:** 2026-10-14. **Source branch:** `feat/ticket-04-inbox-lists-tags-deadlines` at immutable reviewed head `c8be24c440c7b5612e3e47a3d45b01ec84bffc2b`, base `main` `118e4240389fc7c96addf26241bab8a3dd5356b8`. **Current PR:** #16 Draft. **Authorization:** the owner's active Ticket 04 implementation workflow and provision of the full external independent reviewer report. This is source remediation only; no merge, deployment, installation, real Tasknotes database, or alteration of normal macOS app.

## 1. Define the proven review gap

The external independent reviewer gave **REQUEST CHANGES** at pinned SHA `c8be24c` and reproduced three bugs using mock Tauri responses and clock controls:
- **S-01 / Major:** `nextTaskPage` can resolve after successful Task creation and replace the just-updated visible page.
- **S-02 / Major:** `createTask` captures an outdated `sort` and may refresh in Priority order after the user selects Newest. Also guard opposite response completion orders.
- **SPEC-01 / Major:** `today` is computed on React render only, not revalidated when a local calendar day changes, so the visual overdue badge stays stale after midnight.
All 30 React and 30 Rust baseline tests passed independently; these three cases are gaps in test coverage, not evidence that SQLite is generally broken. Native GUI interactive acceptance is still **NOT VERIFIED**; compiling an unsigned QA app does not prove clicks/restore/restart.

## 2. Breakdown and sequencing (plan before action)

**Public seams already approved in Ticket 04 plan:** typed React↔Tauri command boundary with synthetic Task/list pages and controllable promises; deterministic local-day presentation seam; Rust public `TaskStore::open` with temporary synthetic v3/v4 SQLite for migration faults.

1. **R1 RED:** permanent React test holds a Next-page Tauri reply, creates a Task, confirms it is shown, then releases the older page. Exact failure: new visible Task disappears despite successful save. Minimize and repeat before changing App.
2. **R1 GREEN:** coordinate page reads, refreshes and mutations with a single monotonically increasing query generation, compared before applying asynchronous results. Make read generation invalidation synchronous at navigation, sort and write success. Avoid creating a second copy of business rules.
3. **R2 RED/GREEN:** hold a create response while changing sort; assert last data read and visible Task order correspond to selected sort after create. Add a delayed old-sort/new-sort response inversion regression. Ensure active query metadata is read at completion time, not from stale closure. Reuse R1 query-generation logic rather than piling on ad-hoc disables.
4. **R3 RED/GREEN:** synthetic 23:59:58 local time with due-today Task (and Completed control) advances across midnight and demands Overdue transition with no other user interaction. Implement one small date-only clock hook scheduling the next local midnight (DST-compatible) and rechecking on focus/visibility/time-zone shift. Unit test existing IANA timezones remains a reference. No reminders/network.
5. **SQLite rollback:** reviewer's requested fault injection: synthetic schema v3 with an intentionally conflicting v4 `tags` table so migration errors *after prior DDL*. Assert `TaskStore::open` fails without a partial schema/v4 bump or Task-history loss, remove synthetic collision, then assert successful v4 and record preservation. No real DB access.
6. **Full regression and review:** `pnpm typecheck`, `pnpm test`, `pnpm build`, core `cargo test/fmt/clippy`, full Tauri clippy isolated; compare exact source diff to reviewed head, update QA report and ticket with corrected findings. If safe, isolated QA bundle may be rebuilt but no native full acceptance claim without actual UI interaction. Commit and push only to PR #16, retain Draft and no merge. External independent re-review on **new SHA** required before approval.

## 3. Failure boundaries

- No new library/dependency or unrelated refactor; no user app shutdown, synthetic data or screenshots committed.
- Do not convert source self-review into independent approval. Unknown reviewer-tool capacity is not a reason to fabricate status.
- Keep each RED test separate and observe failure at real React/Tauri seam; change only what makes the test GREEN before moving on.
- If reviewer-described repro does not reproduce locally, report that exact discrepancy before diagnosing.
- Preserve PR #16 base and source `main`, and record current SHA/test totals precisely.
