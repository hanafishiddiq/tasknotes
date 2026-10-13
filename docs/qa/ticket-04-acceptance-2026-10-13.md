# Tasknotes Ticket 04 — source acceptance and QA evidence

Date 2026-10-13. Branch `feat/ticket-04-inbox-lists-tags-deadlines`, baseline `118e4240389fc7c96addf26241bab8a3dd5356b8` (Ticket01–03 in `main`). No merge, no user SQLite access, no deployment.

## Features implemented

- SQLite **v4** migration extends existing v3 Task and append-only Progress Entries in place, adds persistent List ownership (`list_id` FK), tags and Task–Tag associations, calendar-only optional deadline. Old v1/v2/v3 upgrade paths remain validated with synthetic test DBs. Old binaries reject newer schema; backup is a future deployment prerequisite.
- Inbox is Task with NULL list_id. All Tasks is not a second stored record. Named Lists have UUID, rename and reversible removal/restore, including guarded duplicate names and optimistic revisions. Removing List hides its children from normal All/List views without changing Task disposition, status or history.
- Scoped SQLite keyset pages filtered **before LIMIT** (group order In Progress→Ready→Completed), with view/sort-bound cursors preventing cross-List leakage. Lists also have bounded cursor pages, with active/Removed inclusion bound to cursor identity.
- At most 16 normalized Tags per Task (length1–40; de-duplicated), one-transaction replacement, bounded batch hydration after Task reads. Strict real Gregorian YYYY-MM-DD validation with leap days. A deadline is *local calendar date*, no UTC midnight conversion or automatic reminder. Today's due is not overdue; Completed suppresses overdue.
- Native Tauri commands delegate to the same Rust `TaskStore`. Sidebar All Tasks/Inbox/List navigation; List create/rename/restore; Task create/move; Tag and Deadline editing. Date labels show Overdue, Due today, or the future local calendar date. Task title/progress drafts remain isolated from unfinished metadata edits.

## TDD (RED → GREEN, actual executed)

| Slice | Deliberately RED | Verified GREEN |
| --- | --- | --- |
| S1 List foundation | Rust unknown List structs/methods | Name uniqueness, create/rename/restart, reversible removal |
| S2 ownership/views | Rust missing Inbox/List/Move methods | Scoped query, exact List ownership, moved Task, stale revision, hidden/restore children, cursor binding |
| S3 tags/date | Rust missing UpdateTaskDetails | Strict tag/date validation, atomic updates, persisted tag hydration, no stale overwrite |
| S4 UI | React unable to find new List navigation and Task metadata controls | Real React component invokes typed mocked Tauri commands; scoped create/move, tag/date save, restoring Lists and dirty-form guards |
| Date helpers | Missing local-calendar helper | IANA offset examples (Los Angeles vs Jakarta), yesterday/today/future/completed |
| Later gap discovered | List-page cursor erroneously reusable with different Removed visibility | Cursor now binds include_removed mode; regression RED then GREEN |
| Upcoming deadline | Future due label missing from Task row | Future date rendered as local month/day without marking overdue |
| In-flight paging scope race | Inbox switch remained clickable while Load More was unresolved | Navigation blocked until scoped query resolves; pending-promise React regression red to green |
| Rejected List create | Create failure cleared the List name draft | Retain name on failure; clear only after Tauri write success, red to green React regression |

## Verified test totals

- **10 Rust Ticket04 organization tests**: includes v3→v4 preserving prior Task+Progress history, v1/v2 migration tests in existing suites, List CRUD/reopen, 135 Task + 20 neighboring Task bounded pagination (pages ≤50, no leak), metadata after title/status/move, date validation.
- **20 existing Rust tests**, for total **30 Rust integration tests**. Existing migration tests were updated from expecting v3 to current v4; no domain invariant was weakened.
- **9 new React/Vitest tests** (seven UI organization + two local-date helper), **19 existing React tests**, total **28 React tests** after the latest red→green UI corrections.
- `pnpm typecheck`, `pnpm build`, Rust fmt, core Clippy `-D warnings` and Tauri full-app Clippy `-D warnings` passed on the isolated Mac worktree.
- Native QA-only unsigned application: Tasknotes Ticket04 QA.app, bundle identifier store.adamaja.tasknotes.ticket04qa, separate Cargo output directory. Final size 31.72 MiB and native executable SHA256 ba76222eddb30a0fad1d74a7df4f9d9ab8864012589ecb423f270e0ae4e537b8. Strict Tauri Cargo Clippy passed.
- Native background-start smoke PASS: macOS launch services opened only the named QA app with an explicitly private synthetic TASKNOTES_DEV_DATA_DIR. SQLite user_version=4, integrity_check=ok, 0 Task records, and five expected domain tables validated using a read-only Python SQLite connection. The native sqlite3 -readonly CLI initially returned an open error; the separate Python read-only verification succeeded, so no false CLI PASS is claimed. QA process was stopped; normal Tasknotes app was preserved.
- Native interactive WebView CRUD and process restart NOT YET TESTED. A successful package/build/startup is not equivalent to full native acceptance or 60 FPS proof.

## Limits, privacy and release gate

These synthetic test seams demonstrate code behavior, **not real hardware FPS** or owner-operated native GUI acceptance. The original Tasknotes app is still the owner's accepted smooth version and was neither quit nor overwritten. No personal Task/Note/SQLite backups, synthetic DB binaries, temporary absolute paths, raw screenshots, browser sessions, or secrets are committed.

**Remaining:** independent Standards/Spec code review and actual native click/edit/restart acceptance if allowed by the native desktop controls. Keep Draft PR, no merge or deployment.
