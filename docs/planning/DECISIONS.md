# Tasknotes — Accepted product decisions

Date: 2026-10-11
Stage: planning approved, specification and tickets published; implementation not started.

## Core experience

- Desktop macOS first; keep domain logic portable for future Windows/Linux.
- Tauri v2, React/TypeScript, Rust, SQLite; local-first and functional offline.
- Tasks expandable in place to reveal and edit details; quick add; global search.
- Lists/projects, Inbox, tags, optional deadlines and overdue indicators.
- Three work statuses: In Progress, Ready, Completed. These groups retain this exact top-to-bottom order regardless of sorting; sort only within each group, with priority as initial default.
- Five priority levels: Very Low, Low, Medium, High, Very High; intuitive color and icon.
- 0–100% progress plus dated progress history and next-step text. Completed means 100%.
- Standalone rich-text notes with folders, pinned notes, headings, lists, checklists, links, and autosave; optional task-note links.
- Removed is reversible, with no automatic purge, for tasks, notes, and lists. Removing a list hides its active contents without destroying those tasks; restoring recovers the preserved state.
- Undo where appropriate, keyboard shortcuts, Cmd+K palette, light/dark themes, accessibility and Reduce Motion.
- macOS optional menu-bar mode and system-wide quick-capture shortcut.

## AI and privacy

- Local CLI/MCP access works without the main Tauri window open.
- AI has full CRUD on tasks and notes (including soft-remove/restore), with audit logs and concurrency safeguards.
- AI does not physically erase user data; high-risk batch operations require additional safeguards.
- Per-note Exclude from AI control applies to reading and writing through AI interfaces.
- AI outputs should be structured, concise, deterministic, and suitable for agents.

## Data lifecycle, performance and experience

- Automatic local backup, restore, JSON and Markdown export; cloud sync excluded from initial version.
- Performance is an acceptance test, not a promise: 50,000 tasks and 10,000 notes, indexed/paginated access and virtualized UI.
- Light micro-animation, responsive interactions, no motion when Reduce Motion is enabled.
- Interaction and ambient audio **both default ON at quiet volume** with separate volume controls and an OFF option.
- Ambient playback may continue when the window is minimized if enabled, and stops when the application actually quits.
- Review licenses, loop quality, and redistribution requirements of all third-party sounds before bundling any assets.

## Project and delivery

- Directory: user's agreed Playground location; repo name: tasknotes.
- Public GitHub repo approved; local Markdown tracker under .scratch/ approved.
- Follow Matt Pocock skill workflow: grill-with-docs → to-spec → to-tickets → implement/TDD → code-review.
- Repository publication must contain only generic code and planning material, never real task/note data, backups or secrets.

## Round 4 — Accepted (2026-10-11)

- Use `AGENTS.md` as the repository's main engineering instruction file.
- Configure `/setup-matt-pocock-skills` with local Markdown tracking, canonical triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`), one root glossary, and root ADRs.
- Keep the public GitHub repository **without a source-code license for now**; public visibility is not an open-source grant.
- All official AI and desktop adapters use the same Rust application core. CLI/MCP support structured operations, filters, pagination and consistent JSON results; no always-running localhost HTTP service in v1.
- Exclude from AI protects both data and metadata at official AI interfaces; explicitly **not** a guarantee against software with arbitrary filesystem/shell access.
- Every edit must handle concurrent changes using a revision token and optimistic concurrency; stale writes fail clearly rather than silently overwriting.
- Completing a task sets 100%; reopening restores the last in-progress percentage, and changes are recorded in progress history.
- Default interaction audio ON at 12% and ambient audio ON at 8%. Each can be muted and adjusted independently; persisted per-device settings retain volume preferences.
- Markdown is the canonical notes storage format; rich-text/WYSIWYG interaction is an editor presentation layer with stable format round-tripping.


## Round 5 — Accepted (2026-10-11)

### Desktop UX
- **Q33:** Default landing view is All Tasks. Sidebar shows Inbox, Lists, Notes, Removed, and Settings. Empty states invite creation without fabricated sample tasks.
- **Q34:** At most one Task expands inline at a time; details can be edited without leaving the list. Preserve scroll position and keep transitions light.
- **Q35:** Setting progress to 100% completes the Task; increasing a Ready Task to 1–99% switches it to In Progress; lowering a Completed Task below 100% reopens it. Preserve prior in-progress percentage when manually reopening as agreed in Round 4.
- **Q36:** Removing a List hides child Tasks without separately removing them. Restoring the List does not restore any child that was independently Removed.
- **Q37:** Task–Note links are many-to-many; unlinking never destroys either entity.

### Priority, search, AI access
- **Q38:** Priority design uses Very Low = grey, Low = blue, Medium = yellow, High = orange, Very High = red, with symbols/labels in addition to color. Medium is the default.
- **Q39:** Global Search covers Task title, description, progress notes, and Note content. Filters: work status, priority, List, tags. Removed items are omitted from normal search unless explicitly included.
- **Q40:** AI-facing output uses versioned structured JSON with `schema_version`, `items`, `next_cursor`, revision metadata, and stable error codes; default page size is 50.
- **Q41:** Change history is append-only. Individual AI CRUD works directly, while high-risk bulk mutations require a preview and confirmation mechanism.

### Backup, sound, system integration and performance
- **Q42:** Local automatic backup retention is 7 daily / 4 weekly / 3 monthly snapshots. Restore validates the snapshot before applying changes.
- **Q43:** Ambient audio must run offline using third-party assets only after the license and suitability are verified. Provide instant mute and benchmark playback, CPU and minimized-window behavior.
- **Q44:** Closing the window quits by default; optional menu-bar mode keeps the app alive. Global shortcut is user-configurable if it conflicts with existing shortcuts.
- **Q45:** Benchmark 50,000 Tasks and 10,000 Notes on the target Mac, aiming for search p95 <250 ms and scrolling close to 60 FPS. Measure and report results instead of claiming performance without evidence.

## Round 6 — Partial acceptance (2026-10-11)

- **Q47 accepted:** Local AI access is configured explicitly for trusted local clients; subsequent authorized CRUD does not require per-operation confirmation, except previously agreed high-risk bulk operations. No network-exposed service in v1.
- **Q48 accepted:** Test at the shared Rust application-core interface, CLI/MCP process contracts, and real macOS Tauri end-to-end level. Include privacy, optimistic concurrency, backup/recovery, performance, and accessibility.
- **Q49 accepted:** Proposed 18 vertical tickets and the dependency graph in `docs/planning/TICKETS-PROPOSAL.md` are approved in principle. Publishing final issue files waits until the remaining task-state semantics are settled.
- **Q46 accepted:** Ready means a Task that has not started; once work begins, the Task remains In Progress with its saved progress even during inactivity or a temporary pause. Neither inactivity nor closing/reopening Tasknotes automatically moves it to Ready. Do not add a Paused work status. Any explicit future "start over" operation must be deliberate and preserve historical Progress Entries; no implicit reset or data loss is authorized.

## Planning completion and implementation constraints

**All product questions Q1–Q49 are resolved and the 18-ticket plan is approved.** The canonical implementation specification and ticket graph are published under `.scratch/tasknotes-v1/`. The ticket-level implementation choices (e.g. exact shortcuts, audio fallback, restore locking, and benchmark harness) may be resolved in their own implementation tickets without inventing new product requirements.

Planning completion does **not** mean the application is implemented. The user requested planning first; start implementation only upon a subsequent explicit instruction to enter the implementation phase.
