# Ticket 04 — Standards and Spec review (implementing assistant; not independent)

Date: 2026-10-13. Fixed point: main at 118e4240389fc7c96addf26241bab8a3dd5356b8. Scope: Ticket 04 feature branch, initial candidate 286482fb60e04dad8d0628372efc981215882cb2 plus pending follow-up fixes. Inspect exact final review-head diff again before any merge.

## Provenance and independent-review blocker

Following /ask-matt and /code-review, two distinct independent reviewer processes (AGY Claude Opus for Standards and Claude Sonnet for Spec) were launched with fixed-point read-only public-source diff packets. Both exited 3 with empty outputs and HTTP 429 RESOURCE_EXHAUSTED: insufficient reviewer credits. These are **not** independent review results or an approval. The following is the **implementing assistant's self-review**, split by axis. Neither the original Draft PR nor this review authorizes merge or release.

## Axis 1 — Standards: self-review

- Confirmed app domain behavior concentrated in shared Rust TaskStore (crates/tasknotes-core/src/organization.rs, tasks.rs), with thin Tauri typed commands (src-tauri/src/lib.rs) and UI state only in React. Validations include List name, UUID List FK, 1–40 char Tag, normalized dedup, Gregorian date, optimistic revision and transactional writes. Source test suite validates v1/v2/v3→v4 without losing data/history on synthetic SQLite.
- Confirmed no Task, Note or List physical delete path added. List soft removal hides children from normal queries without mutating their work status/progress/Task revision. Tag association replacement intentionally deletes association rows in a single transaction, distinct from deleting Task/Note/List.
- Confirmed same status-group sorting remains handled by Rust keyset cursor and no duplicate TS business status transition logic added. Task/Tag hydration executes one bounded query per page rather than per Task (Rust limit 100; UI 50).
- Confirmed release bundle was not deployed, normal user database not accessed, and QA app used unique bundle identity/Cargo target.
- **Nonblocking potential smells:** extended src/App.tsx now coordinates multiple UI forms and fetches (possible Divergent Change); Rust SQL CTE repeats status/priority rank expressions in several places (possible Duplicated Code). Both are Fowler judgment calls, not proven repo rule violations; don't speculative-refactor without a failure.
- **Technical debt:** Tauri maps domain errors to plain strings, a pre-existing adapter convention. A structured versioned CLI/MCP error envelope is a later ticket; the new commands share this pattern. Query-plan p95 on large synthetic data is not measured in Ticket 04 and remains Ticket 17.

**Standards self-verdict:** no proven blocking violation found in inspected source and green strict Rust/Tauri Clippy. **External independent Standards review: BLOCKED (credit quota).**

## Axis 2 — Spec: self-review

| Ticket 04 requirement | Source behavior and proof | Remaining limitation |
| --- | --- | --- |
| Inbox/List and All without duplicates | Task.list_id nullable, one stored Task row, SQL task view predicate, Rust/React tests of create and moving across views | Real owner GUI not clicked |
| Tags and optional deadlines, overdue without altering work status order | Atomic TaskStore detail mutation, normalized tag join, strict ISO local date, React local calendar labels, unchanged status ordering | None in tested domain behavior |
| List and Task persistence/restart and no orphan references | Synthetic v4 migration, FK with restricted deletion, List soft remove/restore, stale-revision and reopened-store tests | Live personal SQLite migration deliberately prohibited |
| Deterministic local-day time zone and no reminders | Injected Intl.DateTimeFormat IANA zone test, yesterday/today/future statuses; no notification code | Device sleep/resume display rollover not an observed GUI test |
| Large List bounded query/viewport | SQLite WHERE before LIMIT (max100) and cursor tied to List/sort. Initial append-only UI was a genuine gap; bounded one-page-at-a-time 50-card Next/Previous navigation added RED→GREEN with 120 synthetic Task sample | Ticket 17 still measures large-dataset performance |

**Spec self-verdict:** five criteria have positive Rust/React evidence including a just-fixed bounded viewport gap. The ticket remains in review because actual macOS WebView click/edit/restart has not been observed and independent Spec review did not run due to AGY 429. Neither app-package smoke nor a visible QA screenshot substitutes for full native acceptance.

## Review-only findings remediated through TDD

1. After a failed List create or rename (e.g. stale revision), the sidebar used to discard drafts. Separate React regressions went RED then GREEN by clearing the draft only on a successful Tauri write.
2. The UI originally appended every Load More page indefinitely; explicitly violates bounded viewport requirement. Replaced with 50-item page, page-cursor history and reversible navigation. Test with 50→50→20→50→50 visible Task cards went RED and then GREEN. Paged navigation remains disabled during in-flight requests to prevent cross-view leakage.
3. Old codepath carried a v3 schema check in migration tests; target version updated to v4, with old v2/v3 actual data preservation tests green.

## Status and gated follow-up

- Branch head may change when this report is committed. Verify final PR head SHA, CI, status and review-diff before release.
- **Independent AI Standards & Spec review remains outstanding**. GitHub Draft PR #16 stays OPEN/DRAFT, no merge.
- Native GUI interactive acceptance (typed click/select/restore/restart with synthetic data) remains **not proven** because AX did not expose embedded WKWebView controls. A separate QA-only app rendered visually and was closed normally; no forced global input.
- Do not deploy a newer app against live SQLite without explicit approval and a verified recoverable backup: schema v4 is forward-only and old v3 binaries reject it.
- No 50,000 Task/10,000 Note performance certification or signed/notarized macOS release is claimed.
