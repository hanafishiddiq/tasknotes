# 05: Indexed Task search and filters

**What to build:** Search title, description, and progress notes; filter by work status, priority, List, and tag; maintain fixed status groups under alternate sorts, exclude Removed, and paginate via indexed core queries. Validate results from the UI.

**Blocked by:** 02 — Work status, priority, and fixed ordering, 03 — Progress history and Next Step, 04 — Inbox, Lists, tags, deadlines

**Status:** ready-for-agent

**Implementation:** review — owner started Ticket 05 on 2026-10-22 after Tickets 01–04 merged. Branch `feat/ticket-05-indexed-search-filters`, Draft PR pending owner merge approval. Schema v5 is forward-only: before installing a build from `main` over the owner's real database, take a verified backup.

## Acceptance criteria

- [x] Search finds Tasks by title, description, Next Step and Progress Entries, including terms in older history. **Evidence:** `search_finds_title_description_next_step_and_older_progress_notes`, `a_v4_database_upgrades_to_v5_and_indexes_existing_history` (backfill of existing history), native QA "wijaya" below.
- [x] Users can combine status, priority, List and tag filters and choose supported secondary sorts while status-group order stays fixed. **Evidence:** `tests/query_filters.rs`, `combines status, priority and tag chips with the search text`, native QA below. The List dimension is the sidebar scope (All / Inbox / a List), not a duplicate chip.
- [x] Task query results are page/cursor bounded; successive pages do not repeat or skip items under a stable query snapshot. **Evidence:** `tests/query_filters.rs` paging and foreign-cursor tests, `tests/large_fixture.rs` (every page ≤ 50, no repeat, no skip over 6,000 Tasks), `does not reuse a page cursor when the filters change during a save`.
- [x] Removed Tasks do not appear in ordinary search; an explicitly requested Removed view can find them. **Evidence:** `removed_tasks_and_tasks_in_removed_lists_stay_out_of_ordinary_search` and the large fixture's Removed scope. **Scope (owner decision 2026-10-22):** the core query is Removed-aware now; the remove/restore operations and the desktop Removed view belong to Ticket 09.
- [x] Indexed query access and incremental UI rendering remain functional with a large synthetic fixture; measured release benchmark is performed again in Ticket 17. **Evidence:** `search_and_filters_stay_correct_and_bounded_on_a_large_fixture` (6,000 Tasks); the reviewer's release run measured 1.5 ms for a rare word and 283–433 ms to page through all 2,880 broad matches. Not a benchmark: Ticket 17 owns the p95 measurement.

## Verification

- [x] Run deterministic indexed query tests and desktop search/filter/ordering E2E tests with synthetic data. Desktop coverage is Vitest against the mocked Tauri seam plus native GUI QA driven by computer use on an isolated QA bundle; there is no automated native E2E harness yet.
- [x] Relevant tests pass and evidence is recorded here (see "Implementation checkpoint — 2026-10-22" below).
- [x] Run `/code-review` and address findings before marking complete. Independent review: **APPROVE** ([review](../../../docs/reviews/ticket-05-independent-review-2026-10-22.md)); Minor findings 1–3 fixed with RED→GREEN tests; Nits 5–7 stay open (below).

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments


## Implementation checkpoint — 2026-10-22

**Design:** [plan](../../../docs/plans/ticket-05-indexed-search-filters-2026-10-22.md). Schema v5 adds `tasks.removed_at` and two FTS5 tables (`unicode61 remove_diacritics 2`) kept in sync by triggers, so every future writer of the SQLite file maintains them. One core entry point (`TaskStore::query_tasks`) handles search text, filters, scope, view and sort; the desktop only passes the query through the Tauri command. Matching is word-prefix and ignores case and accents (owner decision). Every word must match the Task's fields or any of its Progress Entries. More than 16 words is refused with a clear error rather than dropping words.

**Commits:** `b30a542` core search and v5 migration, `a04027c` filters/group order/paging, `69409a1` desktop search and chips, `d642dd9` hidden-Task notice and active-chip styling, `50a2d96` 6,000-Task fixture, `d5c1068` review record, then the review fixes below.

### Review findings

