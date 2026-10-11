# Tasknotes — Accepted product decisions

Date: 2026-10-11
Stage: planning / grilling, not implementation.

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

## Still to resolve in grilling

- Exact AI interface contract, authorization model, concurrency semantics and audit retention.
- Task/notes interaction details, default views and first-run/empty-state design.
- Status transition edge cases and how progress-history entries interact with editing.
- Settings and asset packaging, audio consent and macOS permissions/shortcuts.
- Setup-matt-pocock-skills instruction-file choice (AGENTS.md vs CLAUDE.md) and final setup scaffolding.
- Acceptance criteria and testing thresholds before to-spec/to-tickets.
