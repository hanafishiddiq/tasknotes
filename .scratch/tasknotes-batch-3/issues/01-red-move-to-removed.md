# 01: Red Move to Removed

**What to build:** Move to Removed at the end of an open Task reads Alert Red, text and icon, with a faint red fill under the pointer (spec story 1).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR open; native look waits for the batch-3 install build

## Acceptance criteria

- [x] The button's label and trash icon are Alert Red in both appearances; at rest it has no fill. — `src/stylesheet-a11y.test.tsx` ("colours Move to Removed Alert Red…"); the icon is `currentColor`, so it follows the label.
- [x] Under the pointer it keeps the red and takes a faint red fill; disabled, it dims like any disabled control and takes no fill. — same test (`:hover:not(:disabled)` only); `button:disabled` opacity 0.5 is unchanged.
- [x] The red label stays at WCAG AA 4.5:1 on the canvas and on its hover fill, dark and light. — `src/a11y-contrast.test.tsx`, surfaces `bg` and `remove-hover`: 8% red; 12% gave 4.31 and 10% gave 4.46 in light.
- [x] Only this button changes: Save, the other Task controls and the Notes toolbar trash icon keep their look; the action is still a reversible move with no dialog. — `src/removed.test.tsx` ("marks Move to Removed on an open Task as the red remove button"), existing Removed tests unchanged.
- [ ] The owner sees it red in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md` and `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner decision (Han), Q3:** only the button at the end of an open Task; red text and icon, the plain button background at rest, a faint red fill on hover; no confirmation added. The Notes trash icon sits in the toolbar, not at the bottom right, and stays as it is.
