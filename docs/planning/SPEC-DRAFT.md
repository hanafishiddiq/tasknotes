# Tasknotes — Specification Draft

**Status:** DRAFT / test seams and ticket structure approved; Q46 status semantics pending confirmation.
**Date:** 2026-10-11
**Source of truth:** `docs/planning/DECISIONS.md`, `GLOSSARY.md`, and accepted ADRs.
**Destination after approval:** `.scratch/tasknotes-v1/spec.md` using the configured local Markdown issue tracker.

> This document is planning only. It deliberately does not authorize application implementation.

## Problem Statement

The user needs a fast, elegant macOS app combining many personal Tasks and Notes with reliable progress tracking and a first-class local AI interface. Common to-do interfaces hide progress, mishandle large lists, and make AI agents scrape UI state or edit ad hoc files. Tasknotes should be usable offline without a cloud account, preserve data on removal, and remain responsive with tens of thousands of items.

## Solution

Create a local-first Tauri v2 desktop application with a React/TypeScript interface, a shared Rust application core, and SQLite persistence. The same core handles task/note operations from Tauri UI, a local CLI, and a Model Context Protocol (MCP) stdio server. Core invariants must not be reimplemented by each adapter.

The desktop experience combines a minimal list of Tasks, an independent rich-text Notes workspace, lightweight motion and gentle sounds, quick capture, search, consistent sorting, and accessible keyboard operation. AI gets structured, versioned CRUD access, with explicit privacy controls, revision protection, and auditability.

## User Stories

### Task capture and navigation

1. As a user, I want All Tasks at startup so I can immediately see every active task.
2. As a user, I want Inbox so I can capture unsorted Tasks quickly.
3. As a user, I want Lists so I can group Tasks by area of work.
4. As a user, I want labels/tags so I can categorize Tasks across Lists.
5. As a user, I want an optional deadline so I can see when work is due.
6. As a user, I want an overdue indicator so overdue work is discoverable without changing my fixed status groups.
7. As a user, I want fast Task creation so I can capture work without a complex form.
8. As a user, I want an expandable Task row so I can inspect a description and edit details inline.
9. As a user, I want only one Task expanded at a time so my context stays clear.
10. As a user, I want the scroll position preserved after expansion so I do not lose my place.
11. As a user, I want empty states without fabricated Tasks so I can start clean.
12. As a user, I want search and structured filters so I can find Tasks without manually browsing Lists.
13. As a user, I want priority-based and other sort options within work-status groups so the important work rises to the top.

### Work status, priority, and progress

14. As a user, I want the three Task groups displayed in the fixed order In Progress, Ready, Completed.
15. As a user, I want five priority levels—Very Low, Low, Medium, High, Very High—visible through both symbols and colors.
16. As a user, I want Medium priority by default so a new Task has a sensible initial state.
17. As a user, I want a manual progress percentage from 0–100 so my In Progress work is not opaque.
18. As a user, I want a Next Step field so I can resume a Task across sessions.
19. As a user, I want dated Progress Entries so I can see what changed and when.
20. As a user, I want progress at 100% to automatically mark a Task Completed.
21. As a user, I want changing a Ready Task's progress to 1–99% to move it to In Progress.
22. As a user, I want lowering a Completed Task below 100% to reopen it.
23. As a user, I want a manually reopened Completed Task to regain its previous incomplete percentage.
24. As a user, I want the displayed progress and status to remain consistent under both UI and AI edits.

### Notes, links, and Removed

25. As a user, I want independent Notes so I can write even when there is no associated Task.
26. As a user, I want Markdown-backed rich-text editing so notes are readable and portable without writing raw Markdown.
27. As a user, I want headings, emphasis, checklists, bullet lists, and links so my Notes can represent ordinary working documents.
28. As a user, I want autosave so typing in Notes does not require manual Save actions.
29. As a user, I want folders and pinned Notes so my Notes workspace stays organized.
30. As a user, I want a Note linked to multiple Tasks and a Task linked to multiple Notes without duplicating data.
31. As a user, I want unlinking to remove only the relationship, not the Note or Task.
32. As a user, I want removing a Task, Note, or List to place it into Removed rather than erase it.
33. As a user, I want restoring from Removed to preserve the prior details and work status.
34. As a user, I want a Removed List's Task children hidden but not individually removed.
35. As a user, I want restoring a List to leave independently Removed child Tasks in Removed.
36. As a user, I want Removed items excluded from ordinary search unless I explicitly include them.

### Desktop experience and sound

