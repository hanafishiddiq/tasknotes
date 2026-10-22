# Ticket 05 — Indexed Task search and filters: plan (2026-10-22)

Owner instruction 2026-10-22: start Ticket 05 after Tickets 01–04 were merged (PRs #1–#3, #15, #16, #17).

## Owner decisions

- **Matching:** word-prefix, case- and accent-insensitive (`plan` → "planning", `cafe` → "Café"). Not substring/trigram.
- **UI:** search field in the toolbar; compact filter chips under the page title, in the same capsule style as Task properties.
- **Removed:** "the best option, not the fastest". The query layer is Removed-aware now so Ticket 09 does not rework search or add another migration (see below).

## Design

**Schema v5 (forward-only, one transaction):**
- `tasks.removed_at INTEGER NULL`: the Task's Removed disposition (ADR-0002). Ticket 05 only *reads* it. The remove/restore operations, audit and UI belong to Ticket 09.
- `task_fields_fts` (FTS5, `unicode61 remove_diacritics 2`, rowid = `tasks.rowid`): title, description, Next Step. Kept in sync by triggers on insert/update.
- `progress_fts` (FTS5, rowid = `progress_entries.sequence`, `task_rowid UNINDEXED`): one row per Progress Entry note. Entries are append-only, so this is insert-only.
- Backfill both tables from existing rows inside the migration.

Triggers rather than Rust-side writes so that every future writer (CLI, MCP) keeps the index consistent through the same SQLite file. Two tables keyed by rowid make every update O(log n); a single table keyed by an UNINDEXED task id would need full scans on update.

**Query semantics (one core entry point):**
- Text is tokenised in Rust into word tokens, each quoted and prefix-matched (`"plan"*`). User input never reaches FTS syntax. More than 16 words is refused with a clear error instead of silently dropping words (review finding 2); the desktop shows "Search couldn't run" with the message rather than the database-failure state or rows from the previous query.
- A Task matches when **every** token appears somewhere in its fields **or** any of its Progress Entries.
- Filters are combinable: work status, priority and tag (one value each, or Any). The List dimension is the existing sidebar scope (All / Inbox / a List) instead of a duplicate chip.
- Ordering stays In Progress → Ready → Completed with the chosen secondary sort (priority or newest). No relevance sort: the spec fixes the group order under any sort.
- Scope: Active by default excludes Removed Tasks and Tasks in removed Lists. An explicit Removed scope returns only Removed Tasks.
- Keyset cursor bound to a fingerprint of the full query (text, filters, scope, view, sort), so a cursor from another query is rejected instead of skipping or repeating rows.

## Vertical TDD slices

1. **Core search:** v5 migration (v4 → v5 with history intact; v1 → v5), FTS on title, description, Next Step and old Progress Entries; prefix and diacritics; Removed exclusion and explicit Removed scope; Tasks in removed Lists excluded.
2. **Core filters and paging:** status, priority and tag combinations; both sorts keep group order; paging over a filtered and searched set never repeats or skips; foreign cursors rejected.
3. **Desktop:** Tauri command carries the query; toolbar search (debounced) and filter chips; empty-result state; paging stays bounded; async race guards as in Ticket 04.
4. **Large fixture:** functional (not benchmark) test with several thousand synthetic Tasks that search and filter stay page-bounded. Measured benchmark remains Ticket 17.
5. Native QA on a synthetic QA bundle, independent review, ticket evidence.

## Risks

- Schema v5 is forward-only like v4: no build from `main` should open the owner's database without a verified backup.
- Broad one-letter prefixes match many rows; the page is still bounded but SQLite sorts all matches. Ticket 17 measures this against the 250 ms p95 target.
