# 06: ⌘↵ saves the open edit group

**What to build:** ⌘↵ saves whichever Task edit group is open (spec story 3).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] ⌘↵ in the title/description, progress update, or tags/deadline group does exactly what that group's Save button does, including revision checks.
- [ ] With no unsaved edits ⌘↵ does nothing; Notes are unaffected (they autosave).
- [ ] Each Save button shows a small `⌘↵` hint, readable by screen readers as "Command Return".
- [ ] The shortcut list in the app includes it.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q12:** saves the group being edited; show a hint on the button.
