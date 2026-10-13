# Ticket 02 — Two-axis review

**Fixed point:** `b799cce` (`feat/ticket-01-first-task`).
**Diff:** `git diff b799cce...HEAD` after committing Ticket 02.
**Sources:** `AGENTS.md`, `GLOSSARY.md`, `docs/adr/0006-idle-tasks-remain-in-progress.md`, `.scratch/tasknotes-v1/issues/02-work-status-priority-fixed-order.md`, `.scratch/tasknotes-v1/spec.md`.
**Review method:** Two separate manual passes for Standards and Spec. No claim that independent subagents or a human reviewer have approved the PR.

## Standards

1. **Pass — domain ownership:** `tasknotes-core` owns status/progress invariants, validation, migration, sorting and optimistic concurrency. React sends typed commands but does not reimplement transition rules. The Tauri command adapter delegates to Rust.
2. **Pass — deep modules:** `TaskRow` was extracted into `src/components/TaskRow.tsx` and priority/status display metadata into `src/lib/task-visuals.ts`, avoiding one large monolithic React component for future tickets.
3. **Pass — persistence and concurrency:** `change_task` uses SQLite's IMMEDIATE transaction and revision check, with error return and no implicit removal/reset of user data. Migration v1→v2 is transactional; old progress is preserved by a targeted migration.
4. **Pass — test seams:** public Rust `TaskStore` integration tests against temporary SQLite and React tests at the Tauri command seam. Tie ordering, invalid transitions, existing-data migration and write-success/refresh-error have dedicated checks.
5. **Pass — security:** no new web server, network permission, user data export, destructive deletion, cloud sync or uncontrolled external assets. Tests use synthetic data only.
6. **Pass — visual accessibility:** priority and status badges each have explicit text and redundant icons; foreground colors were darkened for WCAG AA normal-text contrast (individual pill colors approximately 4.8:1–6.3:1 over their respective backgrounds).
7. **Maintainability observation (nonblocking):** `App.tsx` has repeated first-page refresh state assignments in create/change/load. If complexity grows with Ticket 03+, consolidate these as a UI data-loading boundary; do not add a speculative abstraction now.
8. **Runtime QA limitation (not a code-standard failure):** real native WebView controls were not addressable by the existing macOS accessibility automation, so native GUI click/type has not been verified.

## Spec

1. **Pass — statuses:** Ready, In Progress and Completed are supported through UI controls and the Rust core; idle unfinished Tasks remain In Progress, even with 0% progress after having started.
2. **Pass — priorities:** very low, low, medium, high and very high, with distinct icons, labels, and compliant contrast. Medium remains the default for newly created Tasks.
3. **Pass — invariants:** 1–99% on Ready starts work; 100% completes; explicit reopen restores previously incomplete progress. Contradictory status/progress pairs are rejected, and Ready cannot silently reset already-started work.
4. **Pass — fixed ordering:** group sort is always In Progress → Ready → Completed, with secondary Priority or Newest and deterministic cursor pagination, including ties. Sorting is implemented in the SQLite-backed Rust core, not as a frontend-only reorder.
5. **Pass — v1 data:** migrations preserve existing Task records and their incomplete progress and are tested with realistic synthetic preexisting v1 data.
6. **Open — native GUI acceptance:** no witnessed real WebView create/priority/status/progress/restart journey. Automated React/Tauri contract tests and native app startup do not substitute for this end-to-end gate.
7. **Open — parent dependency:** Ticket 01 still has incomplete manual native GUI acceptance. Ticket 02 must remain stacked in a Draft PR based on Ticket 01's branch; no direct merge into `main`.

## Merge danger

**One-way for storage schema:** the v1→v2 SQLite migration is forward-only; a Ticket 01-era binary rejects schema v2. Before any deployment, preserve a backup and confirm Ticket 01's native acceptance. No deletion of user data occurs as part of this ticket.

**Review conclusion:** automated acceptance and native debug packaging are verified; the remaining items concern manual native GUI validation and the prerequisite parent ticket. Keep Ticket 02 in review and its PR Draft until those gates are cleared.

## 2026-10-13 evidence correction (not independent review)

The old native behavior gap was covered by owner-operated status/priority/reopen and native WDIO sorting through scoped DOM change events; the OS picker itself remains unverified. The implementing assistant reviewed the Standards and Spec axes again; **independent review was not obtained** because the Copilot reviewer returned a quota-exhausted comment. See [acceptance report](../qa/tickets-01-03-acceptance-reconciliation-2026-10-13.md). The Draft PR remains unmerged and the ticket stays in review.
