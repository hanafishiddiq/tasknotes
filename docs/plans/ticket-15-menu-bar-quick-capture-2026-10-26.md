# Ticket 15 — Menu bar and global Quick Capture: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at 57d0ec6 (Tickets 07–10 and 14). Spec stories 40, 41 and 42. Owner decision 2026-10-25: the global Quick Capture shortcut is **off by default**; the person turns it on and picks the combination in Settings.

Tickets 16 (sound) and 11 (Note CLI privacy) run in parallel. This ticket touches no core crate and no schema, adds its own Settings section as one component (`MenuBarSettings`), and changes `SettingsView` by one import and one line.

## Decisions made by the implementer (owner may revisit)

- **saya pilih a small JSON file, `shell-settings.json`, beside the database in the app data folder** karena the shell needs these settings before the page loads (to show the menu-bar item, register the shortcut and decide what closing does), so `localStorage` (Ticket 14's theme pattern) is too late, and they describe this device's window behaviour, not Tasks or Notes, so they do not belong in the core schema (Ticket 11 may take v11). The debug-only `TASKNOTES_DEV_DATA_DIR` override applies to it too. Writes go to a `.partial` file renamed over the old one. An unreadable file means defaults for this launch and is left alone until the person changes a setting.
- **saya pilih official Tauri v2 pieces only:** `tauri` feature `tray-icon` for the menu-bar item and `tauri-plugin-global-shortcut` pinned `=2.4.1` (16 days old; 3.x is still alpha). Both are used from Rust only, so **no capability or JS permission was added**; `capabilities/default.json` still grants `core:default` alone. The plugin is a dependency only on desktop targets, and the shell modules are `#[cfg(desktop)]`.
- **Closing quits by default.** With menu-bar mode on, closing hides the window and the menu-bar item stays (Open Tasknotes, New Task…, Quit Tasknotes). Quit there and ⌘Q go through the existing save-before-quit path. **If the system could not show the menu-bar item, closing still quits** so the app can never be stranded without a window; Settings says so.
- **Quick Capture is the ordinary New task field.** The shortcut (or New Task… in the menu-bar item) shows and focuses the main window and the page focuses "New task", exactly like ⌘N. Tasks are created through the same `create_task` command; there is no second capture form and no second rule.
- **Shortcut rules live in Rust** (`src-tauri/src/quick_capture.rs`); Settings only records keys and words the answer. Accepted: two or more modifiers with a key, or a function key alone. Refused: one modifier or none (it would take over ⌘C, ⌘N and the like everywhere), a short list of well-known macOS shortcuts (Screenshots, Emoji & Symbols, Force Quit, Lock Screen, Log Out, Full screen, Finder search), and media keys (they would need Accessibility access, which Tasknotes does not ask for). Anything else the system refuses is caught when registering.
- **A refused or taken combination is not saved**, and a shortcut that was working keeps working ("⌃⌥Space still opens Quick Capture."). A saved shortcut another app took since the last launch is reported in Settings and kept, so the next launch tries again. Every fallback says "⌘N still creates a task inside Tasknotes."; nothing disables ordinary Task creation.
- **Turning it off releases the shortcut but remembers it**, so turning it back on needs no new recording.
- **Recording uses the physical key** (`KeyboardEvent.code`), so the keyboard layout does not change what is registered. While recording, every key belongs to the recorder: ⌘N and other in-app shortcuts do nothing; Esc cancels.
- **Permission denial:** macOS registers these hotkeys without Accessibility or Input Monitoring permission (the plugin's Carbon `RegisterEventHotKey` path). The only key type that would need a permission (media keys) is refused up front; any other refusal from the system shows the same "already used by another app or by macOS, or the system refused it" fallback.
- **Other devices:** where the shell has no such commands (a future non-desktop build or an older shell), Settings shows "Menu bar and Quick Capture are not available on this device." and the rest of Settings works. Nothing in the UI is macOS window chrome.
- **Dock click** on a hidden window brings it back (`RunEvent::Reopen`).

## For owner review (visual/product preferences not settled by DESIGN.md or the spec)

1. The Dock icon stays while Tasknotes lives in the menu bar (no switch to a menu-bar-only "accessory" app). Hiding it is a small change if preferred.
2. The menu-bar item uses the full-colour app icon, not a monochrome template glyph; a dedicated template icon would look more native.
3. Menu-bar item contents and wording: Open Tasknotes, New Task…, Quit Tasknotes.
4. Quick Capture brings up the whole main window on the New task field rather than a small floating capture panel.
5. Settings wording: "Keep Tasknotes in the menu bar", "Open Quick Capture with a global shortcut", "Press the new shortcut", and the fallback sentences.
6. The two-modifier rule (a single ⌥ or ⌃ plus a key is refused).
7. The section sits between Appearance and Keyboard under one heading, "Menu bar and Quick Capture".

## Seams under test (/tdd)

- **Rust rules** (`shell_settings.rs`, `quick_capture.rs`), unit tests: close decision (quit by default, hide with menu-bar mode, quit when the item could not be shown, never hide during a quit); settings file (first launch, round trip, unreadable file left alone, missing fields); shortcut validation and labels; turning Quick Capture on/off, changing it, a taken combination, a refused one and launch with a taken one — against a fake shortcut table (`Registrar`). Catches rule regressions; misses the real OS table and tray.
- **React App with mocked Tauri invoke and events** (`src/menu-bar-quick-capture.test.tsx`): the Quick Capture event focuses New task from Settings and creates a Task through `create_task`; menu-bar setting default/save/unavailable/missing shell; shortcut off by default, recording, in-app keys inert while recording, refused and taken fallbacks with ⌘N still working, launch-time failure, off keeps the combination. Misses native delivery of the event and real key presses from other apps.
- **Native glue** (`shell.rs`: tray, plugin, window hide, Dock reopen) is build-checked only; the owner checklist in the ticket covers it.

## Not done here

- No native launch was run: an isolated QA bundle (separate bundle identifier and data folder) was not set up in this worktree, and launching the shared identifier could touch the owner's WebView storage. The ticket carries a click-checklist.
