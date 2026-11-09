# 12: Lists inside Lists

**What to build:** Lists can contain Lists, five levels deep (ADR 0009; spec story 12).

**Blocked by:** 04 (manual sidebar order, PR #24)

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] A List can be dragged into or out of another List in the sidebar (and moved by keyboard as in PR #24); depth above 5 is refused.
- [ ] A List shows only its own Tasks; a new sub-List starts empty.
- [ ] Inbox cannot be moved into a List and cannot hold Lists.
- [ ] Removing a List hides the Lists and Tasks inside it until it is restored; through AI it needs Bulk Change approval when it holds anything.
- [ ] CLI and MCP can create a List inside another, move a List and list the tree; exports keep the nesting.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q7/Q8/Q18/Q24.**
