# Tasknotes v1 — Tracer-Bullet Ticket Proposal

**Status:** DRAFT, not published to the official local tracker.
**Source:** `docs/planning/SPEC-DRAFT.md` and accepted Q1–Q45 decisions.
**Publish after approval:** one ticket per `.scratch/tasknotes-v1/issues/<NN>-<slug>.md`.
**Default ticket state after approval:** `ready-for-agent`; blocker edges determine what is actually ready to start.

Matt Pocock `/to-tickets` expects independently demonstrable **vertical slices** with explicit blocking edges. Each ticket below must ship its own test(s) across the meaningful user-visible interface, not merely add a data table or a React component.

## Proposed tickets

### 01 — First Task on a local desktop

**Blocked by:** None.
**Delivers:** Launch a minimal Tauri/macOS shell; create and edit a Task from UI; persist to local SQLite through Rust core; reload and find the same Task. Includes migration, minimal integration tests and synthetic-only fixtures.

### 02 — Work status, priority, and fixed ordering

**Blocked by:** 01.
**Delivers:** Change Task status and all five priority levels from UI, see intuitive symbols/colors and predictable In Progress → Ready → Completed grouping. Enforce 100%-is-Completed and progress-driven work status behavior in the core, not only React.

### 03 — Progress history and Next Step

**Blocked by:** 02.
**Delivers:** Record 0–100% progress with dated append-only Progress Entries and Next Step; view recent history on expanded Tasks; manually reopen Completed Tasks at the previous incomplete percentage. Test time ordering and transitions.

### 04 — Inbox, Lists, tags, deadlines

**Blocked by:** 01.
**Delivers:** Organize Tasks in Lists/Inbox, add tags and optional deadlines, show overdue indicators, and persist CRUD changes across restart. Test List ownership and relative date display.

### 05 — Indexed Task search and filters

**Blocked by:** 02, 03, 04.
**Delivers:** Search title, description, and progress notes; filter by work status, priority, List, and tag; maintain fixed status groups under alternate sorts, exclude Removed, and paginate via indexed core queries. Validate results from the UI.

### 06 — Standalone Markdown Notes

**Blocked by:** 01.
**Delivers:** Notes view and first Note end-to-end: rich-text editor creates/updates canonical Markdown, autosave persists and reloads; basic headings, lists, checklists, formatting and links round-trip safely. Uses a focused core Note interface.

### 07 — Note organization and searching

**Blocked by:** 06.
**Delivers:** Folders, pinned Notes, notes list/search, and global query coverage of Note content, with keyboard-ready navigation and reasonable page sizes. Verify Markdown content indexing and ordering.

### 08 — Reversible Task–Note links

**Blocked by:** 04, 06.
**Delivers:** Link one or more Notes to a Task and one Note to multiple Tasks, navigate the relationship from each view and unlink without destroying either resource. Test link integrity after edits.

### 09 — Removed and restore

**Blocked by:** 04, 06, 08.
**Delivers:** Move Tasks, Notes, and Lists into Removed instead of deleting; restore prior state. Removing a List hides child Tasks, and restoring it preserves any independently Removed children. Links remain valid. Normal search/list excludes Removed.

### 10 — Local CLI Task CRUD with concurrency and audit

**Blocked by:** 03, 05, 09.
**Delivers:** Structured JSON CLI Tasks CRUD, filtering, cursor pagination, revision-protected edits, stable error codes, and append-only Audit Entries. Operate while the desktop window is closed; race UI and CLI writers to prove no silent overwrite.

### 11 — Private, AI-optimized Note CRUD in CLI

**Blocked by:** 07, 08, 10.
**Delivers:** Extend the same local CLI schema to Notes and links; enforce Exclude from AI on get/list/search/write and relationship metadata; ensure private Notes are not leaked through error messages or audit/log output. Validate privacy in isolated command-line tests.

### 12 — MCP stdio integration and safe bulk mutations

**Blocked by:** 10, 11.
**Delivers:** MCP tools with same core operations and schema; no always-listening HTTP port; read/write from an authorized local client. For high-risk bulk operations, produce a preview and require an explicit confirmation step. Test the stdio protocol end-to-end.

