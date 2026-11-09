# 07: Toast to jump to a new Task

**What to build:** After adding a Task, a quiet toast offers to jump to it (spec stories 1–2).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] A toast "Task added · Click to edit" appears bottom-right after any Task is created in the window, fading in and out, about 6 s, paused while hovered; no sound. — `src/new-task-toast.test.tsx` (fake timers: leaves at 6000 ms + 120 ms fade; hover and focus hold it and resume with the time left). Every UI path (capture row, ⌘N, palette, Quick Capture) ends in `createTask`, which calls `added()`; the toast plays nothing (the existing create sound is unchanged).
- [x] Clicking it scrolls to the Task, opens it and flashes it yellow briefly; respects Reduce Motion. — App test: Task details open, `.task-slot--flash`, scroll target > 0 and never `smooth`, row focused; stylesheet test: fades only under `no-preference`, outline highlight outside any motion query. A Task the view hides opens on its own ("Showing the Task you just added").
- [x] Adding several Tasks in a row keeps one toast pointing to the latest. — unit and App tests: one toast, click opens the newest, timer restarts.
- [x] The toast never moves the list by itself and never steals keyboard focus; it is announced politely to screen readers. — App test: after adding, scrollTop stays 0, focus stays in the New task field; the toast sits in a persistent `aria-live="polite"` region positioned over the rows (absolute, so no layout shift). Native visual check still open.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q3/Q14/Q25:** toast instead of a "Recently added" section or a lasting yellow mark.
