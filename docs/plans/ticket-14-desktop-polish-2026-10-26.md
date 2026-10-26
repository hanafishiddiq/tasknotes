# Ticket 14 — Desktop interaction and accessibility polish: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at a4621ca (Tickets 06–09 done; schema v9). Frontend only: no schema, core or Tauri command changes, so it cannot collide with Ticket 10 (CLI and audit entries) running in parallel.

Scope is spec stories 37, 38, 39, 43, 44 and the virtualization line of "UI and performance". Stories 40–42 (global Quick Capture, menu bar, close-to-quit) are Ticket 15; story 45 (sound) is Ticket 16; the 50k/10k measurements are Ticket 17.

## What already existed

Sidebar with All tasks, Inbox, Lists, Notes and Removed; empty states with no fake Tasks; dark default plus a light toggle; global `:focus-visible` ring; motion behind `prefers-reduced-motion: no-preference`; Undo for a Task moved to Removed and for unlinking; arrow keys in the Note list; 50-row pages with Next/Previous (Ticket 04 deferred virtualization to this ticket).

## Decisions made by the implementer (owner may revisit)

- **saya pilih TanStack Virtual (`@tanstack/react-virtual` 3.14.18, pinned exact)** karena the spec links it for dynamic-height rows and it has no other dependencies. Version is about two weeks old.
- **Rows stay in normal flow with spacers**, not absolutely positioned, so each status group keeps its own `<section>` landmark and an opening row simply grows. Space above a later group is part of its heading, so it is measured.
- **The open Task, a row holding unsaved edits, and the row keyboard focus is moving to are always kept mounted**, so virtualization never discards typing or loses focus.
- **A hidden view keeps its last height** (and a row that measures 0 keeps its estimate), so visiting Notes or Settings does not unmount the open Task.
- **Reopening a Task shows what was typed and not saved** (previously reopening reset the drafts). A closed row with drafts says "Unsaved edits". Nothing typed is thrown away silently.
- **Paging stays at 50 per page with Next/Previous** (Ticket 04's decision). Virtualization is in place, so a larger page or continuous loading is a one-line change later if the owner wants it.
- **Undo covers every move to Removed**: Task, List, Note, Note Folder (plus the existing unlink Undo). One app-level notice, ⌘Z outside a text field, and a palette entry. Inside a text field ⌘Z stays the field's own undo. The notice leaves on the next navigation. Status, priority, progress and List moves are not given Undo: they are one control away from being set back, and progress history is append-only, so an "undo" would be a new entry anyway.
- **⌘K palette actions:** go to All tasks, Inbox, each loaded List, All notes, Removed, Settings; New task; Search; Undo (when there is one); Appearance Dark/Light/Match system; free text becomes "Search for "…"". **Shortcuts:** ⌘K, ⌘N (New task), ⌘F (search), ⌘, (Settings), ⌘Z (Undo), ↑/↓/Home/End between rows, Return to open/close, Esc to close the open Task.
- **Settings is minimal:** Appearance (Dark default, Light, Match system) and a list of shortcuts. The sidebar appearance toggle stays, as DESIGN.md describes. Tickets 15 and 16 add their settings here.
- **Focus, motion and contrast are guarded by a stylesheet test** (jsdom applies no CSS): every control that hides the native outline has a recorded replacement, all motion is behind `no-preference`, durations stay ≤ 240ms, and text tokens reach WCAG AA 4.5:1 in both themes. It found two real gaps (sort control and progress note had no visible focus), now fixed.

## For owner review (visual/product preferences not settled by DESIGN.md or the spec)

1. Undo notice as a flat bar in the window's last row (not floating), leaving on the next navigation.
2. Palette as a flat sidebar-gray sheet with a hairline edge, no dimmed backdrop.
3. "Unsaved edits" marker wording and italic style on a closed row.
4. Opening a Task fades in over 120ms (story 43 "subtle motion"); nothing slides.
5. Keeping 50-row pages rather than continuous scrolling.

## Vertical TDD slices (React seam, mocked Tauri invoke)

1. Settings view and appearance preference (Dark/Light/Match system, live system changes).
2. Navigation to every place with empty states and no made-up Tasks.
3. Virtualized rows: bounded DOM for a 300-Task page, scrolling brings later rows in.
4. Scroll position and focus survive open/close; an unsaved edit survives scrolling far away and opening another Task.
5. Keyboard navigation across groups and off screen; Return/Escape with focus handed back.
6. Undo for List (button and ⌘Z), Task (⌘Z ignored inside a text field), Note and folder.
7. ⌘K palette (filter, arrows, Escape returns focus, free-text search, Undo/appearance/Settings) and ⌘N/⌘F/⌘,.
8. Stylesheet guards for focus, Reduce Motion and contrast.

Native macOS checks (WDIO harness is on `test/native-wdio-e2e`, Draft #4) were not run here: an isolated QA bundle and synthetic DB were not cheap to set up in this worktree. The ticket carries a click-checklist for the owner instead.
