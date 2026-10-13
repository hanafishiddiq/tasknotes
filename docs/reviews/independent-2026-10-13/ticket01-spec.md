> **Review archive:** Ticket 01, independent AI SPEC axis, Claude Sonnet 4.6, read-only no-tool prompt with inline fixed-point public source. This is not an authenticated GitHub or human approval. Review assertions are preserved as originally produced (except removing absolute local paths); independently cross-checked factual corrections are in [the reconciled findings](../tickets-01-03-independent-2026-10-13.md).

# Ticket 01 / SPEC — Independent Code Review

**Reviewer role:** Independent AI code reviewer.
**Scope:** SPEC axis only — exact acceptance criteria vs. implementation at head `b799ccec`.
**Limitations acknowledged:** Generated binary assets, lockfiles (`pnpm-lock.yaml`), `Cargo.lock`, icon files, and any QA-only branches (`test/native-wdio-e2e`, docs PR14, QA Issue19) are excluded. The native WKWebView E2E evidence from `docs/qa/native-webdriver-results-2026-10-12.md` is noted as *later* evidence stacked on Ticket 03 and is not treated as independent approval of Ticket 01.

---

## HARD FINDINGS

### F-1 · Status display is hardcoded `"Ready"` — does not reflect stored `status` field · **BLOCKING**

**Spec requirement (AC-2):** "Creating a Task in the UI stores a stable ID, **Ready status**, 0% progress, and **Medium priority** through the Rust application core."

**Relevant spec invariants:** The glossary defines three statuses (`Ready`, `In Progress`, `Completed`); the spec (user story 14) mandates the fixed group order *In Progress → Ready → Completed* be visible in the UI, and user stories 20–23 require the displayed status to change as progress changes.

**Code location:** `src/App.tsx`, the `TaskRow` component's `task-row-meta` block (diff hunk approx. lines +162–168):

```tsx
<span className="status-pill"><span className="status-dot" />Ready</span>
<span className="priority-pill">Medium</span>
```

Both strings are **hardcoded literals**. The `Task` type in `src/lib/task-api.ts` carries `status: WorkStatus` and `priority: Priority` fields, and the Rust core correctly stores and returns them. The UI never reads `task.status` or `task.priority` from props.

**Consequence:** Any task received from the store with status `"in_progress"` or `"completed"`, or priority other than `"medium"`, will still display `"Ready"` / `"Medium"`. This directly violates AC-2's requirement that status and priority are conveyed *through* the core and reflected in the UI, and contradicts spec invariants 14, 20–23. The React test mock only ever produces `status: "ready"` and `priority: "medium"` tasks, so the test suite does not exercise — and cannot catch — this defect.

**Reproduction at typed React/Tauri seam:** Invoke `create_task` from the Tauri command layer with any valid input; the Rust core will return `status: "ready"` and `priority: "medium"` (correct). Now open `TaskStore::update_task` externally (e.g., via future CLI or direct test) and mutate status to `"in_progress"` — or simply build Ticket 02 which adds status transitions. Call `list_tasks`; the returned `Task` has `status: "in_progress"`. The `TaskRow` still renders "Ready". The typed `Task` interface makes this a provable gap: `task.status` is available at the call site but is unused.

---

### F-2 · `updated_at` monotonicity guard `max(updated_at + 1, ?5)` silently advances timestamp beyond wall clock · **MODERATE**

**Spec requirement:** The spec mandates `updated_at` timestamps be reliable enough to serve as cursor keys for `created_at DESC, id DESC` ordering, and that Audit Entries are "immutable dated records." The core relies on `updated_at` as part of data integrity.

**Code location:** `crates/tasknotes-core/src/tasks.rs`, `update_task`, the UPDATE SQL (diff hunk approx. lines +235–238):

```sql
revision = revision + 1,
updated_at = max(updated_at + 1, ?5)
```

If the wall-clock time `?5 (now)` equals `updated_at` (e.g., rapid successive edits within the same millisecond), `updated_at` becomes `updated_at + 1`. After enough rapid edits, `updated_at` diverges arbitrarily from real time. The current `list_tasks` index and cursor are on `created_at`, not `updated_at`, so this does not affect pagination directly; however it is a correctness concern for any future time-based display or Audit Entry timestamps that future tickets build upon.

This is moderate because Ticket 01's acceptance criteria do not explicitly test sub-millisecond edit sequences, and the failing scenario is unlikely in manual use.

---

## NONBLOCKING OBSERVATIONS

### O-1 · `TaskPage` missing `schema_version` field

**Spec (Implementation Decisions — Data and behavior):** "Paginated list/search responses include `schema_version`, `items`, `next_cursor`, and revision metadata."

`TaskPage` in `crates/tasknotes-core/src/tasks.rs` (diff lines +139–142) only includes `items` and `next_cursor`. No `schema_version`. Ticket 01 is explicitly scoped to a minimal shell, so this may be intentionally deferred, but it should be called out as a known gap before the AI/MCP adapter tickets close.

### O-2 · WAL mode set on every connection open, not once at migration

`connect()` (approx. diff line +196) does not set `journal_mode = WAL`. WAL is set only in `open()` during migration (approx. line +218). Per SQLite documentation, WAL is a database-level persistent setting once set; subsequent connections inherit it. This is functionally correct, but it means a database opened by a future process without going through `open()` first would not explicitly re-enable WAL. Observation only — no AC is breached.

### O-3 · `TASKNOTES_DEV_DATA_DIR` override is `#[cfg(debug_assertions)]` only

`src-tauri/src/lib.rs` (approx. diff lines +34–40): the dev-data-dir override disappears in release builds, which is the correct security posture per AGENTS.md ("The release build always uses the OS-managed private application directory"). This is intentional and correct. Noted for audit completeness.

### O-4 · React test mock uses fixed `id: "task-001"` for all creates

`src/App.test.tsx` (approx. diff line +52): every `create_task` call returns `id: "task-001"`, so the "Load more" and multi-task tests cannot catch duplicate-ID collisions. This is a test fixture limitation, not a production defect; the Rust core uses `Uuid::new_v4()`.

---

## VERDICT

**Status: NOT CLEAR FOR MERGE — one blocking spec gap.**

| Finding | File | Severity |
|---|---|---|
| F-1: Hardcoded `"Ready"` / `"Medium"` — status/priority not rendered from store data | `src/App.tsx` TaskRow | **Blocking** |
| F-2: `max(updated_at + 1, now)` monotonicity drift | `crates/tasknotes-core/src/tasks.rs` | Moderate |
| O-1: `schema_version` absent from `TaskPage` | `crates/tasknotes-core/src/tasks.rs` | Observation |
| O-2: WAL not re-applied in `connect()` | `crates/tasknotes-core/src/tasks.rs` | Observation |

**F-1 is blocking** because the spec explicitly requires status and priority to be stored *through* the Rust core and visible in the UI, and AC-2 depends on the correct state being visible to the user. The automated test suite does not cover this gap (all mock tasks have `status: "ready"` and `priority: "medium"`). The native WKWebView results from `docs/qa/native-webdriver-results-2026-10-12.md` are stacked on Ticket 03 and do not constitute independent review of this axis.

The two open manual acceptance items (create/edit/restart native path) remain outstanding per the ticket itself and are correctly not claimed as complete.
