# Ticket 01 — Two-axis code review

**Fixed point:** `main` at planning completion (`e4c728c`).
**Review source:** `.scratch/tasknotes-v1/issues/01-first-task-on-local-desktop.md`, `.scratch/tasknotes-v1/spec.md`, `AGENTS.md`, relevant ADRs and `GLOSSARY.md`.
**Method:** Standards and Spec axes reviewed independently against the feature changes. This document does not claim a separate third-party/subagent approval.

## Standards axis

- **Pass:** Rust domain/storage logic is isolated in a small, testable `tasknotes-core` interface; Tauri command adapters delegate without duplicating validation or persistence.
- **Pass:** SQLite migrations/validation are inside the Rust core, not in the UI; default Task values and revisions enforced at the storage seam.
- **Pass:** Typescript uses a typed Tauri command interface. No Tauri demo `greet`/opener permission. No personal Tasks, Notes, passwords, backups or SQLite databases staged.
- **Pass:** `cargo clippy --all-targets -- -D warnings`, rustfmt, TypeScript typecheck, and frontend tests/build have all passed locally.
- **Finding corrected:** the source stylesheet was initially compressed and difficult to maintain. It is now formatted into readable CSS.
- **Finding corrected:** failed writes previously looked like failed initial loads when an empty list was displayed. Separate `loadError` state and regression assertions now prevent that.
- **Remaining technical debt:** no standalone native macOS GUI automation harness; future higher-level UX work should build one if feasible without compromising the user's computer focus.

## Spec axis

- **Pass with automated evidence:** Creating Task in UI calls Tauri `create_task`, using Ready/Medium/0%; Rust core test covers persisted values through an actual SQLite file.
- **Pass with automated evidence:** Expanding/editing sends revision-protected Tauri `update_task`; Rust core covers edit, reopen and preservation in SQLite.
- **Pass:** Migration and invalid-input rollback verified by Rust integration tests; native Tauri startup created v1 schema on isolated temporary storage.
- **Pass:** Initial list uses bounded 50-record request and explicit Load More; core enforces upper page size 100 and cursor deduplication.
- **Pass:** Offline unsigned `Tasknotes.app` built successfully on ARM64 macOS and actual native startup/window was observed.
- **Open acceptance item:** UI click/type in the real native WebView, then restart and re-open the edited Task, has **not** been witnessed. macOS accessibility tooling found the native window but could not address its HTML controls. Unit/adapter-contract tests and real core integration tests must not be presented as equivalent to that native GUI journey.
- **Out of scope:** 5-priority editing, statuses beyond initial Ready, Notes, AI CLI/MCP, ambient sounds, search/indexing, full 50k/10k performance acceptance (Tickets 02–18).

**Review conclusion:** code and automated acceptance ready for PR; keep Ticket 01 in **review** until the single native manual E2E criterion is verified. Do not merge until acceptance review and CI are green.

## 2026-10-13 evidence correction (not independent review)

The original open native-click/restart finding has been superseded by direct manual native evidence and two real WKWebView/SQLite create→edit→second-process verify runs. The implementing assistant reviewed the Standards and Spec axes again; **independent review was not obtained** because the Copilot reviewer returned a quota-exhausted comment. See [acceptance report](../qa/tickets-01-03-acceptance-reconciliation-2026-10-13.md). The Draft PR remains unmerged and the ticket stays in review.

## 2026-10-13 independent AI two-axis review

Two read-only, separate external AI agents reviewed the **pinned original feature diff** on Standards and Spec axes. See [source reports and evidence checks](tickets-01-03-independent-2026-10-13.md), including the independent factual correction of a false Spec blocker or stale native-evidence statement. This does **not** constitute an authenticated GitHub human approval and does not authorize merging.
