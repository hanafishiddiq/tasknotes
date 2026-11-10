# 16: Click a Task's check to complete it

**What to build:** The round check at the left of every Task row becomes a button: clicking it completes the Task, clicking it again reopens it.

**Blocked by:** 13 — Drag Tasks into a Custom Order (same row markup; build on it to avoid conflicting edits)

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] Clicking the check of a Ready or In Progress Task makes it Completed at 100% through the same core change the status control uses (revision checked, Progress Entry, completion sound).
- [ ] Clicking the check of a Completed Task reopens it to its previous incomplete progress, by the existing reopen rule.
- [ ] Clicking the check (✕) of a Won't Do Task clears Won't Do and returns it to the status it had before (ADR 0008).
- [ ] After completing from the check, an "Task completed · Undo" bar shows for a few seconds, like moving to Removed; Undo restores the previous status and progress.
- [ ] The check is a real button with an accessible name that says what a click does ("Complete …" / "Reopen …"), keyboard reachable, with a visible focus ring; it does not expand the row, and it is disabled while the Task has unsaved edits or a save is in flight.
- [ ] The list never moves by itself beyond the Task leaving for its new group (PR #21), and the new-Task toast and drag-to-reorder keep working.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md`, ADR 0006 and 0008 before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI calls the existing status change.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-10 — owner request and decisions (Han), Q45–Q47:** "Buat agar tombol lingkaran task itu bisa dipencet untuk menandai task sebagai selesai", including In Progress Tasks. The check toggles: complete ↔ reopen; on a Won't Do Task it clears Won't Do; completing shows an Undo bar and plays the completion sound.