37. As a user, I want a sidebar with All Tasks, Inbox, Lists, Notes, Removed, and Settings so navigation is predictable.
38. As a user, I want light and dark themes so the app matches my environment.
39. As a user, I want keyboard shortcuts and a Cmd+K command palette so common actions are fast.
40. As a user, I want an optional global shortcut for Quick Capture so I can add a Task from another app.
41. As a user, I want optional menu-bar mode so I can keep Tasknotes available without a visible window.
42. As a user, I want closing the window to quit by default, with menu-bar persistence only when enabled.
43. As a user, I want subtle motion in Task expansion, progress bars, and status changes, without slow transitions.
44. As a user with Reduce Motion enabled, I want nonessential animations suppressed.
45. As a user, I want interaction and ambient sounds enabled at quiet default levels, with instant mute and independent controls.
46. As a user, I want ambient audio to continue if the enabled app is merely minimized but stop when it quits.
47. As a user, I want sound settings preserved, and audio assets to work without the internet.

### AI, concurrency, privacy, and resilience

48. As a trusted local AI client, I want to list, search, create, update, and move Tasks to Removed via structured interfaces.
49. As a trusted local AI client, I want CRUD and restore for Notes and Lists where authorized.
50. As a trusted local AI client, I want one stable, versioned JSON envelope with pagination cursors and revisions so I can reliably process large outputs.
51. As a trusted local AI client, I want access while the main desktop window is closed without requiring an always-running web server.
52. As a user, I want to exclude selected Notes from AI reads, searches, metadata, links, and writes.
53. As a user, I want an AI write based on an out-of-date revision to fail clearly rather than overwrite a recent UI edit.
54. As a user, I want AI and UI changes recorded in append-only Audit Entries so unexpected edits can be investigated.
55. As a user, I want high-risk bulk changes to present a preview and require confirmation.
56. As a user, I want local automatic backup snapshots so app failures or bad edits can be recovered.
57. As a user, I want a restore to validate a backup before replacing the active data.
58. As a user, I want JSON and Markdown export so I can migrate or examine my own information.
59. As a user, I want predictable handling of crash/restart and concurrent CLI/UI access so my data remains consistent.
60. As a user, I want the app tested with 50,000 Tasks and 10,000 Notes to ensure scalability is demonstrated rather than assumed.

## Implementation Decisions

### Core modules and interfaces

- **Application core (Rust):** owns Task, List, Note, Note Link, Removed, Progress Entry, Audit Entry, query, backup/restore, validation, and transaction semantics. Keep interfaces small and capabilities deep.
- **Persistence adapter (SQLite):** stores domain records with versioned migrations, indexed queries/FTS, revision fields, and safe multi-process access. SQLite WAL allows readers alongside one writer; enforce short transactions, busy timeouts, and sensible checkpoint behavior.
- **Desktop adapter (Tauri):** invokes core operations. TypeScript presents UI and transient interaction state; it must not encode competing business rules.
- **CLI adapter:** local machine-readable CRUD, filtering, search and export. Provides deterministic exit statuses and stable structured errors.
- **MCP adapter:** stdio transport exposing approved tools through the same core, not a second database implementation. No port/listener or remote network API in v1.
- **Privacy policy:** enforced in core at the official AI-facing interface, including search results, links and metadata; it does not claim to protect a database from arbitrary same-user shell access.

### Data and behavior

- Domain entities: Task, List, Tag, Note, Note Folder, Task–Note Link, Progress Entry, Audit Entry, application Setting, Backup Manifest. Lists have an Inbox default.
- Task fields include stable ID, title, optional description, List, tags, deadline, work status, 1–5 priority, progress 0–100%, previous incomplete percentage, Next Step, creation/update timestamps, revision and Removed disposition.
- Work status values are In Progress, Ready, and Completed; Removed is a separate dimension. The only status-group ordering is In Progress → Ready → Completed, including when the user changes a sort option.
- A new Task starts Ready with 0% progress and Medium priority. Progress 100% means Completed. A change from Ready to 1–99% means In Progress. Reducing progress on a Completed Task reopens it; an explicit reopen restores the prior incomplete percentage.
- Progress Entries and meaningful Audit Entries are append-only. Expanding Task rows shows a summary rather than loading every entry.
- Notes store canonical Markdown; rich-text editor must round-trip supported formatting without data loss. Note-to-Task relationships are many-to-many and non-owning.
- Removing a List hides its child Tasks without mutating their own Removed disposition. Restore exposes children except those separately Removed.
- A note marked Exclude from AI must not be exposed in AI-facing search, listing, direct access, metadata or task-note linkage. From the AI perspective, a private Note must behave like a non-discoverable resource.
- User-visible search covers Task title, description, Next Step, dated progress notes, and Note content. Official AI search returns only authorized matches.
- Paginated list/search responses include `schema_version`, `items`, `next_cursor`, and revision metadata; page-size default is 50. A versioned error envelope uses stable identifiers (e.g., revision conflict, invalid input, unavailable storage).
- Mutations enforce revision checks atomically; avoid silent last-writer-wins overwrite.
- Backup operates whenever the app or local CLI/MCP is active, using consistent SQLite snapshot mechanisms, with 7 daily, 4 weekly and 3 monthly retained backups. No background scheduler is promised when all Tasknotes processes are closed. Validate restore inputs, stop/suspend conflicting writers, and take a safe pre-restore snapshot.
- Note that retention of backup *copies* is independent of the user's no-permanent-deletion rule for live Tasks, Notes and Lists.

