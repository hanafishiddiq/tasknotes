# Tasknotes native macOS acceptance — 2026-10-12

**Target:** PR #1 → PR #2 → PR #3 stacked Draft branches (latest synthetic debug app: `c083c7d`).
**Method:** User manually operated native Tasknotes; assistant inspected only the previously created temporary database referenced by the ignored `src-tauri/target/.tasknotes-acceptance-data-dir` marker. No real personal user database was opened or committed.

## Evidence table

| Native acceptance check | Result | Evidence |
| --- | --- | --- |
| Native Tauri macOS window launches | PASS | macOS window/screenshot observed |
| Create Task with synthetic title | PASS | Exactly one row persisted |
| Edit title and description | PASS | `QA-NATIVE-EDITED`, `Edited via native GUI` present |
| Set Very High priority | PASS | `priority=very_high` |
| Set 60% progress and In Progress | PASS | `progress=60`, `status=in_progress` |
| Complete then reopen to 60% | PASS | Journal entries include 60 → 100 → 60 and Completed → In Progress |
| Save Next Step | PASS | Stored `Verify Restart` (capitalization differs only from test prompt) |
| Save Progress Note `Native acceptance passed` | NOT VERIFIED | All three journal entries have empty `note`; user may have omitted or not saved the field. Do not infer a product defect without reproduction |
| Quit native app and reopen with same database | PASS | Native process stopped; subsequent process/window launched; UI screenshot shows one In Progress Very High Task at 60%; SQLite persisted revision 9, history count 3 |
| SQLite integrity | PASS | `PRAGMA integrity_check = ok` |
| Independent review per Matt Pocock | BLOCKED | GitHub Copilot review requests returned a quota-exhausted comment on PRs #1, #2 and #3; existing reviews are manual self-reviews, not independent |
| Smooth UX | FAIL — user report | User describes very severe lag during native interactions; no measured repro yet. See Bug #19 |

## Safety and decision

- Database is synthetic and isolated in a temporary directory under system `TMPDIR`. Never commit its raw bytes, machine absolute path, Task UUID or hidden caches.
- Native GUI basic CRUD/status/progress/reopen acceptance now has **direct user action + persisted SQLite + screenshot evidence**.
- Ticket #03's progress-note entry remains an **unverified** acceptance criterion.
- **Merging PR #3** remains gated on the missing progress-note acceptance proof and independent Standards/Spec review. Bug #19 has been deliberately scheduled for diagnosis/fix before Ticket 17/18 performance/release gates; the user requested future improvement, so it is **not automatically a blocker for merging a documented early feature PR** if its own criteria pass and reviewers approve. Green CI is necessary but not sufficient.
- Before any schema migration on personal data, create and validate a recoverable backup; older v1/v2 binaries reject SQLite v3.

## Reviewer guidance

Use `git diff e4c728c...b799cce` for Ticket 01; `git diff b799cce...bc8ac50` for Ticket 02; `git diff bc8ac50...c083c7d` for Ticket 03. Compare both documented Standards and the ticket's Spec, and report findings separately.

## Later autonomous QA resolution (2026-10-12)

The original manual test's missing Progress Note is a **historical observation**, not evidence that the app cannot persist notes. The new [native WKWebView WebDriverIO harness](native-webdriver-e2e.md) ran **twice** using new synthetic SQLite databases, with both native app sessions passing and a note of exactly `Native acceptance passed` found in the actual `progress_entries` rows. The original manual test was not overwritten or reinterpreted; its cause remains unknown.

Automation included a test-driver workaround: a hidden native `select` did not respond to WebDriver's `selectByAttribute`, so the runner dispatched a scoped input/change event inside Tasknotes' own WKWebView and verified Rust/SQLite effects. No unrestricted OS-level input was used. See [results](native-webdriver-results-2026-10-12.md).

**The user should not be required to repeat manual CRUD/progress/restart testing.** Performance Bug #19 and independent Standards/Spec code review remain pending. No PR has been merged.
