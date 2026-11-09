# 07: Toast to jump to a new Task

**What to build:** After adding a Task, a quiet toast offers to jump to it (spec stories 1–2).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — PR #27 merged 2026-11-09; the native check waits for the single batch-2 build (Ticket 15)

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

**2026-11-09 — review fixes on PR #27:** the toast's scroll request now gets one try, hit or miss, and any change of view drops it, so the list never moves by itself (PR #21); moving the toast's Task to Removed takes the toast away; a Task opened alone shows one banner; `showTaskAlone(taskId, reason)` replaces the borrowed `openLinkedTask` path. Tests in `src/new-task-toast.test.tsx`.

**2026-11-09 — accepted limit (not fixed):** a new Task that belongs in the current view but is not on a loaded page (past the first page) is shown alone ("Showing the Task you just added") instead of scrolled to in place, because the toast only checks the Tasks already loaded. Also accepted: while Notes or Settings is open the toast is hidden but its 6 s timer keeps running. Only an in-app move to Removed clears the toast; a Task removed with its List, by a Bulk Change, or from the CLI or MCP still opens alone and shows an empty view.
