# 06: Pomodoro notifications on macOS 27 (modern center only)

**What to build:** The Pomodoro's "Focus done" and "Break over" notifications appear again on the installed app, with their buttons (Start break, Count overtime, Start focus) answering the timer, by posting, hearing clicks and withdrawing through `UNUserNotificationCenter` only (batch-2 spec items 17–18, unchanged).

**Blocked by:** None (found in the batch-3 install check; PR #43 merged)

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR #44 on `fix/batch3-modern-notifications`; two-axis review done and its findings fixed; the merge, the install build and the native check wait for the owner

## Acceptance criteria

- [x] No call to `NSUserNotificationCenter` is left: the notification plugin (`tauri-plugin-notification`, through `notify-rust` and `mac-notification-sys`) is gone from the shell's dependencies (`cargo tree -i` finds neither).
- [x] A focus end asks for one notification with Start break before Count overtime, with the system sound unless Silent; a break end asks for Start focus (unit tests on `post_for`; whether the sound plays is a native check below).
- [x] Each kind's buttons are registered at launch from the same table its notifications are shown and withdrawn by (`POMODORO_NOTIFICATIONS`).
- [x] A button's id names the click it answers, and the notification itself shows the window (unit tests on `reply`; a click on an older notification changes nothing, `pomodoro_state`'s existing tests).
- [x] Outside a bundled app (tests, `cargo run`) nothing is asked of macOS and nothing crashes.
- [x] On macOS 27.2, under Tasknotes' identifier, a notification shown by this module is delivered and presented (no "Denying" in `usernoted`'s log) and withdrawn.
- [ ] Native, in the installed app: a 1-minute focus shows "Focus done" with its sound; Start break and Count overtime on it answer the timer; a click on it shows the window; answering from the window or the menu bar takes it away; quitting with one shown takes it away.

## Delivery guardrails

- Own branch, Draft PR, `/tdd`, then `/code-review`; merge and install need the owner's confirmation.
- Synthetic fixtures only; scrub logs before they reach the public repository.

## Comments

**2026-11-11 — found in the install check:** after the batch-3 install no Pomodoro notification appeared. At the focus end `usernoted` logged `Denying message 3 from connection <LegacyConnection identifier: store.adamaja.tasknotes …>`, and at every launch since the install `Legacy client store.adamaja.tasknotes connecting to modern client. You can't mix modern clients with legacy clients.` (message 7, the start-up withdrawal; the nil it returned is what PR #43 stopped from aborting the app). The batch-2 build, running since its install, had delivered notifications the same day; code and bundle were otherwise the same (Info.plist, linked frameworks and ad-hoc signature identical; no change to the notification code between the two).

**2026-11-11 — diagnosis:** a throwaway probe app reproduced it in a second: under Tasknotes' identifier every `NSUserNotificationCenter` call gets nil and a "Denying" line, while `UNUserNotificationCenter` reports notifications allowed (banner style). Under a fresh synthetic identifier the older center kept working, both after a read through the modern center and after the binary was replaced, so neither alone explains the switch; the difference left is that Tasknotes is allowed to notify and reads its settings through the modern center (the one-time tip, `persistent()`), and macOS then treats it as a modern client. Which exact step flipped it is not proven; what is proven is that macOS 27.2 refuses the older center for this identifier, and that the modern center delivers.

**2026-11-11 — fix:** `macos_notifications` posts, registers the buttons, hears clicks (a delegate that also shows a notification while Tasknotes is in front) and withdraws through `UNUserNotificationCenter`; each kind keeps one request identifier, so withdrawing needs no list, and the notice number travels in the notification's user info. The first post asks macOS for permission (it asks the person only if it never has). `pomodoro.rs` keeps the words and buttons (`post_for`) and the meaning of a click (`reply`), both unit-tested; the native part has no seam a unit test can reach, so it was checked with a harness that compiles this same file into a bundle with Tasknotes' identifier: delivered and shown at the first try, and withdrawn.

**2026-11-11 — two-axis review (Standards, Spec) and what changed:**
- A real duplication: the buttons macOS showed came from the kinds registered in `listen`, while the tested `Post.buttons` reached nothing. Now one `Kind` (identifier and buttons) is registered, shown and withdrawn by, from one table; the test checks every notification is one of the registered kinds.
- Behaviour kept as in batch 2, which the first version had changed: while Tasknotes is in front a notification goes to Notification Center without a banner or sound (the older center did not present then), and macOS is asked for permission at launch (the older center asked at launch too), so the first notification no longer waits for that answer.
- Smaller: the user-info number is a generic `tag`, with its key named `TAG_KEY`; `show` instead of `post`; the click's store write runs on a named thread whose failure is reported, and showing the window needs none; an authorization error is reported; the SAFETY note on the user-info cast says why widening is sound; comments no longer claim the main thread or overstate the cause; the outside-a-bundle test also calls `listen`.
- Criteria above were ticked only where a test or the native harness backs them; clicks, sound and the withdrawal at quit wait for the installed app.
- Left as in batch 2 (Ticket 02 accepted it): the notification's button reads "Count overtime", not the live "Count +07:12" of spec 17–18; the live Overtime is in the window and the menu bar.
- Native check of the final code (same harness): delivered and presented at once, withdrawn 8 s later, no "Denying".
