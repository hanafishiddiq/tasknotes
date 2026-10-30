# 14: Desktop interaction and accessibility polish

**What to build:** All Tasks default, expandable one-at-a-time rows, empty states, keyboard navigation/Cmd+K, light/dark mode, Undo, focus visibility and Reduce Motion. Test that inline edits and scroll state are preserved.

**Blocked by:** 05 — Indexed Task search and filters, 07 — Note organization and searching, 09 — Removed and restore

**Status:** ready-for-agent

**Implementation:** review findings fixed; re-review done on 2026-10-30 and its findings fixed; awaiting the owner's native check and merge decision; on `integration/tasknotes-v1-07-18` (Draft PR #20). Plan: `docs/plans/ticket-14-desktop-polish-2026-10-26.md`.

## Acceptance criteria

- [x] Tasknotes opens All Tasks with navigation to Inbox, Lists, Notes, Removed and Settings; empty states contain no fake user Tasks. *(Settings view added; `src/desktop-polish.test.tsx` "opens All tasks and reaches every place…")*
- [x] Task details expand inline one-at-a-time without losing scroll position; inline edits and progress animation do not block keyboard input. *(scroll/focus kept on open and close; unsaved edits are kept above the rows per Task with the revision they were typed against, so they survive scrolling, paging, filters, sort and opening another Task, and a change made elsewhere is refused rather than saved over; progress fill is a CSS transform that never disables a field)*
- [x] Cmd+K, standard keyboard navigation, clear focus indicators and Undo cover the agreed major actions. *(palette, ⌘N/⌘F/⌘,/⌘Z, ↑↓/Home/End/Return/Esc; stylesheet focus guard. The agreed major actions Undo covers: moving a Task, a List, a Note or a Note Folder to Removed (the last 10, newest first, across views), plus the existing unlink of a Task–Note link. Not covered, by decision: status, priority, progress, tags/deadline and List moves, which are one control away and whose progress history is append-only)*
- [x] Light/dark mode, readable contrast, semantic priority symbols and macOS Reduce Motion are respected. *(Dark/Light/Match system; contrast ≥ 4.5:1 checked for both themes; priority icon + label unchanged; all motion behind `prefers-reduced-motion: no-preference`)*
- [x] Task rows use virtualized rendering with dynamic height measurement; responsive interaction under sizeable fixtures is demonstrated. *(TanStack Virtual. What is shown here is only DOM-row bounds in jsdom: a 300-Task page keeps under 40 rows in the DOM and scrolls/keyboard-navigates to row 200+. No timing or frame rate was measured; that is Ticket 17 and the native checklist below)*

## Verification

- [ ] Run macOS Tauri E2E navigation, keyboard, focus, screen-state and Reduce Motion checks.
- [x] Relevant tests pass and evidence is recorded here. *(see Comments, 2026-10-26)*
- [x] Run `/code-review` and address findings before marking complete. *(see Comments, 2026-10-26 — review fixes)*

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

For owner review (updated by the review fixes below): Undo bar placement (flat last row) and lifetime (bar leaves on navigation; the last 10 moves stay undoable by ⌘Z/palette); palette look (flat sheet, no backdrop); "Unsaved edits" wording; the "changed elsewhere" notice and its Discard my edits / Keep my edits choice; unsaved Task edits kept for the session only (lost on quit); 120ms fade-in on opening a Task; keeping 50-row pages.

Open: native macOS E2E was not run (WDIO harness lives on `test/native-wdio-e2e`; an isolated QA bundle was not cheap here). `/code-review` not run yet.

Native click-checklist for Han (synthetic data only, e.g. a fresh QA database):
1. Launch: All tasks opens; sidebar shows All tasks, Inbox, Lists, All notes, Removed and Settings at the bottom. With no data each place shows its empty state and no sample Tasks.
2. Settings → Appearance: Light, Dark, then Match system; toggle macOS System Settings → Appearance and see the app follow without restarting. Quit and reopen: the choice is kept.
3. With 100+ synthetic Tasks: scroll fast top to bottom; rows appear without gaps or jumps; group headings stay in order In Progress → Ready → Completed.
4. Scroll halfway, open a Task, type in its title without saving, scroll far away and back: the text is still there and the scroll position did not jump when opening/closing. Open another Task: the first row says "Unsaved edits"; reopen it and the text is back. Change the sort or a filter and come back: still there. Then rename that Task with the CLI, change the sort to reload: the row shows "changed elsewhere"; Save changes is refused and the CLI's title stays; Discard my edits shows the CLI's title.
5. Keyboard only: Tab to a Task row, ↑/↓/Home/End move between rows (including off screen), Return opens, Esc closes and focus returns to the row. Every focused control shows the yellow focus ring (sort menu, property capsules, progress note).
6. ⌘K opens the palette; type a List name and Return; Esc closes and returns focus. Type free text and pick "Search for …". Check ⌘N, ⌘F, ⌘, — and that macOS menu items do not swallow them.
7. Move a Task, a List, a Note and a Note folder to Removed; each time Undo (button) or ⌘Z (with focus outside a text field) brings it back. Remove two Tasks, visit Inbox and come back, press ⌘Z twice: the second comes back first. In a text field ⌘Z undoes typing only. With ⌘K open, ⌘N/⌘F/⌘, do nothing behind it.
8. System Settings → Accessibility → Display → Reduce motion ON: opening a Task, hover fades, progress fill and spinners no longer animate; everything still works.

