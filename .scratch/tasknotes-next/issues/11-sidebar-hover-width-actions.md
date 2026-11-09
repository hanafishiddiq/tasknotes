# 11: Sidebar hover, width and action icons

**What to build:** Make the sidebar readable (spec stories 13–14).

**Blocked by:** 04 (manual sidebar order, PR #24)

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] Hovering a List or Note Folder shows a clearly visible pale-yellow background in light and dark themes, still AA for the text.
- [ ] The sidebar edge can be dragged between 210 and 400 px (default 260 px) and by keyboard; the width is remembered.
- [ ] When a name does not fit, the hover action icons cover its end on the row's background; nothing overlaps visibly (owner's screenshot case: "Content" with four icons).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q11/Q21/Q28.**
