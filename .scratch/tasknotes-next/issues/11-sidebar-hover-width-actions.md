# 11: Sidebar hover, width and action icons

**What to build:** Make the sidebar readable (spec stories 13–14).

**Blocked by:** 04 (manual sidebar order, PR #24)

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR (branch `feat/ticket-11-sidebar`)

## Acceptance criteria

- [x] Hovering a List or Note Folder shows a clearly visible pale-yellow background in light and dark themes, still AA for the text. — Evidence: new token `--side-hover` (dark `#4c4424`, light `#fbeca0`); `src/sidebar-stylesheet.test.tsx` (yellow, opaque, distinct from sidebar and current; hover never reaches the current folder); `src/a11y-contrast.test.tsx` checks `.folder`, `.folder--quiet`, `.folder-action` and the focus ring on `side-hover` in both themes. Native on-screen look still to be checked by the owner.
- [x] The sidebar edge can be dragged between 210 and 400 px (default 260 px) and by keyboard; the width is remembered. — Evidence: `src/sidebar-width.test.tsx` (separator a11y attributes, mouse drag, clamping, arrows/Home/End/Return, double-click reset, persistence across remount, bad or unavailable storage); `src/sidebar-stylesheet.test.tsx` (shell column from `--sidebar-width`, edge outside the sidebar scroll, hidden at ≤720px). Row drag-reorder suite `src/sidebar-order.test.tsx` unchanged and green.
- [x] When a name does not fit, the hover action icons cover its end on the row's background; nothing overlaps visibly (owner's screenshot case: "Content" with four icons). — Evidence: `.folder-actions` takes the opaque `--side-hover` fill (`--side-selected` after the current folder) and the hover gutter that was too narrow for four icons (76px for 98px of icons) is gone; `src/sidebar-stylesheet.test.tsx` and the "sidebar row actions" test in `src/sidebar-width.test.tsx` (List and Note Folder "Content" rows). Native on-screen look still to be checked by the owner.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q11/Q21/Q28.**
