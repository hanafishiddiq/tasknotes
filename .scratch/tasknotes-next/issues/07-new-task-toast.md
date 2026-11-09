# 07: Toast to jump to a new Task

**What to build:** After adding a Task, a quiet toast offers to jump to it (spec stories 1–2).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] A toast "Task added · Click to edit" appears bottom-right after any Task is created in the window, fading in and out, about 6 s, paused while hovered; no sound.
- [ ] Clicking it scrolls to the Task, opens it and flashes it yellow briefly; respects Reduce Motion.
- [ ] Adding several Tasks in a row keeps one toast pointing to the latest.
- [ ] The toast never moves the list by itself and never steals keyboard focus; it is announced politely to screen readers.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q3/Q14/Q25:** toast instead of a "Recently added" section or a lasting yellow mark.
