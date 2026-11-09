# 14: Pomodoro daily history

**What to build:** A Pomodoro page with daily history (spec story 21).

**Blocked by:** 02 — Pomodoro timer with notifications and Overtime

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] A Pomodoro page lists days with session count, total focus minutes (planned + Overtime of finished sessions) and each session's time and Task.
- [ ] Stopped sessions are listed as Stopped and not counted.
- [ ] CLI and MCP can read the history by day range.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q9/Q19.**
