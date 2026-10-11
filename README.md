# Tasknotes

A local-first macOS desktop app for tasks and notes, with a machine-friendly interface for trusted AI tools.

> **Project stage: planning.** No application code has been implemented yet.

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

See [planning decisions](docs/planning/DECISIONS.md) and [domain terms](GLOSSARY.md).

Current review artifacts: [specification draft](docs/planning/SPEC-DRAFT.md) and [tracer-bullet ticket proposal](docs/planning/TICKETS-PROPOSAL.md). These are **drafts, not implementation authorizations**.

## Privacy

This repository is public, **but user tasks, notes, SQLite databases, backups, private exports, secrets, and ambient audio assets with unverified redistribution rights must never be committed**. Never use real personal content in fixtures or screenshots. Development samples must be synthetic.

## Scope

This initial repository contains planning documents only. Implementation and dependency scaffolding will start after the remaining product decisions are finalized.
