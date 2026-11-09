# 13: Drag Tasks into a Custom Order

**What to build:** Drag to reorder Tasks in one List (spec stories 7–11).

**Blocked by:** 09 — Difficulty

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] In a single-List view, a Task can be dragged (and moved by keyboard) within its work-status group; dropping elsewhere does not change status.
- [ ] A drag that breaks the current sort switches that List to Custom, starting from the order on screen; changing priority through its control never switches to Custom.
- [ ] Each List remembers its sort and its Custom Order; switching away and back restores it; a new Task lands at the top of its group in Custom.
- [ ] Drag is off in search, tag filters and multi-List views.
- [ ] CLI and MCP can list in Custom Order and move one Task after another with a revision check, without Bulk Change.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q5/Q6/Q16/Q17/Q31.**
