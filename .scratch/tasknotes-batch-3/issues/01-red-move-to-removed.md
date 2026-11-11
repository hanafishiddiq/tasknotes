# 01: Red Move to Removed

**What to build:** Move to Removed at the end of an open Task reads Alert Red, text and icon, with a faint red fill under the pointer (spec story 1).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR open; native look waits for the batch-3 install build

## Acceptance criteria

- [x] The button's label and trash icon are Alert Red in both appearances; at rest it has no fill. — `src/stylesheet-a11y.test.tsx` ("colours Move to Removed Alert Red…"); the icon is `currentColor`, so it follows the label.
- [x] Under the pointer it keeps the red and takes a faint red fill; disabled, it dims like any disabled control and takes no fill. — same test (`:hover:not(:disabled)` only); `button:disabled` opacity 0.5 is unchanged.
- [x] The red label stays at WCAG AA 4.5:1 on the canvas and on its hover fill, dark and light. — `src/a11y-contrast.test.tsx`, surfaces `bg` and `remove-hover`: 8% red; 12% gave 4.31 and 10% gave 4.46 in light. A new check there fails when any `color-mix` fill in App.css is not a measured surface (review finding).
- [x] Only this button changes: Save, the other Task controls and the Notes toolbar trash icon keep their look; the action is still a reversible move with no dialog. — `src/removed.test.tsx` ("marks Move to Removed on an open Task as the red remove button") checks the Task details hold exactly one red button; the Notes trash icon is outside any test here and unchanged by construction (`button-remove` appears only in `TaskRow.tsx`; `NotesWorkspace.tsx` untouched); `onClick` and the existing Removed tests are unchanged.
- [ ] The owner sees it red in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md` and `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner decision (Han), Q3:** only the button at the end of an open Task; red text and icon, the plain button background at rest, a faint red fill on hover; no confirmation added. The Notes trash icon sits in the toolbar, not at the bottom right, and stays as it is.

**2026-11-11 — review on PR #38:** no code defects. Fixed: DESIGN.md's One Voice Rule and the surface-steps Do now name this exception, the Buttons intro says "a fill at rest", and the front matter lists `button-remove`; the contrast test now requires every `color-mix` fill to be a measured surface; this ticket's evidence no longer claims a test for the Notes icon. Carried to Ticket 02: for 1.5 s after the toast opens a Task, `.task-slot--flash` lays `--selection` over the open Task, where the red label measures about 2.66:1 dark and 4.29:1 light; whatever replaces that mark must keep Move to Removed readable.
