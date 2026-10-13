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
- **11 new React/Vitest tests** (nine UI organization + two local-date helper), **19 existing React tests**, total **30 React tests** after the final red→green fixes.
- `pnpm typecheck`, `pnpm build`, Rust fmt, core Clippy `-D warnings` and Tauri full-app Clippy `-D warnings` passed on the isolated Mac worktree.
- Native QA-only unsigned application: Tasknotes Ticket04 QA.app, bundle identifier store.adamaja.tasknotes.ticket04qa, separate Cargo output directory. Final size 31.72 MiB and native executable SHA256 ba76222eddb30a0fad1d74a7df4f9d9ab8864012589ecb423f270e0ae4e537b8. Strict Tauri Cargo Clippy passed.
- Native background-start smoke PASS: macOS launch services opened only the named QA app with an explicitly private synthetic TASKNOTES_DEV_DATA_DIR. SQLite user_version=4, integrity_check=ok, 0 Task records, and five expected domain tables validated using a read-only Python SQLite connection. The native sqlite3 -readonly CLI initially returned an open error; the separate Python read-only verification succeeded, so no false CLI PASS is claimed. QA process was stopped; normal Tasknotes app was preserved.
- Mac native QA *visible UI rendering* observed on the separate isolated bundle after activating its hidden window: All Tasks, Inbox, Lists and Task composer appeared. Accessibility tree did not expose embedded WebView input controls; native user-like click/type/restart is NOT proven. The QA app was closed again without touching the running ordinary Tasknotes; no raw screenshot was committed. This native app was built BEFORE the later paging and rename-source patches, so no claim of the latest bundle passing visual E2E is made.

## Limits, privacy and release gate

These synthetic test seams demonstrate code behavior, **not real hardware FPS** or owner-operated native GUI acceptance. The original Tasknotes app is still the owner's accepted smooth version and was neither quit nor overwritten. No personal Task/Note/SQLite backups, synthetic DB binaries, temporary absolute paths, raw screenshots, browser sessions, or secrets are committed.

**Remaining:** independent Standards/Spec code review and actual native click/edit/restart acceptance if allowed by the native desktop controls. Keep Draft PR, no merge or deployment.


## Post-review source correction — 2026-10-13

- Bounded Tasks in React: previously "Load more" indefinitely appended into the DOM despite SQL LIMIT. The precise Ticket04 requirement is now implemented as **one page of at most 50 Task cards at a time**, with reversible Previous/Next controls and a scope/sort-bound cursor stack. A 120-Task mocked public Tauri interface test went RED on missing "Next page", then GREEN and explicitly verifies 50→50→20→50→50 *visible cards* across navigation. Pending-page List switching remains disabled until the query returns.
- Safe List rename: a revision-conflict during rename previously closed the edit form and erased user input. Dedicated React test went RED, then GREEN after making the callback return whether the native write succeeded, preserving the draft on error.
- After these corrections: **30 React tests PASS**, typecheck PASS, frontend production build PASS. Rust tests remain **30 PASS**, core/Tauri Clippy with -D warnings and Rust format previously PASS because only React code changed. The compiled QA bundle predates these changes and must not be represented as a fresh native UI acceptance.
- **Independent AI reviewer gate BLOCKED:** Two separate read-only AGY calls were attempted against the exact initial PR head (Standards and Spec, prebuilt public-source diff packets); both returned RESOURCE_EXHAUSTED (HTTP 429, insufficient credits) with empty reviewer output. There is no independent reviewer verdict, and neither process should be represented as an approval. A documented, implementing-assistant two-axis review is provided separately and explicitly non-independent. This PR stays Draft and unmerged.

**As of the current handoff:** release/install/live SQLite migration are not authorized. Owner-operated macOS UI click/type/reopen has not been proved by AX, and an independent external Standards/Spec pass is still required before marking this Ticket completed.
