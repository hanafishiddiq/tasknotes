# 13: Backup, validated restore, and export

**What to build:** Automatic local backup snapshots (7 daily, 4 weekly, 3 monthly), validated restoration coordinated with other writers, and user-controlled JSON/Markdown export. Test corrupted backups, conflict handling and preservation of Removed data.

**Blocked by:** 09 — Removed and restore, 11 — Private, AI-optimized Note CRUD in CLI

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] While a Tasknotes process is active, consistent local SQLite backups follow the approved 7 daily / 4 weekly / 3 monthly retention policy.
- [ ] Restoring a valid backup verifies integrity and coordinates with active UI, CLI and MCP writers to prevent stale writes.
- [ ] A corrupted/incomplete snapshot cannot replace the live database; a pre-restore recovery snapshot is retained.
- [ ] Users can export Task and Note data to JSON and Markdown; removed items remain recoverable and export options are clearly labeled.
- [ ] An AI-facing export must respect Exclude from AI, whereas explicit human-facing export can include user-selected private Notes.
- [ ] The recovery tests cover process restart, interrupted writes, locked files and empty databases without touching real user data.

## Verification

- [ ] Run backup/restore integration tests with deliberate corruption, writer-contention and synthetic Removed/private items.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-22:** Note Folders can be nested (owner decision, see Ticket 07). Markdown export should keep the folder tree (for example as directories) and JSON export must keep each folder's parent, so a round trip does not flatten it.
