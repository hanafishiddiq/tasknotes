# 01: First Task on a local desktop

**What to build:** Launch a minimal Tauri/macOS shell; create and edit a Task from UI; persist to local SQLite through Rust core; reload and find the same Task. Includes migration, minimal integration tests and synthetic-only fixtures.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Implementation:** review — implementation and automated checks passed; native GUI click/type acceptance still requires manual verification.

## Acceptance criteria

- [x] Launching Tasknotes on the target Mac opens a working Tauri desktop window without any sign-in or network dependency. **Evidence:** native ARM64 debug .app bundle launched, Tasknotes window enumerated by macOS, and synthetic app-data directory initialized SQLite schema v1.
- [ ] Creating a Task in the UI stores a stable ID, Ready status, 0% progress, and Medium priority through the Rust application core. **Automated seams pass:** React/Tauri command contract + Rust/SQLite core; **actual native window interaction not yet verified**.
- [ ] Editing a Task title/description and restarting the application preserves exactly one updated Task in SQLite. **Automated seams pass:** UI command contract and core reopen/persist test; **native clicked editor-to-restart path not yet verified**.
- [x] A fresh local database is migrated automatically, while invalid Task data produces an actionable error and does not leave a half-written record. **Evidence:** synthetic native startup schema v1, Rust invalid write + newer-schema negative controls.
- [x] The first list/query path is bounded and supports pagination or bounded result windows; it does not load every Task into the UI. **Evidence:** Rust cursor pagination test with cap 100; initial UI page 50 and explicit Load More test.

## Verification

- [ ] Perform manual Tauri GUI create/edit/restart using the built `Tasknotes.app` (macOS accessibility inspection could not expose native WebView input elements; no clicked native E2E result is claimed).
- [x] Rust core integration tests on isolated temporary SQLite.
- [x] React UI tests via the Tauri command seam.
- [x] Native app launch and schema migration in isolated app-data directory.
- [x] Relevant automated tests pass (5 React UI tests + 5 Rust core integration tests), TypeScript typecheck, release frontend build, Rust Clippy -D warnings, Rust formatting, native unsigned debug bundle.
- [x] Apply the two-axis `/code-review` checklist against `main`; see `docs/reviews/ticket-01.md`. Native GUI acceptance remains open, so Ticket 01 is **in review**, not complete.

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
