# Ticket 04 — independent REQUEST CHANGES remediation

Date: 2026-10-14. Reviewed head: c8be24c440c7b5612e3e47a3d45b01ec84bffc2b. Base: 118e4240389fc7c96addf26241bab8a3dd5356b8. Pull request: #16 Draft. **Origin:** complete independent reviewer report supplied by the owner on 2026-10-14. This document is implementing-agent response, NOT independent approval.

## S-01 — delayed pagination replaces newly saved Task

The reviewer reproduced a stale Next-page response replacing a newly created visible Task. New React/Tauri R1 test reproduced that behavior (RED), then passed (GREEN) after versioning async queries through one monotonically increasing request generation. A second R1b test similarly went RED then GREEN for moving a Task to a List while an old page was pending. Initial reads, Next and Previous, Task create, Task metadata/progress mutations, List navigation and changes of sort now compare their captured generation before committing async query results.

## S-02 — create uses stale sort closure

R2 test delayed Task creation while switching Priority to Newest. Previously a Priority refresh overwrote the new query, leaving rows inconsistent with selected sort (RED). The fix maintains a live current query reference (view/sort) updated in event handlers. Mutations refresh the latest query, and stale responses are discarded by the same request-generation rule (GREEN). R2b held the prior sort request while the newer create refresh completed, then released the old request in reverse order. It exposed a permanently visible loading state (RED), corrected by completing loading only for the latest generation (GREEN).

## SPEC-01 — overdue does not update across midnight

R3 set local time to 23:59:58 and moved past midnight without other UI interaction. Prior code kept Due today despite the Task becoming overdue (RED). New useLocalCalendarDay hook updates on the next local midnight, rechecks at most every 60 seconds for timezone change, and refreshes immediately on focus / visibility regain. R3 and R3b (suspended app clock jump then focus) are GREEN. Completed Task control remained free of Overdue. Existing IANA local-day tests for Jakarta and Los Angeles remain valid. No reminder, notification or network behavior was introduced.

## Reviewer-requested synthetic migration failure injection

A temporary SQLite v3 fixture is intentionally given a conflicting tags table, causing v4 migration failure after earlier List DDL and Task schema ALTER commands. TaskStore::open rejects the migration. Test asserts schema user_version remains 3, prior List DDL and new Task columns are rolled back, and Task/immutable Progress Note remain intact. Removing only the synthetic collision allows v4 retry with the records preserved. No personal SQLite was opened.

## Evidence and remaining approval gates

Full React / Rust tests, typecheck, frontend build, fmt / core Clippy and isolated Tauri build verification are recorded separately in the canonical QA report after actually completing those checks. This record is not proof of GitHub CI on the new SHA.

**NOT YET APPROVED:** external independent re-review of the repaired PR head, native GUI actual click/edit/restore/restart acceptance, real-user-data backup/restore, 50k/10k performance, macOS release-quality tests.

All changes remain in the Ticket 04 feature branch. No merge or deployment; ordinary Tasknotes app and user SQLite remain unchanged. Reviewer re-review must use the NEW head after the remediation commit, not c8be24c.
