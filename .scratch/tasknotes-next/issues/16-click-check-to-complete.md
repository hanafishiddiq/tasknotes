# 16: Click a Task's check to complete it

**What to build:** The round check at the left of every Task row becomes a button: clicking it completes the Task, clicking it again reopens it.

**Blocked by:** 13 — Drag Tasks into a Custom Order (same row markup; build on it to avoid conflicting edits)

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] Clicking the check of a Ready or In Progress Task makes it Completed at 100% through the same core change the status control uses (revision checked, Progress Entry, completion sound). _Evidence: the check sends `change_task {id, expectedRevision, status: "completed"}`, the status capsule's own args (`checkActionArgs`, `src/lib/task-api.ts`), through `commitTaskMutation`, which plays the cue from the core's answer: `src/check-to-complete.test.tsx` "completes an In Progress Task at 100% through the core's status change, with the completion cue", and "is reached with Tab and pressed with Space or Return…" (a Ready Task). Revision and Progress Entry are the core's `change_task` (`tests/status_priority.rs`, `tests/progress_history.rs`); no Rust changed. Native GUI not checked._
- [x] Clicking the check of a Completed Task reopens it to its previous incomplete progress, by the existing reopen rule. _Evidence: `status: "in_progress"` with no progress, so the core restores the last incomplete percentage (`percent_and_status_transitions_preserve_last_incomplete_progress`): `src/check-to-complete.test.tsx` "reopens a Completed Task to the progress it had before…" (no cue, no Undo bar)._
- [x] Clicking the check (✕) of a Won't Do Task clears Won't Do and returns it to the status it had before (ADR 0008). _Evidence: `wontDo: false` alone, the core's clearing rule (`wont_do_from`, `tests/wont_do.rs`): `src/check-to-complete.test.tsx` "clears Won't Do back to the status the Task had before it was set aside" (progress kept, no Undo bar)._
- [ ] After completing from the check, an "Task completed · Undo" bar shows for a few seconds, like moving to Removed; Undo restores the previous status and progress. _Evidence: the Removed bar's markup in the window's last row, "Task completed" with Undo, 6 s, held under the pointer or focus (`src/lib/use-completion-undo.ts`): `src/check-to-complete.test.tsx` "the Undo offer … on its own" (timing, hold, newest wins, runs once), "shows “Task completed” with Undo, which restores the status and progress through the core, revision checked" (`{status: "in_progress", progress: <before>}` named against the completion's revision), "says so in the Task view when the core refuses the Undo", "goes when the Task changes again, on leaving the Task view, and is not offered by the status control", "takes the place of the Removed notice, whose move ⌘Z can still take back". **Left open:** a Task completed from Ready comes back In Progress at 0%, not Ready, because the core opens Ready only to work that has not started and counts a Completed Task as started ("brings a Task completed from Ready back In Progress at 0%…"); see Comments._
- [x] The check is a real button with an accessible name that says what a click does ("Complete …" / "Reopen …"), keyboard reachable, with a visible focus ring; it does not expand the row, and it is disabled while the Task has unsaved edits or a save is in flight. _Evidence: `<button type="button">` before the open button, named "Complete “Title”", "Reopen “Title”" or "Clear Won't do for “Title”": `src/check-to-complete.test.tsx` "is reached with Tab and pressed with Space or Return, leaving the row closed", "waits while the Task has unsaved edits or a save is on its way"; every scene's Tab reach and axe pass (`src/a11y-keyboard.test.tsx`, `src/a11y-axe.test.tsx`); `src/stylesheet-a11y.test.tsx` "keeps the round check a circle with the focus ring…" (shared `:focus-visible` ring, 26px hit area). The focus ring is checked in the stylesheet only; jsdom draws none._
- [x] The list never moves by itself beyond the Task leaving for its new group (PR #21), and the new-Task toast and drag-to-reorder keep working. _Evidence: `src/check-to-complete.test.tsx` "asks for no scroll when a check completes a Task or Undo brings it back, and the new Task toast stays"; `src/custom-order.test.tsx` "never starts a drag or a move from a Task's check…" (no drag mark, Alt+Up on the check moves nothing); the whole suite passes (`src/new-task-toast.test.tsx`, `src/custom-order.test.tsx` unchanged). Focus falls to the window after the Task leaves for its new group, like the other controls whose row moves; pinned in `src/a11y-keyboard.test.tsx` "waiting for the owner…"._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md`, ADR 0006 and 0008 before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI calls the existing status change.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-10 — owner request and decisions (Han), Q45–Q47:** "Buat agar tombol lingkaran task itu bisa dipencet untuk menandai task sebagai selesai", including In Progress Tasks. The check toggles: complete ↔ reopen; on a Won't Do Task it clears Won't Do; completing shows an Undo bar and plays the completion sound.

**2026-11-11 — implementation notes (Draft PR), for the owner.**
- Open, needs Han: Undo after completing a **Ready** Task from the check brings it back **In Progress at 0%**, not Ready. The core's `change_task` refuses Ready for any Completed Task ("Ready is only available for work that has not started"), and changing that is a core rule shared by the CLI and MCP (ADR 0006), so it was not changed here. For an In Progress Task, Undo restores status and progress exactly. Options: keep it; or a core rule that a completion taken back at once (revision checked) returns the Task to Ready, which touches `crates/tasknotes-core/src/tasks.rs`.
- Open: after a check moves its Task to another group, keyboard focus falls to the window, as it does for the status capsule; following the Task would scroll the list, which PR #21 forbids. Pinned with the other "waiting for the owner" focus cases.
- Choices: Undo is offered only after the check completes a Task (not after the status capsule, reopening or clearing Won't Do); it is not on ⌘Z, which stays "Undo the latest move to Removed"; one bar shows at a time, the newer taking the place of the Removed notice; the bar says "Task completed" with an Undo button, the owner's wording without the dot.
