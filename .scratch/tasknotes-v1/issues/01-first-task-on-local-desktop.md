# 01: First Task on a local desktop

**What to build:** Launch a minimal Tauri/macOS shell; create and edit a Task from UI; persist to local SQLite through Rust core; reload and find the same Task. Includes migration, minimal integration tests and synthetic-only fixtures.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Launching Tasknotes on the target Mac opens a working Tauri desktop window without any sign-in or network dependency.
- [ ] Creating a Task in the UI stores a stable ID, Ready status, 0% progress, and Medium priority through the Rust application core.
- [ ] Editing a Task title/description and restarting the application preserves exactly one updated Task in SQLite.
- [ ] A fresh local database is migrated automatically, while invalid Task data produces an actionable error and does not leave a half-written record.
- [ ] The first list/query path is bounded and supports pagination or bounded result windows; it does not load every Task into the UI.

## Verification

- [ ] Run one Tauri GUI create/edit/restart scenario and Rust core integration tests against a fresh isolated SQLite database.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
