> **Review archive:** Ticket 02, independent AI STANDARDS axis, Claude Opus 4.6 (Thinking), read-only no-tool prompt with inline fixed-point public source. This is not an authenticated GitHub or human approval. Review assertions are preserved as originally produced (except removing absolute local paths); independently cross-checked factual corrections are in [the reconciled findings](../tickets-01-03-independent-2026-10-13.md).

# Ticket 02 / STANDARDS — Independent Code Review

**Axis:** Standards (documented rules, invariants, Fowler smells)
**Diff:** `b799ccec...bc8ac501` (three-dot), files under `src/`, `src-tauri/src/`, `crates/tasknotes-core/`
**Scope exclusion:** Generated binary assets, lockfiles, and QA-only branch code (`test/native-wdio-e2e`) are not reviewed.

---

## HARD FINDINGS

**None proven.** Every documented invariant I can verify from the embedded source data is satisfied:

| Invariant (AGENTS.md / Spec / ADR) | Evidence in diff |
|---|---|
| Work Status distinct from Removed | `change_task` operates on status/priority/progress only; no Removed logic |
| Fixed group order IP → Ready → Completed | `WorkStatus::rank()` (0,1,2); `STATUS_ORDER` in `task-visuals.ts`; `status_rank ASC` in SQL; React `groups` filter preserves order |
| Revision checks + transactions for concurrency | `TransactionBehavior::Immediate` + `WHERE revision = ?7` in `change_task` (tasks.rs ~L449) |
| No duplicated business rules in UI | `TaskRow` sends raw `{status, priority, progress}` to Rust via `onChange`; transition logic is solely in `change_task`. The `disabled` guard on the Ready `<option>` is a UX affordance, not a competing rule |
| Ready = not started; idle IP stays IP (ADR-0006) | `change_task` rejects Ready for any previously-started task; progress 0 on an IP task keeps status IP (tasks.rs ~L422–426) |
| 100% = Completed; reopen restores last incomplete (Spec) | Explicit branches at ~L413–419 and ~L436–438; `previous_incomplete_progress` column with CHECK 0–99 |
| Contradictory writes rejected, not coerced | `InvalidTransition` errors for Ready-on-started, IP+100%, Completed+non-100 (tasks.rs ~L428–448); negative-control test at `status_priority.rs:L86–141` |
| Synthetic fixtures only; no user data in Git | All tests use `tempfile::tempdir()` / in-memory mocks |

---

## NONBLOCKING OBSERVATIONS (Fowler heuristic smells)

1. **Duplicated Code (SQL CASE expressions)** — The `CASE status WHEN…` / `CASE priority WHEN…` ranking expressions appear identically in the v2 migration DDL (tasks.rs ~L277–286) and the query constants `STATUS_RANK` / `PRIORITY_RANK` (~L487–489). This is standard SQL practice (DDL indexes vs. DQL projection), but extracting a shared `const` or doc-linking the two sites would reduce drift risk during future schema changes.

2. **Primitive Obsession (cursor format)** — The v2 cursor is a `|`-delimited string (`"v2|priority|0|5|123456|uuid"`) parsed by positional splits (~L507–536). A lightweight struct or named-field serialization would be self-documenting, though the validation is thorough and the format is internal.

3. **Speculative Generality (taskApi.list conditional omission)** — `task-api.ts:L37–39` conditionally omits the `sort` parameter for backward compatibility with Ticket 01's command contract, but the Tauri command already accepts `sort: Option<SortMode>`. The `None` default produces the same result, making the conditional unnecessary. Documented, harmless.

4. **Data Clumps (CSS hardcoded hex)** — All 25+ color values in `App.css` are raw hex literals. Theme tokens or CSS custom properties would support the spec's light/dark theme requirement, but theming is scoped to a later ticket.

---

## VERDICT

**PASS on Standards axis — no blocking findings.** The implementation correctly enforces every documented invariant reachable from this diff. Business rules are concentrated in the Rust core behind a small typed interface; the React layer is a presentation adapter. Transition-table coverage (11 Rust + 13 React tests in source) exercises positive and negative paths including revision conflicts, contradictory writes, migration, cursor cross-mode rejection, and deterministic tie-breaking.

**Stated gaps:** This review cannot verify runtime index performance of the CTE-based query plan, dark-mode visual rendering, or behavior of CLI/MCP adapters (not yet implemented). The review is limited to embedded source and cannot substitute for the unresolved manual native GUI acceptance (Ticket 01 dependency) or the independent Spec-axis review.
