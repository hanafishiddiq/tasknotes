# 02: New Task yellow mark

**What to build:** A pale-yellow mark on Tasks just added, by the owner in the window or by an AI through the CLI or MCP; the new Task toast stays 10 seconds with an X; opening a Task from the toast shows no outline or flash (spec, "New Task yellow mark", Q4–Q13).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — toast and flash part done on `feat/batch3-new-task-mark`; the mark waits for Q11/Q12 to be clarified

## Acceptance criteria

- [x] The toast stays 10 seconds, held while the pointer or focus is anywhere on it, including moving focus between its two buttons. — `src/new-task-toast.test.tsx` ("shows for 10 seconds…", "pauses while hovered or focused…": 4 s, held, then 6 s left).
- [x] Its X ("Dismiss") closes it at once, without a fade, and opens nothing; a later add brings it back with its full time. — `src/new-task-toast.test.tsx` ("goes at once from its X…").
- [x] Opening a Task from the toast lays no outline and no flash over it; the open Task keeps its usual open-row yellow. — App test "clicking it scrolls to the new Task and opens it, with no outline or flash"; stylesheet test: no `.task-slot--flash` rule and no outline on `.task-slot`.
- [x] The X is readable on the toast's hover-gray box (3:1 for an icon-only control). — `src/a11y-contrast.test.tsx`, `.task-toast-close` and its hover.
- [ ] A Task the owner adds in the window is marked (sidebar hover yellow fill on its row, no border, kept under the pointer) until any Task is opened; several adds in a row are all marked; switching List keeps the marks; quitting clears them.
- [ ] A Task created through the CLI or MCP is marked the same way, with no toast; when its mark goes per Q11/Q12 (being clarified).
- [ ] Text on the mark (title, metadata, priority and deadline colours) and the red Move to Removed keep WCAG AA.
- [ ] Screen readers hear that a marked Task was just added.
- [ ] The owner sees the toast, the X and the mark in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md` and `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner decisions (Han), Q4–Q13:** see the spec's "New Task yellow mark" section. Q4 replaces the 2026-11-09 decision (batch 2 Ticket 07: "toast instead of a Recently added section or a lasting yellow mark"): the toast stays and a lasting mark is added. Out of this ticket: the reorder notice above the toast keeps its 2 seconds and has no X; the completion Undo bar keeps its 6 seconds.
