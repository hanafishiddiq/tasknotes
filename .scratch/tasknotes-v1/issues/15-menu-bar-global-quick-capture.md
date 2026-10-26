# 15: Menu bar and global Quick Capture

**What to build:** Optional macOS menu-bar mode, close-to-quit default, configurable global Quick Capture shortcut, shortcut conflict handling and user-visible settings. Verify app lifecycle behavior on Mac.

**Blocked by:** 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** implemented, awaiting independent review (branch `feat/ticket-15-menu-bar-quick-capture`; plan `docs/plans/ticket-15-menu-bar-quick-capture-2026-10-26.md`).

## Acceptance criteria

- [x] Closing the window quits by default when menu-bar persistence is not enabled. *(`close_action` unit tests in `src-tauri/src/shell_settings.rs`; closing still goes through the save-before-quit path; native check: checklist 1)*
- [x] Optional menu-bar mode keeps Tasknotes available without a window and allows reopening the main UI. *(tray item Open Tasknotes / New Task… / Quit Tasknotes, Dock click reopens; closing quits if the item could not be shown; React tests "Menu bar setting"; native: checklist 2–3, not yet run)*
- [x] A configurable global shortcut opens Quick Capture from another app when permissions permit. *(off by default; recorded in Settings; `tauri-plugin-global-shortcut` 2.4.1; opens the window on the ordinary New task field; React test "Quick Capture from the global shortcut"; native: checklist 4, not yet run)*
- [x] Shortcut conflicts and permission denial show a comprehensible fallback without disabling ordinary Task creation. *(Rust validation/registration tests with a fake shortcut table; React tests for refused, taken and launch-time failures, each naming ⌘N, and ⌘N still focusing an enabled New task field)*
- [x] App quit/launch and persisted menu-bar settings behave consistently across restarts. *(`shell-settings.json` round trip, unreadable file and launch with a taken shortcut tests; native restart: checklist 5, not yet run)*

## Verification

- [ ] Verify native macOS tray/shortcut behavior through manual and supported automated acceptance checks.
- [x] Relevant tests pass and evidence is recorded here. *(see Comments, 2026-10-26)*
- [x] Run `/code-review` and address findings before marking complete. *(findings fixed, see Comments 2026-10-26 — review fixes; native checks remain open)*

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-25 — owner decision (Han):** The global Quick Capture shortcut is **off by default**. The user turns it on and picks the key combination in Settings, so nothing conflicts with other apps until then.

**2026-10-26 — implementer (Ticket 15), implemented, awaiting independent review.** Status unchanged.

