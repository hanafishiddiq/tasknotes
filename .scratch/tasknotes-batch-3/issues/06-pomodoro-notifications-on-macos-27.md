# 06: Pomodoro notifications on macOS 27 (modern center only)

**What to build:** The Pomodoro's "Focus done" and "Break over" notifications appear again on the installed app, with their buttons (Start break, Count overtime, Start focus) answering the timer, by posting, hearing clicks and withdrawing through `UNUserNotificationCenter` only (batch-2 spec items 17–18, unchanged).

**Blocked by:** None (found in the batch-3 install check; PR #43 merged)

**Status:** ready-for-agent

**Implementation:** in progress, on `fix/batch3-modern-notifications`

## Acceptance criteria

- [x] No call to `NSUserNotificationCenter` is left: the notification plugin (`tauri-plugin-notification`, through `notify-rust` and `mac-notification-sys`) is gone from the shell's dependencies.
- [x] A focus end posts one notification with Start break before Count overtime, with the system sound unless Silent; a break end posts Start focus.
- [x] A button answers the notice it came from (a click on an older one changes nothing); a click on the notification itself shows the window.
- [x] Answering from the window or the menu bar, quitting, and a restart take the Pomodoro notifications away.
- [x] Outside a bundled app (tests, `cargo run`) nothing is asked of macOS and nothing crashes.
- [x] On macOS 27.2, under Tasknotes' identifier, a notification posted by this module is delivered and shown (no "Denying" in `usernoted`'s log) and withdrawn.
- [ ] In the installed app: a 1-minute focus shows "Focus done", and its Start break button starts the break.

## Delivery guardrails

- Own branch, Draft PR, `/tdd`, then `/code-review`; merge and install need the owner's confirmation.
- Synthetic fixtures only; scrub logs before they reach the public repository.

## Comments

**2026-11-11 — found in the install check:** after the batch-3 install no Pomodoro notification appeared. At the focus end `usernoted` logged `Denying message 3 from connection <LegacyConnection identifier: store.adamaja.tasknotes …>`, and at every launch since the install `Legacy client store.adamaja.tasknotes connecting to modern client. You can't mix modern clients with legacy clients.` (message 7, the start-up withdrawal; the nil it returned is what PR #43 stopped from aborting the app). The batch-2 build, running since its install, had delivered notifications the same day; code and bundle were otherwise the same (Info.plist, linked frameworks and ad-hoc signature identical; no change to the notification code between the two).

**2026-11-11 — diagnosis:** a throwaway probe app reproduced it in a second: under Tasknotes' identifier every `NSUserNotificationCenter` call gets nil and a "Denying" line, while `UNUserNotificationCenter` reports notifications allowed (banner style). Under a fresh synthetic identifier the older center kept working, both after a read through the modern center and after the binary was replaced, so neither alone explains the switch; the difference left is that Tasknotes is allowed to notify and reads its settings through the modern center (the one-time tip, `persistent()`), and macOS then treats it as a modern client. Which exact step flipped it is not proven; what is proven is that macOS 27.2 refuses the older center for this identifier, and that the modern center delivers.

**2026-11-11 — fix:** `macos_notifications` posts, registers the buttons, hears clicks (a delegate that also shows a notification while Tasknotes is in front) and withdraws through `UNUserNotificationCenter`; each kind keeps one request identifier, so withdrawing needs no list, and the notice number travels in the notification's user info. The first post asks macOS for permission (it asks the person only if it never has). `pomodoro.rs` keeps the words and buttons (`post_for`) and the meaning of a click (`reply`), both unit-tested; the native part has no seam a unit test can reach, so it was checked with a harness that compiles this same file into a bundle with Tasknotes' identifier: delivered and shown at the first try, and withdrawn.