| Finding | Severity | Fix | Evidence |
| --- | --- | --- | --- |
| 1. Filter change while a save is pending reused the old page cursor ("Invalid task page cursor") | Minor | A save refreshes from page 1 when the query changed during it (`12d59d3`) | `does not reuse a page cursor when the filters change during a save` RED (cursor `page-2` sent with the new filter) → GREEN |
| 2. Words after the 8th were dropped silently | Minor | Up to 16 words; more is refused with "Search can use at most 16 words". The desktop shows "Search couldn't run" with Clear and Try again instead of the database-failure state or the previous query's rows (`bfc29d5`) | `every_search_word_counts_and_too_many_words_are_refused` RED (16th word ignored) → GREEN; `explains a refused search instead of claiming the database failed` RED → GREEN |
| 3. No hidden-Task notice when the filtered result had more pages | Minor | When the new Task is not on page 1, the desktop asks the core whether it matches through an optional `taskId` on the same query; if that check fails, the list stays and no notice is claimed (`717d2db`) | `a_query_can_check_whether_one_task_matches_it` RED → GREEN, and fails again with the `id` filter removed (mutation check); three UI tests RED → GREEN |
| 4. No evidence in this ticket | Minor | This section | — |
| 5. Tasks in a removed List appear in neither scope | Nit / spec | Open: settle with the Removed view in Ticket 09 | — |
| 6. A whitespace-only tag in the core means "no results" instead of Any | Nit | Open. The desktop trims the tag and never sends a blank one | — |
| 7. The `role="status"` notice may not be announced by VoiceOver; up to three same-named Clear buttons | Nit (unverified) | Open: Ticket 14 accessibility pass | — |

Native QA of the fixes found one more problem: under "Search couldn't run" the header and sidebar still counted the previous query's rows. Fixed in `d1f7d8e` (count hidden when unknown), RED → GREEN in the refused-search test.

### Native GUI QA

Unsigned debug **Tasknotes Ticket05 QA.app**, identifier `store.adamaja.tasknotes.ticket05qa`, own app-data directory with 64 synthetic Tasks / 2 Lists. Driven by the assistant through macOS computer use (real clicks, typing and native pop-up menus), cross-checked with read-only SQLite queries. The owner's database (`store.adamaja.tasknotes`) was never opened; its mtime is still 2026-10-13.

| Case | Build | Result |
| --- | --- | --- |
| Toolbar Search plus Any status / Any priority / Tag chips render | `69409a1` | PASS |
| `sample 5` → Sample task 50–55 (word prefix) | `69409a1` | PASS |
| `zebra` → "No matching tasks"; Clear restores every Task | `69409a1` | PASS |
| New Progress note "…Dr Wijaya", then search `wijaya` finds the Task (live FTS trigger) | `69409a1` | PASS |
| High → 1 Task; High + tag `urgent` → none; Any + `urgent` → Sample task 55 | `69409a1` | PASS |
| From page 2, choosing Low returns to page 1 with the filtered result | `69409a1` | PASS |
| New Task outside the Low filter → "Task added, but hidden…"; Clear shows it | `d642dd9` | PASS |
| Active chip distinct from focus ring (CSS, screenshot only) | `d642dd9` | PASS |
| Search `sample` (55 matches, more than one page), add "Order printer ink" → hidden notice shown. Without `taskId` reaching the core the one-row check would have returned a match and stayed quiet, so this also proves the Tauri wiring | `717d2db` | PASS |
| 17-word search → "Search can use at most 16 words" and "Search couldn't run"; Clear restores the list | `717d2db`, `d1f7d8e` | PASS (count bug found on `717d2db`, fixed and rechecked on `d1f7d8e`) |

Finding 1 is a race (the native save finishes in milliseconds), so it is covered by the deferred-save Vitest test, not native QA. Background typing into the WKWebView drops characters; that is a computer-use tool limitation, proved with foreground input and a jsdom test.

### Checks at `d1f7d8e`

64 Vitest tests, typecheck, frontend build, 49 Rust core tests (including the 6,000-Task fixture), core and Tauri Clippy `-D warnings`, Rust fmt: all PASS, synthetic data only.

### Known risks

- Schema v5 is forward-only like v4. No build from `main` may open the owner's database without a verified backup.
- The FTS tables are keyed to the implicit `tasks.rowid`. `VACUUM` keeps rowids on the bundled SQLite 3.50.2 (checked, and confirmed by the reviewer), but a future restore or table rebuild that re-inserts Tasks must rebuild the index.
- Broad one-letter prefixes match many rows: the page stays bounded but SQLite sorts all matches. Ticket 17 measures this against the 250 ms p95 target.

