# 06: ⌘↵ saves the open edit group

**What to build:** ⌘↵ saves whichever Task edit group is open (spec story 3).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] ⌘↵ in the title/description, progress update, or tags/deadline group does exactly what that group's Save button does, including revision checks. — each group's key handler calls that group's existing save (form `requestSubmit`, `saveProgressUpdate`, `saveDetails`); `src/cmd-enter-save.test.tsx` checks the saved values and that an older draft still names its base revision (2, not 3).
- [x] With no unsaved edits ⌘↵ does nothing; Notes are unaffected (they autosave). — "does nothing in any group with no unsaved edits" test; the handlers sit only on TaskRow's three groups, the Notes editor is untouched; plain Return still makes a newline (two tests).
- [x] Each Save button shows a small `⌘↵` hint, readable by screen readers as "Command Return". — `aria-hidden` `kbd.save-shortcut` (11px, weight 400, button ink), `aria-keyshortcuts="Meta+Enter"` and accessible description "Command Return"; button names unchanged; recorded in DESIGN.md › Buttons. Review fixes: the description source is `hidden` (not `sr-only`) so VoiceOver's line reading skips it; one `SaveButton` carries the hint and attributes; the hint drops while "Saving..."; Return/⌘↵ during IME composition does not save; after a save by ⌘↵ or the button, focus lands on the Task's row (`a11y-keyboard.test.tsx`). Ctrl+Return is a quiet alias. Native VoiceOver check not done.
- [x] The shortcut list in the app includes it. — `SHORTCUTS` gains "⌘↵ Save the open Task edit"; Settings › Keyboard test.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q12:** saves the group being edited; show a hint on the button.
