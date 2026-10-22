# Ticket 05 — independent review (2026-10-22)

Range `origin/main..50a2d96` (5 commits). Verdict: **APPROVE**, no Blocker or Major issues.
Reviewer runs: 60 Vitest, typecheck, 47 Rust core tests, Clippy `-D warnings`: all pass. Release large fixture: rare word 1.5 ms; 2,880 matches over all pages 283–433 ms.

Verified OK: FTS index stays in sync on every write path; older Next Step values searchable through Progress Entries; rowids unchanged by VACUUM on bundled SQLite 3.50.2 (risk only if a future restore or table rebuild re-inserts Tasks); no SQL/FTS injection; tag fold matches Ticket 04; v5 migration atomic with rollback; cursor fingerprint rejects foreign queries; `list_tasks_in_view` delegation preserves behaviour.

## Open findings (to fix before PR)

1. Minor (reproduced): a filter change while a save is pending reuses the old page cursor → "Invalid task page cursor" error. `src/App.tsx` commitTaskMutation / applyFilters. Disable filters while saving or reset the cursor when the query changed.
2. Minor (reproduced): words after the 8th are dropped silently, so "every word must match" breaks. `crates/tasknotes-core/src/search.rs` MAX_SEARCH_WORDS.
3. Minor (reproduced, deliberate): no hidden-Task notice when the filtered result has more pages, even when the status filter alone proves the new Ready Task is hidden.
4. Minor (process): Ticket 05 issue file has no evidence yet; native QA evidence to record.
5. Nit / spec: Tasks in a removed List appear in neither scope — settle in Ticket 09.
6. Nit: whitespace-only tag in the core means "no results"; should mean Any.
7. Nit (unverified): the `role="status"` notice may not be announced by VoiceOver; up to three same-named Clear buttons.
