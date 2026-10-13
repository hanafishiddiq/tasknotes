# Ticket 03 — Two-axis code review

**Fixed point:** `bc8ac50` (Ticket 02 implementation).
**Diff:** `git diff bc8ac50...HEAD` after committing Ticket 03.
**Sources:** `AGENTS.md`, `GLOSSARY.md`, `docs/adr/0005-append-only-task-and-audit-history.md`, `docs/adr/0006-idle-tasks-remain-in-progress.md`, `.scratch/tasknotes-v1/issues/03-progress-history-next-step.md`, approved spec.

**Method:** Two independent manual passes across the *Standards* and *Spec* dimensions. This does not claim separate subagent or human reviewer approval.

## Standards

- **Pass — one business-logic owner:** The shared Rust `TaskStore` owns validations, status transitions, optimistic concurrency, Next Step persistence and event creation. `change_task` and `record_progress` share the same transactional mutation routine; there is no second status policy in React or Tauri.
- **Pass — durable local data:** Forward-only schema v3 migration adds `next_step`, indexed `progress_entries`, and an append-only sequence. All writes to Task and its Progress Entry occur in one immediate SQLite transaction. A deliberately failing SQLite trigger was tested and rolled back both writes.
- **Pass — modular UI:** `TaskProgressHistory` handles bounded, lazy-loaded history; `TaskRow` owns only expanded per-task drafts. No global history list is loaded. A single UI mutation helper preserves accurate write-success/read-refresh error reporting.
- **Pass — safety and privacy:** Test data is synthetic, local SQLite is outside the Git repository, and no permanent deletion or network listener was introduced. A private note cannot be leaked because the independent Notes/AI modules are out of scope.
- **Pass — quality checks:** 19 React tests, 20 Rust integration tests, TypeScript typecheck, strict Clippy for shared core and Tauri adapter, rustfmt, production frontend build and unsigned native ARM64 macOS bundle passed.
- **Pass — predictable history pagination:** history entries use a monotonic integer sequence, deterministic latest-first order, an index on Task/sequence, and hard page limits. Activity is requested only when expanded.
- **Observation (nonblocking):** `TaskRow` combines text editing, status controls, and progress journaling; it remains one deep, task-focused UI module for now. Reevaluate extraction only if additional task fields in future tickets create divergent reasons for change.
- **Observation (nonblocking):** The UI's typed command results do not yet expose the versioned AI envelope; this is explicitly scheduled for Ticket 10 and should not be introduced speculatively here.

## Spec

- **Pass — Next Step and dated notes:** User can edit Next Step and enter a Progress note without leaving Task details; a single `record_progress` request can save note, step and percentage, or just a note/step.
- **Pass — append-only Progress Entries:** Every substantive status/progress change through existing or new Rust operations inserts a dated event; unchanged priority-only edits do not invent progress history. History snapshots include prior/current status and percentage, note and Next Step.
- **Pass — completion/reopen:** 0, 1, 99, 100 and explicit reopen are tested; Completed always means 100%, reopening uses the previous incomplete percentage and returns In Progress; 0% on a Task that has started does not change it to Ready.
- **Pass — integrity and restart:** Existing schema v1 and v2 fixtures migrate safely to v3; Task revision, description, previous percentage and Next Step/history remain available after Rust core reopen. Invalid/oversized inputs and stale revisions leave Task and journal unchanged.
- **Pass — large-history UX:** latest 10 records loaded on expand, Load older requests are bounded/cursor-based. Failure shows retry, and a subsequent revision does not display stale entries.
- **Open — real macOS WebView GUI workflow:** Native startup and schema v3 creation were verified using a temporary synthetic database, but actual click/type/save/quit/relaunch in the WebView has not been witnessed; React command-contract tests do not replace this.
- **Open — stacked dependencies:** Tickets 01–02 still have Draft PRs and unverified native GUI acceptance. This ticket must be a Draft PR targeting Ticket 02 and cannot be merged into `main` until parent gates are satisfied.

## Merge danger

**Door:** One-way for storage upgrades. SQLite v3 is an additive forward-only schema; older Ticket 01/02 binaries refuse version 3. A recoverable backup is required before applying it to real user data. No live Task or Note data was deleted in this implementation.

**Blast radius:** Tasknotes local macOS desktop and its SQLite schema only. No Adamaja production repository, hosted service, credentials, network API or third-party audio assets were touched.

**Review conclusion:** Implementation and automated verification are ready for a stacked Draft PR. Ticket 03 stays **in review** until the native WebView acceptance workflow and parent PR dependencies are resolved.

## 2026-10-13 evidence correction (not independent review)

The old native behavior gap is covered by two actual WKWebView/SQLite note/Next Step/history tests across separate app processes. The failed manual note attempt remains unexplained. The implementing assistant reviewed the Standards and Spec axes again; **independent review was not obtained** because the Copilot reviewer returned a quota-exhausted comment. See [acceptance report](../qa/tickets-01-03-acceptance-reconciliation-2026-10-13.md). The Draft PR remains unmerged and the ticket stays in review.

## 2026-10-13 independent AI two-axis review

Two read-only, separate external AI agents reviewed the **pinned original feature diff** on Standards and Spec axes. See [source reports and evidence checks](tickets-01-03-independent-2026-10-13.md), including the independent factual correction of a false Spec blocker or stale native-evidence statement. This does **not** constitute an authenticated GitHub human approval and does not authorize merging.
