# Tasknotes — Specification, batch 2

**Status:** APPROVED by the owner on 2026-11-09 after a grilling session (Q1–Q32).
**Source of truth:** this file, `GLOSSARY.md`, ADR 0008 (Won't Do), ADR 0009 (nested Lists) and, once merged, ADR 0007 (manual sidebar order, PR #24).
**Issue tracker:** `.scratch/tasknotes-next/issues/`.
**Delivery:** every ticket lands on `main` first; the owner then gets **one** install build in `/Applications`, after which build caches and finished worktrees are removed. The owner authorized all tickets of this batch at once (Draft PR per ticket, AI review gate); merging, the install build, real data and cache removal each still need the owner's confirmation.

## Problem Statement

After using Tasknotes v1 daily, the owner listed friction points in the "Tasknotes Features" List: a new Task lands far down the list, saving needs the mouse, finished-but-abandoned work has nowhere to go, Tasks cannot be put in a personal order, Lists cannot be grouped, the sidebar is cramped and its hover is too faint, there is no way to rate how hard a Task is, there is no focus timer, and it is unclear whether muted rain still costs CPU.

## Solution

Ship eleven changes in one build: two finished Draft PRs (#23 outside changes appear live, #24 manual sidebar order) and nine features, each a vertical slice through the shared Rust core, the desktop UI, the CLI and MCP where it touches data.

## User Stories

### Capture and editing

1. As the owner, after adding a Task I want a quiet toast in the bottom-right corner ("Task added · Click to edit") that fades in and out, stays about 6 seconds and pauses while hovered, so I can jump to the new Task without scrolling.
2. As the owner, clicking that toast scrolls to the Task, opens it and flashes it yellow briefly. Adding several Tasks in a row shows one toast for the latest.
3. As the owner, I want ⌘↵ to save whichever edit group is open in a Task (title and description, progress update, or tags and deadline), with a small `⌘↵` hint on that Save button.

### Work status and attributes

4. As the owner, I want to mark a Task **Won't Do**: it is Completed, shown in the Completed group with a distinct mark, keeps its progress, and can be reopened to Ready or In Progress.
5. As the owner, I want an optional **Difficulty** (Very Easy … Very Hard, default No Difficulty), shown as a pill beside Priority only when set, usable as a filter and a sort.
6. As an AI client, I can read and set Difficulty and Won't Do through the CLI and MCP.

### Order and grouping

7. As the owner, I want to drag Tasks to reorder them within one work-status group while viewing a single List (Inbox included). Dropping into another group never changes status.
8. As the owner, when a drag breaks the current sort (Priority, Difficulty or Newest), the List switches to **Custom**, starting from the order that was on screen. Changing a Task's priority through its control while sorted by Priority only re-sorts.
9. As the owner, each List keeps its own Custom Order; switching to another sort and back restores it. A new Task lands at the top of its group in Custom.
10. Drag is disabled in search results, tag filters and views spanning several Lists.
11. As an AI client, I can read Tasks in Custom Order and move one Task after another (no Bulk Change approval), like PR #24's sidebar moves.
12. As the owner, I want Lists inside Lists, up to five levels, by dragging in the sidebar. A List shows only its own Tasks. Inbox stays at the top level and holds no Lists. Removing a List hides the Lists and Tasks inside it until restored; through AI that goes through Bulk Change approval when it holds anything.

### Sidebar

13. As the owner, I want a clearly visible pale-yellow hover on Lists and Note Folders.
14. As the owner, I want to resize the sidebar by dragging its edge (210–400 px, default 260 px), remembered across launches. When a name still does not fit, the hover action icons cover its end on the row's background instead of overlapping it.
15. As the owner, I want an icon and a colour for each List and Note Folder: 12 colours readable in light and dark themes, icons from the app's Lucide set with a search box. Existing emoji in names are left alone. The CLI and MCP can read and set them.

### Pomodoro

16. As the owner, I want a Pomodoro timer, optionally for one Task: 25 min focus, 5 min break, 15 min long break after every 4 sessions, all adjustable.
17. When focus ends: one macOS notification (silent when the Pomodoro's own **Silent** setting is on, the system sound otherwise; Silent is independent of interaction sounds and Ambient rain), and the menu bar, the notification and the window show the running Overtime ("🍅 +07:12") until I respond. When a break ends, another notification (silent likewise) offers **Start focus**, and the menu bar shows "☕ Done" until I respond. The menu bar shows the time left while running, and the timer keeps running with the window closed. The timer pauses while the Mac sleeps: sleep never counts as focus, break or Overtime.
18. Every switch (focus → break → focus) waits for my click. At the focus end I choose **Start break** (the break starts now, the Overtime is not counted) or **Count +07:12** (the label shows the live Overtime: I was still focusing, so the Overtime up to the click counts as focus, the session is recorded Finished with it and the break starts at once, in one click). There is no "keep focusing" state. Stop after the planned end records the session Finished without Overtime. Quitting (⌘Q) during a session saves it: Stopped before the planned end, Finished without Overtime once it was reached.
19. A finished session tied to a Task adds a Progress Entry to that Task ("Pomodoro: 25 min focus", or "Pomodoro: 25 + 7 min focus" only when Overtime was counted); it never changes the Task's status.
20. On first use Tasknotes reads the notification style; if it is not Persistent, it shows a one-time tip with a button opening the Tasknotes page in System Settings. No extra window, no repeating sound.
21. As the owner, I want a "Pomodoro" page with a daily history: per day the number of sessions, total focus minutes and each session's time and Task. Only finished sessions count; Stopped ones are listed but not counted. The CLI and MCP can read this history.
22. Pomodoro and Ambient rain are independent.

### Sound check

23. As the owner, I want evidence of what muted rain costs: CPU of the real app with (a) only rain muted and (b) rain and interaction sounds muted, against sound fully on. If (a) still keeps an idle audio device busy, report it with a proposed fix.

## Implementation Decisions

- **Shared core first** (ADR 0001): Won't Do, Difficulty, Custom Order, List nesting, icons/colours and Pomodoro sessions are core operations with revision checks; the UI, CLI and MCP call them, never re-implement them.
- **Schema versions** are assigned in merge order. PR #24 takes v16; each later schema ticket rebases and takes the next number at merge. Older binaries refuse a newer schema, as today.
- **Won't Do** is a resolution on a Completed Task (ADR 0008), recorded in Progress history like any status transition. Reopening clears it.
- **Difficulty** mirrors Priority's storage and ranking, but nullable; sort by Difficulty orders Very Hard first, No Difficulty last, then the existing tie-breakers.
- **Custom Order** is stored per List (Inbox included) as a position among Tasks of the same List; work-status grouping still comes first. The chosen sort per List is remembered.
- **Nested Lists** reuse PR #24's sibling ordering and the Note Folder depth limit of 5 (ADR 0009).
- **Sidebar width** is a per-device UI preference, not Tasknotes data.
- **Pomodoro** timing lives in the Rust shell so it survives a closed window; sessions are stored in the database so history, CLI and MCP share them. Notifications use the macOS notification permission (new for Tasknotes); the app reads but never changes the alert style.
- **Toast** and the yellow flash are view state only, never stored.

## Testing Decisions

- Core rules (Won't Do transitions, Difficulty sort, Custom Order moves and the switch to Custom, nesting depth and Inbox rule, Overtime accounting, session counting) get Rust unit tests at the core seam, plus CLI and MCP contract tests where exposed.
- UI behaviour (toast lifecycle, ⌘↵, drag within one group only, sidebar resize and hover, icon picker) gets Vitest + Testing Library tests; drag and resize also get a WDIO native check where the existing harness allows.
- Native-only checks (notification style, menu bar, CPU sampling) are recorded as evidence in their ticket, with the screen awake (see `docs/qa`), and synthetic data only.

## Acceptance Criteria

- Each ticket's criteria pass with recorded evidence, `/code-review` findings addressed, and its Draft PR reviewed.
- After all tickets merge: `pnpm test`, typecheck, build and `cargo test` green on `main`; one install build passes an owner click check on the real app.
- No permanent deletion, no network exposure, no cloud sync introduced; Exclude from AI still holds at the CLI and MCP.

## Out of Scope

- Assign Task to People (waits for multi-user work).
- Cloudflare D1 and web access (`issues/03`).
- Rolling sub-List Tasks up into a parent List; dragging Tasks across status groups to change status; a fourth work status.
- Any always-on-top Pomodoro window.
