# 10: Won't Do

**What to build:** Mark a Task Won't Do (ADR 0008; spec story 4).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] A Task can be marked Won't Do from its status control; it moves to the Completed group with a distinct mark and keeps its progress.
- [ ] Reopening clears Won't Do and returns it to Ready or In Progress by the usual progress rules.
- [ ] The transition is a Progress Entry; CLI and MCP can set, clear and filter Won't Do; Bulk Change status requests can include it.
- [ ] Glossary and ADR 0008 agree with the behaviour.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q1/Q15.**
