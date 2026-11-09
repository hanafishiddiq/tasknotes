# 02: Pomodoro timer with notifications and Overtime

**What to build:** A Pomodoro timer, optionally for one Task, that keeps running with the window closed (spec stories 16–20, 22).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] Durations default to 25 / 5 / 15 (long break after every 4 sessions) and are adjustable and remembered.
- [ ] A session can be started with or without a Task; the menu bar shows time left while running and "🍅 Done" after focus ends until the owner responds.
- [ ] Focus end delivers one macOS notification, silent in Silent mode, with Start break and Keep focusing; every switch waits for a click.
- [ ] Keep focusing counts Overtime from the planned end ("🍅 +mm:ss") until Start break or Stop; choosing Start break directly counts no waiting time.
- [ ] A finished session tied to a Task adds one Progress Entry ("Pomodoro: 25 + N min focus") and never changes status; a Stopped session adds none.
- [ ] On first use, if the notification style is not Persistent, a one-time tip opens the Tasknotes page in System Settings; the exact macOS label is verified on the owner's Mac.
- [ ] Sessions are stored in the core with start, planned length, Overtime, outcome (finished / stopped) and optional Task.
- [ ] Ambient rain is unaffected by Pomodoro.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decisions (Han), grilling Q9, Q10, Q13, Q20, Q26, Q27, Q29, Q30, Q32:** as in the criteria. Owner rejected an always-on-top reminder window.
