# 09: Difficulty on Tasks

**What to build:** Optional Difficulty beside Priority (spec stories 5–6).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] A Task has Difficulty Very Easy … Very Hard or No Difficulty; existing and new Tasks start with No Difficulty.
- [ ] The row shows a Difficulty pill only when set; the editor can set and clear it.
- [ ] Filter by Difficulty and sort by Difficulty (Very Hard first, No Difficulty last) work in the UI, CLI and MCP.
- [ ] Changing Difficulty is a revisioned change recorded in audit history; exports include it.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q4/Q23.**
