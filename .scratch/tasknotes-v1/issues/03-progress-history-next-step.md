# 03: Progress history and Next Step

**What to build:** Record 0–100% progress with dated append-only Progress Entries and Next Step; view recent history on expanded Tasks; manually reopen Completed Tasks at the previous incomplete percentage. Test time ordering and transitions.

**Blocked by:** 02 — Work status, priority, and fixed ordering

**Status:** ready-for-agent

**Implementation:** review — native WKWebView automated create/edit/progress-note/Next Step/quit-reopen behavior passed twice; the earlier manual Progress Note omission remains unexplained. Independent review and parent Tickets 01–02 are still open.

## Acceptance criteria

- [x] A Task exposes editable progress 0–100%, Next Step, and dated progress notes without leaving the expanded Task. **Evidence:** two native WKWebView WebDriverIO runs drove the real UI and asserted actual Rust/SQLite persistence of the note and step.
- [x] Every substantive progress/status change creates an append-only dated Progress Entry; earlier history remains readable. **Evidence:** Rust tests for status/progress updates, note-only updates, recent history, paging, and immutable snapshots.
- [x] Completing a Task forces 100%, while an explicit reopen restores its previous incomplete percentage and returns to In Progress. **Evidence:** 0/1/99/100 transition table and prior incomplete progress tests.
- [x] Temporarily inactive Tasks retain In Progress status, progress, Next Step and history across application restarts. **Evidence:** Rust core reopen persistence and React Tauri-command contract remount tests.
- [x] Invalid percentages or updates that conflict with current status do not corrupt stored values or history. **Evidence:** invalid inputs, optimistic revision conflict, oversized text rejection, and SQLite synthetic journal-failure rollback test.

## Verification

- [x] Run core behavior tests for 0/1/99/100 transitions, reopen and history, plus React desktop-command persistence/reload tests.
- [x] Native UI entry/edit and app process restart through embedded WKWebView WebDriverIO, with read-only SQLite acceptance across two separate app processes. Dropdown selection uses the documented hidden-window fallback.
- [x] Relevant automated tests passed: 19 React, 20 Rust integration; TypeScript, Rust core Clippy, Tauri Clippy, Rust formatting, production frontend build, macOS unsigned app build, and isolated native SQLite schema v3 startup.
- [x] Two-axis Standards/Spec review documented in `docs/reviews/ticket-03.md`; no independent subagent approval claimed. Manual native GUI acceptance and parent dependency remain open.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

## Dependency handling

- Work is stacked above `feat/ticket-02-status-priority-order`, with PR base Ticket 02, not `main`.
- Native GUI interaction/restart acceptance of parent tickets remains unverified; do not claim parent completion or merge this ticket to main.

## Evidence — 2026-10-11

- **RED→GREEN:** missing `RecordProgress`/history API produced the expected red Rust tests; then they passed after the v3 migration and shared atomic mutation implementation. UI Next Step/Progress History tests failed first and later passed. Oversized whitespace note validation and simulated history-write failure were tested as negative controls.
- **Rust core:** `record_progress` and all pre-existing `change_task` writes route through one transaction so Task revision/progress and append-only event cannot diverge. Previous status/progress, new status/progress, note, Next Step snapshot, and timestamp are persisted; priority-only edits do not produce fake Progress Entries.
- **Pagination:** recent history loads only on expansion, with 10 entries by default. SQLite enforces limit 1–50 and keyset cursor by monotonically increasing sequence; timestamps are displayed in the local UI.
- **Migration:** v2→v3 preserves existing Tasks, revision, title, description, status, priority, and previous incomplete progress. v1→v2→v3 also passes. Old application binaries reject the newer schema, so obtain a safe backup before deployment.
- **UI:** Next Step and dated notes live in the expanded task editor. Percentage and note/Next Step can be saved atomically; note-only or Next Step-only entries work. Invalid/stale revisions keep draft text visible, history read failures expose retry, and multi-page history avoids loading entire logs.
- **Native:** ARM64 unsigned `Tasknotes.app` built (~31 MB), started with `TASKNOTES_DEV_DATA_DIR` set to an isolated synthetic temporary directory, and verified SQLite v3, task.next_step, progress_entries, index, zero personal records.
- **Gate:** Ticket 01 and Ticket 02 are still in Draft PRs. This ticket must remain stacked and unmerged; native GUI click/type/quit/reopen acceptance is still outstanding.

## Follow-up native QA (2026-10-12)

- User's actual native GUI operations persisted task title/description, priority Very High, progress 60%, Next Step and Completed→In Progress (60%) transition. Quit/restart and UI screenshot confirmed retention.
- **Progress Note remains unverified:** SQLite contains three history events with empty `note` fields even though the user replied they finished the test; do not infer the cause or mark note-entry acceptance complete. See [native QA evidence](../../../docs/qa/native-acceptance-2026-10-12.md).
- **UX performance bug:** user reports severe lag; tracked in [Bug 19](19-native-interaction-lag.md). Ticket 03 remains in review.

## Automated native QA — 2026-10-12

- Native E2E via embedded WKWebView WebDriverIO **passed twice** and SQLite confirms the exact dated Progress Note `Native acceptance passed`, Next Step, four append-only entries and persistence after a separate app process relaunch. The manual 2026-10-12 test had an absent note; the automated test demonstrates that this field can be saved but does not prove why that prior attempt omitted it. See [native WebDriver evidence](../../../docs/qa/native-webdriver-results-2026-10-12.md). Independent review is still pending; ticket remains Review.

## Acceptance reconciliation — 2026-10-13

The manual 2026-10-12 test did not persist the expected Progress Note; two later WKWebView runs did persist the exact note, Next Step and four immutable entries across a separate app process. This is valid automated native evidence; no claim is made about why the earlier manual note was empty. Independent review and parent dependencies remain open; **Implementation stays review**. See [joint evidence matrix](../../../docs/qa/tickets-01-03-acceptance-reconciliation-2026-10-13.md). No native QA rerun, production data access, merge or release was performed.