**2026-10-26 — review fixes (implementer, Ticket 14).** Status unchanged.

What each `/code-review` finding became:
1. Data safety: unsaved edits are held per Task in an App-level store (`src/lib/use-task-drafts.ts`) with the revision they were typed against; every save (title/description, progress, tags/deadline) names that revision. A draft whose Task changed elsewhere shows "changed elsewhere" with Discard my edits / Keep my edits (keep carries the typed fields onto the newer version; untyped fields show what is saved now). No draft field resets on a revision change any more.
2. Drafts survive paging, filters, sort, scrolling and other views (store above the rows); rows no longer need to stay mounted for them.
3. Undo is a stack of the last 10 moves to Removed, newest first, kept across views (`src/lib/use-undo.ts`); restoring an item from Removed drops it from the stack. DESIGN.md Undo Notice updated; its CSS placement (last grid row, in flow) already matched.
4. Palette: renders in a portal, the app shell is `inert` while it is open, ⌘N/⌘F/⌘,/⌘Z are swallowed behind it; the empty state is a `role="status"` paragraph outside the listbox.
5. VirtualTaskList: focus target cleared once focus lands (no `seq`), the focused row is pinned, headings found by `data-heading`, no `as Item` casts, the dependency-free layout effect documents why.
6. TaskRow's dirty-reporting effect is gone (the store knows); the marker is plain italic text, now class `unsaved-marker`, with a stylesheet guard against a background/border.
7. One `SHORTCUTS` table drives Settings and `useShortcuts`; `APPEARANCES`/`APPEARANCE_LABELS` from `use-theme`; `buildCommands` in `src/lib/app-commands.ts`; one `busy` guard; `void runUndo()` gone.
8. `scrollToIndex` names `behavior: "auto"`; a test fails if any script scroll asks for `smooth` (checked red by setting it to smooth).
9. The virtualization criterion now says only DOM-row bounds in jsdom are shown here.

Evidence (implementer's own runs, after merging integration tip a73f031, Ticket 10, which touched no frontend file): `pnpm test` 17 files / 186 tests passed (178 before the fixes; 8 new: three draft/revision tests, Undo stack, palette inert, palette empty state, instant scroll, marker guard); `pnpm typecheck` and `pnpm build` clean; `cargo check` in `src-tauri` clean. Native macOS checks still not run.

Not done: after a refused save the window does not reload the Task by itself (the error banner shows; the notice appears on the next list refresh), as before this ticket. Unsaved edits are not kept across quitting the app.

**2026-10-30 — re-review outcome (implementer).** Each fix went red first against the mocked Tauri seam (`src/undo.test.tsx`):
1. A refused Undo shows at once while older moves remain (`01bfc7e`). Saya pilih the failure takes the Undo bar until dismissed, then the older move's notice returns, because both bars share the window's last grid row and would overlap. The refused move stays off the stack, as before; it is still in Removed.
2. A refused Task or List Undo shows in every view (`c25e747`): both now reject instead of reporting through the Task view's banner, which is hidden in Settings, Removed and Notes.
3. Undo refreshes the view showing now (`cacaa4c`): it reads the current page, not the cursor from when the Task was removed. After going to another view the refresh is no longer refused; after going back a page, page 1 stays with its controls; still on the same page, it stays there.
4. No false "Changed elsewhere" for your own save (`e18c726`): the draft is cleared when the save lands, not after the list refresh.

Test gaps closed (`da246db`): a progress update, a percentage alone and tags/deadline each name the base revision, and restoring from Removed drops the item from the Undo stack. Mutation checks, each failing exactly its one test: `task.revision` in each of the three saves, `forget` as a no-op.

Not changed: "Change saved, but the ordered list could not refresh" and "List updated, but the sidebar could not refresh" still show only in the Task view's banner. The restore itself succeeded in those cases.

Evidence (implementer's own runs, synthetic data; these fixes have had no further independent review): `cargo test` core 204, CLI 45, MCP 34, src-tauri 42, all passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean in all four; `pnpm test` 261 passed in 25 files; `pnpm typecheck` and `pnpm build` clean. Native macOS checks are still the owner's.
