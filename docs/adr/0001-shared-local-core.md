# Shared local-first application core

Status: accepted (2026-10-11)

Tasknotes will be a local-first desktop application whose business rules and SQLite access are provided by a shared Rust application core. The React/Tauri UI, local CLI, and MCP adapter will call that core rather than implementing independent task/notes rules or editing SQLite directly. We chose this over separate frontend and agent implementations because consistent validation, ordering, audit, and concurrency guarantees matter when both humans and AI perform CRUD; it also avoids a permanently listening local HTTP service. Consequence: UI and AI adapters need stable, versioned contracts and compatible core delivery even when the desktop window is closed.