### UI and performance

- Default All Tasks screen; expandable/virtualized Task rows; each priority has a redundant label and icon, not color-only meaning.
- Use indexed queries, cursor-based paging, and virtualized row rendering with dynamic height measurement; do not render every row.
- Search benchmark target: p95 below 250 ms on the verified target Mac and synthetic test dataset. Scrolling target: near 60 FPS, measured and reported.
- Keyboard navigation, Cmd+K, light/dark themes, Undo, macOS Reduce Motion, and clear focus states.
- Separate sound controls: interaction sound initial volume 12%, ambient initial volume 8%, both enabled by default with one-step mute. Gracefully handle WebView autoplay restrictions rather than playing unexpectedly loud fallback audio.
- Third-party ambient/audio assets are never bundled before their licenses and redistribution conditions have been reviewed.

## Testing Decisions

**Approved test seams (Q48):**
1. **Primary: shared application-core interface** against an isolated temporary SQLite database. Exercise the same operations, invariants, privacy filtering, and error behavior that every adapter must observe.
2. **Adapter contracts:** run the CLI and stdio MCP adapters as separate processes against controlled temporary data, validating JSON envelopes, cursor paging, revisions, error codes, and Exclude from AI behavior.
3. **Desktop end-to-end:** exercise core capture, edit, sort, Notes, Removed/restore, keyboard accessibility, and failure states in the real macOS Tauri shell.
4. **Nonfunctional:** synthesize 50k Tasks and 10k Notes and record query latency p95 and scroll FPS, with device, scenario, warm/cold condition and methodology. Verify CPU/audio behavior, startup and minimized-window state.
5. **Data safety:** fault-injection around transactions, concurrent UI/AI writes, backup-and-restore recovery, privacy leaks, Markdown editor round-trip, malformed/untrusted input, and migration/upgrade.

Prefer high-seam behavioral tests over tests of private helpers or the exact directory structure. All fixtures must be synthetic and safe for a public repository. The initial source repository has no existing test suite to inherit.

## Acceptance Criteria

- Users can create, edit, find, sort, complete, reopen, remove, and restore Tasks; use separate Notes; and link entities as specified.
- Fixed status-group order holds across all sort options. Priority, progress, and restore invariants match accepted rounds.
- Desktop and agent operations share the same rules, with verified conflicts, audit history and privacy safeguards.
- Search/query performance and scrolling behavior are benchmarked on the target machine against the agreed 50k/10k dataset; missed targets are disclosed and fixed before claiming release readiness.
- Local backups and restores are tested with corrupted input, interrupted writes, and active client conflicts.
- Audio behavior, accessibility, keyboard navigation, and optional menu-bar/shortcut operation are verified on macOS.
- No sensitive user content, database, backups, or unlicensed third-party assets are included in the public repository or packaging.
- Each vertical ticket has its own behavioral acceptance tests, a review, and evidence before considered complete.

## Out of Scope

- Cloud account, multi-device cloud sync or hosted API.
- Always-running local web server, unsolicited network exposure, remote AI proxy and mobile clients.
- Reminders/notifications, recurring Tasks, collaboration, shared workspaces and file/image attachments.
- Permanent deletion of Tasks, Notes or Lists in v1.
- Windows/Linux release and signed/notarized public distribution in the first macOS milestone unless separately approved.
- Circumventing macOS privacy controls or protecting data against software granted arbitrary local filesystem access.

## Further Notes / Proposed Clarifications

Q47–Q49 have been accepted. Q46 remains an open domain decision; do not treat its proposed resolution as authorized.

1. **Q46 pending:** Pausing work or leaving a Task untouched does not automatically change In Progress to Ready or erase progress. Recommended: once begun, it stays In Progress unless completed or explicitly reset. Whether a manual reset back to Ready should be allowed—and whether it also resets progress—must be confirmed.
2. **Q47 accepted:** AI integration is explicitly configured by the user for trusted local clients; CLI/MCP is not exposed over a network port. OS-level arbitrary same-user process access is outside the security guarantee.
3. **Test obligation (Q48):** Restore requires coordination so simultaneously running UI/CLI/MCP clients cannot continue writing stale database state.
4. Source-code modules and library picks should be decided within the relevant ticket that first needs them; avoid adding hypothetical adapter layers or redundant state managers now.
5. No external audio is shipped until a license-and-asset review and playback quality check passes.

### Official technical references

- SQLite WAL design and concurrency: https://sqlite.org/wal.html
- Tauri v2 global shortcut permissions: https://v2.tauri.app/plugin/global-shortcut/
- Dynamic-height list virtualization: https://tanstack.com/virtual/latest/docs/api/virtualizer
