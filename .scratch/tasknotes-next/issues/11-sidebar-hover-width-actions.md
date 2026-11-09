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

**2026-11-09 — owner decision (Han): hover scope.** The pale-yellow hover stays on **all** sidebar entries (All tasks, Inbox, Settings, Removed, Lists, Note Folders), not only Lists and Note Folders. Decided, not open; recorded in DESIGN.md (Sidebar Hover Yellow).

**2026-11-09 — review fixes (PR #28).**
- Drag performance: a drag now moves only the shell's `--sidebar-width` and the edge's `aria-valuenow`; React state and `localStorage` are written once on release (and once per keyboard step). Evidence: `src/sidebar-width.test.tsx` "moves the edge without re-rendering the app, and commits once on release" (React `Profiler` sees no commit during five moves, exactly one on release; one `setItem`) and "writes the width once per keyboard step".
- Lost mouseup: window blur ends the drag and keeps the width reached; Escape puts the starting width back; unmounting mid-drag clears the col-resize cursor. Evidence: three tests in "sidebar width while dragging".
- Narrow windows: the shown width is at most `min(400, window width − 480)`, never below 210, and follows window resizes without changing the stored preference; a drag that ends where it started does not overwrite it. Evidence: "sidebar width on a narrow window" (2 tests).
- Drop target: `.folder-row[data-drop-edge="into"] .folder-actions` takes the row's `--side-selected` fill. Disabled rows never turn yellow (`:not(:disabled)` on the row hover rule; their action fill is `--sidebar`). Actions show on `.folder-row:focus-within`. Evidence: `src/sidebar-stylesheet.test.tsx` (3 new tests, 1 updated).
- The App.css and TaskSidebar.tsx comments about where the edge sits now agree: inside the sidebar in the markup, positioned against the shell.
- `pnpm typecheck` passes; `pnpm test` 35 files, 1074 passed, 4 todo. Native drag feel and visuals still to be checked by the owner.
