# Native Tasknotes WKWebView E2E — Standards / Spec review

**Fixed point:** 2937c9a (Ticket 03 QA evidence baseline).
**Branch:** test/native-wdio-e2e; no production Tasknotes feature behavior changed.
**Method:** Two-axis manual diff review. Independent third-party reviewer approval is **not** claimed.

## Standards

- **Pass:** QA integration is an optional Rust native-e2e feature; accidental release builds with this feature cause a compile error.
- **Pass:** The embedded driver refuses any database path other than a canonical private TMPDIR named tasknotes-wdio-*. The server binds only to 127.0.0.1; the QA window is hidden.
- **Pass:** Standard Tauri config enables default capabilities only. The global Tauri API and WDIO permissions are in the test-only overlay.
- **Pass:** Frontend WDIO guest is conditional on VITE_NATIVE_E2E=1. A normal frontend production build contained no QA artifacts, and the ordinary native Tasknotes.app was restored after testing.
- **Pass:** Runner uses fresh mkdtemp synthetic SQLite and deletes after completion. A diagnostic override retains only deliberately isolated failed fixtures.
- **Pass:** Typecheck, 19 React tests, 20 Rust tests, strict Clippy, Rust format, frozen pnpm lockfile and normal native bundle all passed.
- **Observation:** Native QA build temporarily replaces the normal debug app binary in the shared target directory. Documentation requires rebuilding the ordinary app afterward. A dedicated target directory could avoid this in the future at additional cache cost.
- **Observation:** Linux GitHub CI checks core/frontend; native macOS QA is a local evidence gate until an eligible Mac runner is available.

## Spec / acceptance

- **Pass:** Real Tasknotes WKWebView accepted create/edit, status, priority, 60% progress, Next Step, dated Progress Note, Completed to reopened 60% and both sort choices.
- **Pass:** Two complete repetitions, each with separate native processes for creation and reopening, verified persistence after restart.
- **Pass:** Read-only SQLite checker confirmed schema 3, one edited Task, four append-only history events, exact Progress Note and integrity.
- **Pass:** Previously unverified note entry was persisted in actual React/Tauri/Rust/SQLite rather than mocked unit tests.
- **Limit:** Hidden WKWebView selectByAttribute failed to change native select values. The test helper uses a scoped DOM input/change fallback, then asserts Rust/SQLite state. Not a physical mouse/dropdown picker test.
- **Limit:** Passing functional E2E does not prove responsiveness. Native performance Bug #19 remains unmeasured and unresolved.
- **Limit:** Independent Standards/Spec reviewer approval for parent PRs #1–#3 remains outstanding (Copilot review quota exhausted).

**Decision:** Ready for a stacked Draft QA PR targeting Ticket 03. Do not merge the parent feature PRs merely because functional E2E passed; preserve independent review and schema-migration safeguards.
