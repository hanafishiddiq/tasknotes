# Tasknotes

A local-first macOS desktop app for tasks and notes, with a machine-friendly interface for trusted AI tools.

> **Project stage:** Ticket 01 is implemented on the dedicated feature branch. The remaining 17 tickets are not implemented.

## Agreed direction

- Tauri v2 desktop shell, React + TypeScript frontend, Rust core, SQLite persistence.
- Tasks organized in lists with tags, five priorities, progress notes, search, and keyboard-friendly interactions.
- Task groups always sort: **In Progress → Ready → Completed**; sorting happens within each group.
- Tasks, notes, and lists can be moved to **Removed** and restored; routine deletion does not physically erase them.
- Standalone rich-text notes and optional links between notes and tasks.
- AI-accessible CRUD via local CLI/MCP, including an audit trail and opt-out for notes excluded from AI.
- Local backup/restore and JSON/Markdown export.
- Fast, accessible UI with light micro-interactions and quiet, default-on audio that can be disabled.
- Performance to be *tested* with 50,000 tasks and 10,000 notes, not presumed.

## Engineering approach

Follow the locally installed Matt Pocock skill workflow: **grill-with-docs → to-spec → to-tickets → implement (TDD) → code-review**. Prefer deep, testable modules and narrow interfaces over unnecessary abstraction. Local Markdown issue tracking is the agreed issue-tracker model.

## Ticket 01: run the desktop preview

The first vertical slice implements **create, list, expand/edit and persistent local Tasks**. Every new Task starts in **Ready**, at **Medium** priority and **0%** progress. Later tickets add status/priority editing, Notes, search, ambient audio and AI integrations.

Required: macOS with Xcode Command Line Tools, Node.js, pnpm, and Rust (`cargo` available in PATH).

```bash
pnpm install --frozen-lockfile
pnpm tauri dev
```

The application uses a local SQLite file inside the OS-managed Tauri application-data directory (bundle identifier `store.adamaja.tasknotes`). **Do not store personal Task data in the repository.** The initial Task list fetches no more than 50 Tasks and exposes a Load More button for pagination.

### Development tests

```bash
pnpm typecheck
pnpm test
pnpm build
cargo test --manifest-path crates/tasknotes-core/Cargo.toml
cargo fmt --manifest-path crates/tasknotes-core/Cargo.toml --all -- --check
pnpm tauri build --debug --bundles app --no-sign --ci
```

For an isolated debug run against synthetic data, create a temporary directory and set `TASKNOTES_DEV_DATA_DIR` before starting `pnpm tauri dev` or the debug app binary. This override exists **only in debug builds**; release builds always use the standard private app-data directory. Keep database, backups and generated logs out of the public repository.

See [planning decisions](docs/planning/DECISIONS.md) and [domain terms](GLOSSARY.md).

**Planning complete:** [Approved v1 specification](.scratch/tasknotes-v1/spec.md) · [18 implementation tickets](.scratch/tasknotes-v1/README.md) · [Architectural decisions](docs/adr/). Ticket 01 is the only authorized implementation so far.

## Privacy

This repository is public, **but user tasks, notes, SQLite databases, backups, private exports, secrets, and ambient audio assets with unverified redistribution rights must never be committed**. Never use real personal content in fixtures or screenshots. Development samples must be synthetic.

## Scope

Ticket 01 supplies an executable macOS desktop preview and minimal persistent Task CRUD. The remaining approved capabilities are scheduled in Tickets 02–18 and are intentionally not claimed as implemented.
