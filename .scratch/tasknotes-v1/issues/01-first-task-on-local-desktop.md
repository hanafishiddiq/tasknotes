# 01: First Task on a local desktop

**What to build:** Launch a minimal Tauri/macOS shell; create and edit a Task from UI; persist to local SQLite through Rust core; reload and find the same Task. Includes migration, minimal integration tests and synthetic-only fixtures.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Implementation:** review — native GUI Task creation/edit/restart and automated checks verified 2026-10-12; independent code review and merge remain pending.

## Acceptance criteria

- [x] Launching Tasknotes on the target Mac opens a working Tauri desktop window without any sign-in or network dependency. **Evidence:** native ARM64 debug .app bundle launched, Tasknotes window enumerated by macOS, and synthetic app-data directory initialized SQLite schema v1.
- [x] Creating a Task in the UI stores a stable ID, Ready status, 0% progress, and Medium priority through the Rust application core. **Evidence:** actual native GUI Task created 2026-10-12, persisted in isolated SQLite, then edited; default invariants covered by Rust tests.
- [x] Editing a Task title/description and restarting the application preserves exactly one updated Task in SQLite. **Evidence:** user native GUI edits confirmed via read-only SQLite, actual macOS process quit/reopen, and native UI screenshot showing edited Task still present.
- [x] A fresh local database is migrated automatically, while invalid Task data produces an actionable error and does not leave a half-written record. **Evidence:** synthetic native startup schema v1, Rust invalid write + newer-schema negative controls.
- [x] The first list/query path is bounded and supports pagination or bounded result windows; it does not load every Task into the UI. **Evidence:** Rust cursor pagination test with cap 100; initial UI page 50 and explicit Load More test.

## Verification

- [x] User manually exercised Tauri GUI create/edit/restart on 2026-10-12. Read-only SQLite plus native UI screenshot verified the saved Task persisted across an actual process restart.
- [x] Rust core integration tests on isolated temporary SQLite.
- [x] React UI tests via the Tauri command seam.
- [x] Native app launch and schema migration in isolated app-data directory.
- [x] Relevant automated tests pass (5 React UI tests + 5 Rust core integration tests), TypeScript typecheck, release frontend build, Rust Clippy -D warnings, Rust formatting, native unsigned debug bundle.
- [x] Manual two-axis Standards/Spec review recorded in `docs/reviews/ticket-01.md`. **Independent** code review is still pending (Copilot reviewer quota exhausted), so Ticket 01 is **in review**, not complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

## Implementation evidence (2026-10-11)

- **TDD:** Rust core tests failed first when the shared crate module did not exist, then all 5 passed. Frontend behavior tests failed against the original Tauri demo, then passed; separate load-retry and save-error regressions followed red–green.
- **Native startup:** `pnpm tauri build --debug --bundles app --no-sign --ci` produced `src-tauri/target/debug/bundle/macos/Tasknotes.app` (ARM64, approximately 31 MB). Launched with `TASKNOTES_DEV_DATA_DIR` pointing at a private temporary directory. macOS detected the Tasknotes window, and the native process initialized `tasknotes.db` with `PRAGMA user_version=1` and an empty `tasks` table.
- **Data safety:** all synthetic fixture data stayed outside Git; no personal Notes/Tasks, secrets, or generated databases staged.
- **Automated suites:** `pnpm typecheck`; `pnpm test`; `pnpm build`; `cargo test --manifest-path crates/tasknotes-core/Cargo.toml`; `cargo clippy --manifest-path crates/tasknotes-core/Cargo.toml --all-targets -- -D warnings`; both Rust format checks; native Tauri debug bundle build.
- **Limitation:** the macOS accessibility adapter did not expose WebView fields for the requested native click/type smoke. Do not mark Ticket 01 completed until a manual macOS create→edit→quit/relaunch→verify test is recorded. Other unblocked tickets remain blocked pending this acceptance.

## Native acceptance update — 2026-10-12

- Direct user GUI creation and editing verified in isolated SQLite, then separately after macOS Tasknotes process quit/reopen with a screenshot. See [native QA report](../../../docs/qa/native-acceptance-2026-10-12.md).
- Code review still needs a genuinely independent reviewer; initial Copilot review could not run (quota reached). The reported UI lag is tracked as a **future performance bug** in [Issue 19](19-native-interaction-lag.md), blocking performance/release quality but not silently extending this ticket.

## Automated native QA — 2026-10-12

- Native E2E via embedded WKWebView WebDriverIO **passed twice** without user clicking: created Task, edited title/description and reopened a separate native process on the same synthetic SQLite database. See [native WebDriver evidence](../../../docs/qa/native-webdriver-results-2026-10-12.md). The independent Standards/Spec review gate is still open; ticket remains Review.
