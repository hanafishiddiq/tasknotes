# 14: Desktop interaction and accessibility polish

**What to build:** All Tasks default, expandable one-at-a-time rows, empty states, keyboard navigation/Cmd+K, light/dark mode, Undo, focus visibility and Reduce Motion. Test that inline edits and scroll state are preserved.

**Blocked by:** 05 — Indexed Task search and filters, 07 — Note organization and searching, 09 — Removed and restore

**Status:** ready-for-agent

**Implementation:** implemented, awaiting independent review (branch `feat/ticket-14-desktop-polish`; plan `docs/plans/ticket-14-desktop-polish-2026-10-26.md`).

## Acceptance criteria

- [x] Tasknotes opens All Tasks with navigation to Inbox, Lists, Notes, Removed and Settings; empty states contain no fake user Tasks. *(Settings view added; `src/desktop-polish.test.tsx` "opens All tasks and reaches every place…")*
- [x] Task details expand inline one-at-a-time without losing scroll position; inline edits and progress animation do not block keyboard input. *(scroll/focus kept on open and close; unsaved edits survive scrolling away and opening another Task; progress fill is a CSS transform that never disables a field)*
- [x] Cmd+K, standard keyboard navigation, clear focus indicators and Undo cover the agreed major actions. *(palette, ⌘N/⌘F/⌘,/⌘Z, ↑↓/Home/End/Return/Esc; Undo for every move to Removed; stylesheet focus guard)*
- [x] Light/dark mode, readable contrast, semantic priority symbols and macOS Reduce Motion are respected. *(Dark/Light/Match system; contrast ≥ 4.5:1 checked for both themes; priority icon + label unchanged; all motion behind `prefers-reduced-motion: no-preference`)*
- [x] Task rows use virtualized rendering with dynamic height measurement; responsive interaction under sizeable fixtures is demonstrated. *(TanStack Virtual; a 300-Task page keeps under 40 rows in the DOM and scrolls/keyboard-navigates to row 200+; 50k measurement is Ticket 17)*

## Verification

- [ ] Run macOS Tauri E2E navigation, keyboard, focus, screen-state and Reduce Motion checks.
- [x] Relevant tests pass and evidence is recorded here. *(see Comments, 2026-10-26)*
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-26 — implementer (Ticket 14), awaiting independent review.**

Evidence (implementer's own runs on the branch, integration tip a4621ca unchanged): `pnpm test` 17 files / 178 tests passed (159 before; 19 new in `src/desktop-polish.test.tsx`, `src/stylesheet-a11y.test.tsx`, `src/removed-notes.test.tsx`); `pnpm typecheck` and `pnpm build` clean. No Rust crate touched, so cargo was not re-run. One existing test (`organization.test.tsx`, 50-per-page) now reads the page size from the group count instead of counting DOM rows, because rows are virtualized.

Decisions (details in the plan):
- saya pilih TanStack Virtual 3.14.18, pinned, karena the spec names it for dynamic-height rows. Rows stay in normal flow with spacers so status groups keep their landmarks.
- saya pilih keeping the open row, rows with unsaved edits and the keyboard target always mounted karena virtualization must never discard typing or focus.
- saya pilih to show unsaved drafts again on reopen (with an "Unsaved edits" marker on the closed row) karena silently resetting typed text breaks "reversible by default".
- saya pilih Undo for every move to Removed (Task, List, Note, folder) plus ⌘Z outside text fields karena those are the destructive-looking actions; status/priority/progress are one control away and progress history is append-only.
- saya pilih 50-row pages unchanged karena Ticket 04 decided paging; virtualization makes a later change cheap.

For owner review: Undo bar placement (flat last row) and lifetime (until next navigation); palette look (flat sheet, no backdrop); "Unsaved edits" wording; 120ms fade-in on opening a Task; keeping 50-row pages.

Open: native macOS E2E was not run (WDIO harness lives on `test/native-wdio-e2e`; an isolated QA bundle was not cheap here). `/code-review` not run yet.

Native click-checklist for Han (synthetic data only, e.g. a fresh QA database):
1. Launch: All tasks opens; sidebar shows All tasks, Inbox, Lists, All notes, Removed and Settings at the bottom. With no data each place shows its empty state and no sample Tasks.
2. Settings → Appearance: Light, Dark, then Match system; toggle macOS System Settings → Appearance and see the app follow without restarting. Quit and reopen: the choice is kept.
3. With 100+ synthetic Tasks: scroll fast top to bottom; rows appear without gaps or jumps; group headings stay in order In Progress → Ready → Completed.
4. Scroll halfway, open a Task, type in its title without saving, scroll far away and back: the text is still there and the scroll position did not jump when opening/closing. Open another Task: the first row says "Unsaved edits"; reopen it and the text is back.
5. Keyboard only: Tab to a Task row, ↑/↓/Home/End move between rows (including off screen), Return opens, Esc closes and focus returns to the row. Every focused control shows the yellow focus ring (sort menu, property capsules, progress note).
6. ⌘K opens the palette; type a List name and Return; Esc closes and returns focus. Type free text and pick "Search for …". Check ⌘N, ⌘F, ⌘, — and that macOS menu items do not swallow them.
7. Move a Task, a List, a Note and a Note folder to Removed; each time Undo (button) or ⌘Z (with focus outside a text field) brings it back. In a text field ⌘Z undoes typing only.
8. System Settings → Accessibility → Display → Reduce motion ON: opening a Task, hover fades, progress fill and spinners no longer animate; everything still works.
