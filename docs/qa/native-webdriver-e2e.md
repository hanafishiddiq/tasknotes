# Tasknotes native E2E testing (macOS)

**Status:** Passed twice on synthetic native macOS WKWebView (2026-10-12), separate branch `test/native-wdio-e2e`. This is not a product feature or an authorization to merge existing Draft PRs.

## Purpose

Automate the exact native WKWebView workflow that previously needed repeated human clicking:

1. Create and edit a Task using the real macOS Tauri WebView.
2. Update status, five-level priority, percentage, Next Step and a dated Progress Note.
3. Complete the Task at 100% and reopen it to its previous 60%.
4. Switch sorting modes.
5. Close the first native app process, launch a new app process against the **same synthetic SQLite database**, and verify that all fields and dated events persist.
6. Validate SQLite schema, contents and integrity in read-only mode.

## Test commands

From the repository root on the macOS machine:

```bash
pnpm install --frozen-lockfile
pnpm qa:native:build
pnpm qa:native
```

The runner uses `@wdio/tauri-service` with `driverProvider: embedded`, debug-only `tauri-plugin-wdio-webdriver` and `tauri-plugin-wdio`, plus the conditional `@wdio/tauri-plugin` guest. The app's test window is hidden; WebDriver executes scoped element commands without global mouse/keyboard injection.

Tests must not run against normal `~/Library/Application Support` data. The driver only loads if built with the **opt-in `native-e2e` Cargo feature** and the app is given a private temporary directory named `tasknotes-wdio-*`. QA data are deleted after tests. The plugin binds its HTTP server to `127.0.0.1` and is omitted from normal builds; enabling it in release triggers a compile error.

Do not approve `edgedriver` or `geckodriver` installation scripts for this setup. The embedded Tauri provider needs neither; `pnpm-workspace.yaml` explicitly disallows their build scripts.

## Evidence boundaries

A passing native E2E test exercises actual Tauri frontend IPC + Rust application core + SQLite, not merely a mocked React test. It does **not** itself establish acceptable performance/FPS at scale, production release signing, or independent Standards/Spec code review.

If the native test runner fails, retain and report the specific failing assertion, driver errors and environment. The optional `KEEP_QA_DATA=1 pnpm qa:native` command retains only the failed *synthetic* temporary SQLite for diagnosis; normal runs clean it up. Never substitute a passing React unit test for a failing native workflow.

Official guidance:

- https://v2.tauri.app/develop/tests/webdriver/
- https://webdriver.io/docs/desktop-testing/tauri/plugin-setup
- https://webdriver.io/docs/desktop-testing/tauri/quick-start

## Verified results and driver limitation (2026-10-12)

Two complete E2E repetitions passed, each launching and stopping two separate Tasknotes app processes on the real macOS WebKit engine (WKWebView):

- **Spec 01:** native WebDriver input into Task title/description, native app Task creation, inline edits, priority Very High, In Progress/Completed/reopened at 60%, Next Step, dated Progress Note and both sort modes.
- **Spec 02:** new app process with the same synthetic database retained every field and note; UI assertions passed.
- **Read-only SQLite proof:** schema v3, exactly one edited Task, 4 Progress Entries, exact note `Native acceptance passed`, restored progress 60%, and integrity OK.
- Existing 19 React tests, 20 Rust integration tests, TypeScript and normal frontend build remained green.

**Test-driver limitation:** on the test-only, hidden WKWebView, WebDriverIO's `selectByAttribute` did not change a native `select` value. The QA helper first tries that method, then falls back to dispatching `input` and `change` events *inside the Tasknotes WKWebView* only. Actual Rust IPC and SQLite changes are then asserted end-to-end. This does **not** prove a physical mouse-selection interaction, which the owner previously performed manually, and does not justify declaring user-reported native UI lag fixed.

**Security boundary:** the test driver is behind the opt-in `native-e2e` Rust feature, with release-build `compile_error!`, synthetic TMPDIR checks, hidden QA window and localhost-only listener. The normal `pnpm build` did not bundle the QA guest integration. After QA, use `pnpm tauri build --debug --bundles app --no-sign --ci` to restore the *normal* desktop .app at the standard debug bundle path.

See [automated run evidence](native-webdriver-results-2026-10-12.md). No production signing or independent code review was performed by these tests.