### 13 — Backup, validated restore, and export

**Blocked by:** 09, 11.
**Delivers:** Automatic local backup snapshots (7 daily, 4 weekly, 3 monthly), validated restoration coordinated with other writers, and user-controlled JSON/Markdown export. Test corrupted backups, conflict handling and preservation of Removed data.

### 14 — Desktop interaction and accessibility polish

**Blocked by:** 05, 07, 09.
**Delivers:** All Tasks default, expandable one-at-a-time rows, empty states, keyboard navigation/Cmd+K, light/dark mode, Undo, focus visibility and Reduce Motion. Test that inline edits and scroll state are preserved.

### 15 — Menu bar and global Quick Capture

**Blocked by:** 14.
**Delivers:** Optional macOS menu-bar mode, close-to-quit default, configurable global Quick Capture shortcut, shortcut conflict handling and user-visible settings. Verify app lifecycle behavior on Mac.

### 16 — Quiet ambient and interaction sound

**Blocked by:** 14.
**Delivers:** Offline ambient and interaction sounds default ON at 8% and 12% respectively; separate sliders, mute, persistence, minimized-window behavior, quit behavior, graceful autoplay fallback. Include license and asset-quality review evidence before bundling audio.

### 17 — Large-dataset performance acceptance

**Blocked by:** 05, 07, 12, 14.
**Delivers:** Synthetic 50,000 Task/10,000 Note dataset; profile indexed search p95 with agreed <250 ms goal, virtualize and measure dynamically expanded rows, measure scrolling near 60 FPS and AI paging/memory. Fix measured bottlenecks rather than guessing.

### 18 — macOS integration and release-quality verification

**Blocked by:** 12, 13, 15, 16, 17.
**Delivers:** Run full end-to-end user/AI/privacy/recovery regressions, accessibility checks, app packaging and startup tests, backup restore drills, audio CPU assessment, and document evidence for each acceptance gate. Never claim release readiness for unverified checks.

## Cross-cutting guardrails

- Every Task list-related ticket (01 onward) must exercise realistic pagination and avoid unbounded UI rendering; Ticket 17 is the **final measured proof**, not permission to postpone fundamental performance design.
- Every mutation ticket must avoid destructive deletion and respect core invariants, even before the dedicated Removed UI is delivered.
- Integration code must keep AI/data privacy enforceable at the shared core, not as an easily bypassed frontend-only filter.
- Each ticket is implemented with TDD, behavior-level tests at the agreed seams, and a code review before being marked done.

## Dependency review

- **First unblocked slice:** 01.
- **After 01:** 02, 04, 06 may be developed independently on controlled branches.
- **After 02:** 03.
- **After 04 and 06:** 08 can proceed.
- **After 02, 03, 04:** 05.
- **After 06:** 07.
- **After 04, 06, 08:** 09.
- **After 03, 05, 09:** 10, then 11, then 12.
- **After 09 and 11:** 13.
- **After 05, 07, 09:** 14, enabling 15 and 16.
- **After 05, 07, 12, 14:** 17.
- **After all safety/performance/UX gates:** 18.

The graph favors user-visible behaviors instead of building a large abstract `database` layer, then `backend` layer, then `frontend` layer. Every slice has an end-to-end acceptance path.

## Questions to sign off before publication

1. **Granularity:** Are 18 small, demoable tickets an acceptable first-release breakdown, or should specific tickets be merged/split?
2. **Dependencies:** Does the proposed blocker graph match which capabilities must exist first?
3. **Test seams:** Are the shared Rust core, CLI/MCP subprocess contracts, and real macOS Tauri E2E tests the correct verification seams?
4. **Remaining policy:** May we adopt the recommendations listed in the specification's Further Notes, especially preserving partial progress when marking a Task Ready and requiring deliberate local AI trust setup?

Only after affirmative review, publish the final spec and individual tickets under the configured local Markdown tracker, then finish planning. Do not implement the application within this planning phase.