Evidence (implementer's own runs on the branch, integration tip 57d0ec6 unchanged): `cargo test` in `src-tauri` 24 passed (1 before; 23 new in `shell_settings.rs` and `quick_capture.rs`); `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and `cargo build` clean. `pnpm test` 18 files / 196 tests passed (186 before; 10 new in `src/menu-bar-quick-capture.test.tsx`); `pnpm typecheck` and `pnpm build` clean. No core crate or schema touched, so core tests were not re-run. No native launch was done.

Decisions (details in the plan):
- saya pilih `shell-settings.json` beside the database karena the shell needs the settings before the page loads and they are device window behaviour, not core data (no schema change, no clash with Ticket 11).
- saya pilih `tray-icon` + `tauri-plugin-global-shortcut =2.4.1`, used from Rust only, karena that needs no new capability or JS permission.
- saya pilih Quick Capture = the ordinary New task field (window shown and focused) karena one capture path means one rule set.
- saya pilih the shortcut rules in Rust (two modifiers or a function key; known macOS shortcuts and media keys refused) karena the UI must not duplicate them; a refused or taken combination is never saved and a working one keeps working.
- saya pilih closing quits whenever the menu-bar item could not be shown karena hiding the window then would strand the app.

For owner review: Dock icon kept in menu-bar mode; full-colour tray icon instead of a template glyph; menu-bar item wording; whole window rather than a floating capture panel; Settings wording; the two-modifier rule; section placement.

Open: native macOS checks below not run; `/code-review` not run yet.

Native click-checklist for Han (synthetic data only: a debug build with `TASKNOTES_DEV_DATA_DIR` pointing at an empty folder, or an isolated QA bundle; never the real data folder):
1. Fresh data folder, launch. Settings → "Menu bar and Quick Capture": both checkboxes off; hint "Closing the window quits Tasknotes." Close the window (red button): the app quits (no menu-bar item, gone from the Dock).
2. Relaunch, tick "Keep Tasknotes in the menu bar": the Tasknotes icon appears in the menu bar at once. Close the window: it disappears, the app keeps running. Click the menu-bar icon → Open Tasknotes: the window comes back where it was, with any unsaved Note text intact. Close again and click the Dock icon: the window comes back.
3. With the window hidden, menu-bar icon → Quit Tasknotes: it quits (an unsaved Note is saved first, as with ⌘Q). ⌘Q with the window open also quits.
4. Tick "Open Quick Capture with a global shortcut", press ⌃⌥Space: the hint reads "Press ⌃⌥Space in any app to add a task." Switch to Safari, press ⌃⌥Space: Tasknotes comes forward with the New task field focused; type a title, Return: the Task appears in All tasks. Also try it with the window hidden in menu-bar mode, and New Task… from the menu-bar item.
5. Quit and relaunch: both settings are still on, the menu-bar icon is there without opening Settings, and ⌃⌥Space works from another app straight away. Untick both, relaunch: no menu-bar icon, ⌃⌥Space does nothing, the hint says the combination is kept.
6. Conflicts: record ⌘N → refused, with the two-modifier explanation and "⌘N still creates a task inside Tasknotes."; press Esc and check ⌘N still focuses New task. Record ⌘⇧4 → "macOS uses this for Screenshots". Record a combination another running app already holds globally (e.g. one set in Raycast/Alfred): "already used by another app or by macOS…", the previous shortcut keeps working. While recording, ⌘K/⌘F do nothing.
7. Edit `shell-settings.json` in the test data folder to `{ broken`, relaunch: the app opens with defaults (closing quits), and Settings shows "Tasknotes could not read these saved settings, so the defaults are in use. Changing a setting saves them again." Change a setting: the line disappears and the file is rewritten valid.
8. Launch notice: turn Quick Capture on with ⌃⌥Space and quit. In another app (e.g. Raycast/Alfred) bind ⌃⌥Space globally, relaunch Tasknotes: the Task view shows one banner "Quick Capture could not use ⌃⌥Space: it is already used by another app or by macOS, or the system refused it. Choose another shortcut in Settings. ⌘N still creates a task." with Open Settings and Dismiss. Creating a Task works with the banner up; Dismiss removes it; Open Settings opens the section with the same reason. Reload the page (⌘R in a debug build): the banner does not come back until the next launch.
9. Cold start: with Quick Capture on, quit, then launch Tasknotes and press ⌃⌥Space straight away while the window is still loading: once the list appears the New task field is focused (the press is not lost).
10. Rain with menu-bar mode (Ticket 16): with Ambient rain on, close the window in menu-bar mode: rain keeps playing; Open Tasknotes or ⌃⌥Space brings the window back with rain still playing. Untick menu-bar mode and close the window: the rain fades out and the app quits; with menu-bar mode on, Quit Tasknotes in the menu-bar item fades it out too.


**2026-10-26 — review fixes (implementer, Ticket 15).** Status unchanged. Integration tip 930764d (Ticket 16 sound) merged in; Settings keeps both sections (Sound, then Menu bar and Quick Capture).

What each code-review finding became:

1. Platform: new `platform.rs` (`Platform::Mac / Windows / Other`, chosen by `cfg!` so every branch compiles and is tested on any host). Reserved-shortcut lists, key labels (`⌃⌥Space` vs `Ctrl+Alt+Space`, `Win+` on Windows, Return/Enter, Delete/Backspace), the system name in refusals, the tray place ("menu bar" / "notification area" / "system tray") and the Quit keys (⌘Q / Ctrl+Q) are per platform. No "macOS" text reaches another system. Module `cfg(desktop)` gates and the macOS-only Dock reopen are kept.
2. No duplicated rules: refusal reasons and the close hint are worded in Rust (`ShortcutProblem::reason`, `unavailable_reason`, `ShellState::view`); Settings shows them. The New task keys come from `SHORTCUTS` (`newTaskKeys` in `src/lib/shortcuts.ts`).
3. Unreadable `shell-settings.json`: Settings shows a calm status line that the defaults are in use until a setting is saved again.
4. Persistence: `ShellState::set_keep_in_menu_bar` and `set_quick_capture` both undo the change on a failed save (the shortcut registration included, via `QuickCapture::restore`) and return the error; the tray is only synced after a successful save. Settings re-reads the shell after any error.
5. Launch-time failure: the shell keeps a one-time launch notice (`take_launch_notice`); the Task view shows it once as a non-blocking banner with Open Settings / Dismiss and the New task keys. Task creation is unaffected (test).
6. Cold start: a press (or the tray's New Task…) is queued in `ShellState`; the page collects it with `take_quick_capture` after its listener is ready and on each event, so it is neither lost nor doubled. A capture during loading waits for the New task field to be enabled.
7. Tray: `build_tray` fails without an icon; `ShellState` records whether the item was really built and decides closing from that, so closing quits when it was not.
8. Smells: `ShellState` holds one `ShellSettings` (QuickCapture no longer keeps its own copy); the remembered label is computed in `QuickCapture::view`; the run-event closure became `on_run_event(app, event)`.

Ticket 16 seam: hiding to the menu bar tells the page nothing, so rain keeps playing (Ticket 16 engine test plus new App test in `src/sound.test.tsx`); closing with menu-bar mode off and the tray's Quit go through `quit::request`, which emits the quit request the Ticket 16 saver fades on (its App test). The Rust glue between closing and that event is build-checked only.

Evidence (implementer's own runs, after the merge): `cargo test` in `src-tauri` 41 passed (24 before; quick_capture and the new `shell_state` tests); `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo build` clean. `pnpm test` 21 files / 231 tests passed; `pnpm typecheck` and `pnpm build` clean.

Not verified: compiling for Windows or Linux targets (only `aarch64-apple-darwin` is installed; `cargo check` ran for the host only, though every platform branch is compiled and tested there); Linux tray (libappindicator) not built; no native launch (checklist below, items 8–10 new).
