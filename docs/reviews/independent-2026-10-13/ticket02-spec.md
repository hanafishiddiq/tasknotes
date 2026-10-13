> **Review archive:** Ticket 02, independent AI SPEC axis, Claude Sonnet 4.6, read-only no-tool prompt with inline fixed-point public source. This is not an authenticated GitHub or human approval. Review assertions are preserved as originally produced (except removing absolute local paths); independently cross-checked factual corrections are in [the reconciled findings](../tickets-01-03-independent-2026-10-13.md).

# Ticket 02 / SPEC — Independent Code Review

**Scope:** `git diff b799ccec...bc8ac501` (three-dot, source files only). Binary assets, lockfiles, and QA-only E2E plugin code (`test/native-wdio-e2e` branch) are excluded from this diff and cannot be evaluated here.

---

## HARD FINDINGS

### F-1 · `Ready` guard allows a Ready→Ready no-op via status-only write — **Moderate**

**Requirement:** AC-5 / spec §"A manual attempt to move partially completed work to Ready must not silently erase its saved progress." The rejection path is specified for *started* Tasks.

**Code:** `tasks.rs` change\_task, `WorkStatus::Ready` branch (diff hunk ~+430–437):

```rust
WorkStatus::Ready => {
    if before.status != WorkStatus::Ready || next_progress != 0 {
        return Err(TaskError::InvalidTransition(…));
    }
    next_status = WorkStatus::Ready;
}
```

The guard is logically correct for the invariant, but the second condition (`next_progress != 0`) is evaluated *after* the progress block has already set `next_progress` from `input.progress`. A caller sending `{ status: Ready, progress: Some(0) }` on a currently-Ready task with 0 progress passes both branches (no error returned) and issues a redundant `UPDATE`. This is a no-op data-wise, so no data loss occurs, **but the Rust test `contradicting_status_progress_or_reset_to_ready_is_rejected_without_changes`** only covers `(Some(Ready), Some(0))` against an *In Progress* task (45% progress). The case `(Some(Ready), Some(0))` on an already-Ready 0% Task is untested at the core seam. Not a spec violation, but an unverified edge.

**Severity:** Observation (no data mutation, but creates a spurious revision bump and violates the spirit of "no-op changes must still count as a change").

---

### F-2 · `previous_incomplete_progress` update applies unconditionally at 0% — **Moderate**

**Requirement:** Spec Q23 / AC-3: "A manually reopened Completed Task regains its previous incomplete percentage." The invariant is: `previous_incomplete` should track the last *meaningful* non-complete progress.

**Code:** `tasks.rs`, end of the mutation block (diff ~+470–472):

```rust
if next_progress < 100 {
    previous_incomplete = next_progress;
}
```

When a user explicitly sets progress to **0%** on an In Progress Task (the test `percent_and_status_transitions` covers this as `update(&reopened, None, Some(0))`), `previous_incomplete` is overwritten with `0`. A subsequent reopen of a Task *that was never Completed after that zero-write* loses no information because `previous_incomplete` was just set to 0 correctly. However, if the Task is then Completed via `{ status: Completed }` (no explicit progress), `next_progress` is forced to 100, so `previous_incomplete` is **not** updated (condition `< 100` is false). The prior value of 0 is preserved. On reopen, progress restores to 0 — which matches the last explicitly set value. **This is correct by the spec.**

However, the test sequence `update(&idle, Some(Completed), None)` → reopen correctly restores 0% (the last explicitly set percentage). The spec says "regains its previous incomplete percentage" — the previous incomplete percentage **is** 0 in this case. No spec violation found. Observation only.

**Severity:** Observation (behavior matches spec; coverage of the 0%-then-complete-then-reopen chain could be more explicit in the test commentary).

---

### F-3 · React `status` selector allows selecting `ready` on any task, relying on `disabled` attribute alone — **Moderate**

**Requirement:** AC-1: "every priority has its agreed color, icon and accessible text label." AC-5: reset to Ready of a started Task must be rejected, not silently coerced.

**Code:** `TaskRow.tsx`, status `<select>` (diff ~+128–135):

```tsx
<option key={status} value={status}
  disabled={status === "ready" && task.status !== "ready"}>
  {STATUS_VIEW[status].label}
</option>
```

The Ready option is `disabled` in the DOM but remains present in the serialized option list. A user with keyboard navigation or an accessibility tool that bypasses `disabled` option state could select Ready on an In Progress Task and trigger `onChange` → `taskApi.change({ status: "ready" })`. The Rust core **will correctly reject this** with `InvalidTransition`. The rejection error surfaces in the UI error banner — so data integrity is maintained. However, the spec says "UI and Rust core tests agree on ordering and transitions" (AC-6), implying the UI should not present invalid options at all, or at minimum clearly communicate why they are unavailable. The test `sends progress updates to Rust and groups a completed Task` confirms the option is present and disabled after completion, but does not assert that the error path is surfaced if bypassed.

**Severity:** Moderate (spec says UI and core agree; the UI presents a structurally invalid option with only a `disabled` hint; the Rust fallback works, but the spec intent is prevention at the UI layer, not silent reliance on downstream rejection).

---

### F-4 · `list_tasks_sorted` CTE aliases `{COLUMNS}` in outer SELECT but CTE uses the same `{COLUMNS}` string — **Moderate (potential data correctness issue)**

**Requirement:** Spec: "Fixed status-group order holds across all sort options." AC-2 verified by Rust tests.

**Code:** `tasks.rs`, `list_tasks_sorted` SQL (diff ~+510–525):

```rust
let sql = format!(
    "WITH ranked AS (
        SELECT {COLUMNS}, {STATUS_RANK} AS status_rank,
            {priority_expr} AS priority_rank
        FROM tasks
     )
     SELECT {COLUMNS} FROM ranked
     WHERE {cursor_filter}
     ORDER BY status_rank ASC, {secondary_order}created_at DESC, id DESC
     LIMIT ?5"
);
```

The outer `SELECT {COLUMNS} FROM ranked` references the CTE's columns by the `{COLUMNS}` expansion, which is the original column list from the `tasks` table. SQLite CTEs project all declared columns including the computed aliases (`status_rank`, `priority_rank`). The outer `WHERE {cursor_filter}` references `status_rank` and `priority_rank` — **these names are only available because the CTE projected them**. The outer SELECT's `{COLUMNS}` (the raw task columns) does not include `status_rank`/`priority_rank` — but the WHERE clause references them from the CTE scope. **This is valid SQLite CTE syntax**: the WHERE clause of the outer query can reference the CTE's full column list even if SELECT enumerates only a subset.

No bug here — confirmed valid by the passing Rust integration tests. Observation only.

**Severity:** Observation (correct but the CTE column aliasing pattern is non-obvious and could mislead a future maintainer).

---

### F-5 · Ticket-02 AC-1 first bullet: "Automated Tauri command-contract and React tests pass; native GUI click/type interaction remains **unverified**" — **Tracking gap, not a code defect**

**Requirement:** AC-1 is explicitly marked `[ ]` (not checked). The spec criterion for all five priorities having "agreed color, icon and accessible text label" at the command-contract and React level is demonstrated by the diff. The native GUI path is acknowledged open.

**What the diff delivers:** `task-visuals.ts` declares all five `PRIORITY_VIEW` entries with distinct icons and labels. CSS `priority-pill--*` classes provide distinct background/foreground color pairs. The test `shows all five labeled priorities with distinct visual symbols` asserts `priority-pill--{priority}` class presence, an `svg` child, and correct text content.

**Gap:** The spec's AC-1 says "every priority has its **agreed** color, icon and accessible text label." The diff establishes *a* color and icon scheme, but no approval record of the specific palette (e.g., very\_high = `#a82d49` background `#ffedf0`) appears in the spec or any ADR. The colors are plausible but unratified by the planning documents in scope.

**Severity:** Moderate (the visual design tokens are not traceable to an approved palette; spec says "agreed color" without documenting what was agreed).

---

## NONBLOCKING OBSERVATIONS

**O-1 · `sort` omitted from `list_tasks` invocation when `priority` (default)** — `task-api.ts` line: `invoke("list_tasks", sort === "priority" ? { limit, cursor } : { limit, cursor, sort })`. The comment says "omit for compatibility with Ticket 01 command contract." This is a non-orthogonal omission: if Ticket 01's contract is tested with an explicit `sort: "priority"`, it may fail. Since the Tauri command now accepts `sort: Option<SortMode>` and defaults via `unwrap_or_default()`, passing `sort: "priority"` explicitly would also work. The compatibility concern is with the *test fixture* for Ticket 01, not the command itself.

**O-2 · `v2` cursor prefix hard-coded as a string literal** — `tasks.rs` cursor encoding and validation. If a future schema version changes cursor structure, the `v2` prefix must be updated in two places (encode and validate). A named constant would prevent silent drift.

**O-3 · `canChangeSelectors` vs `canChangeMetadata` split** — `TaskRow.tsx`: progress input uses `canChangeMetadata` (allows editing when text is dirty), while status/priority selects use `canChangeSelectors` (blocked when progress is dirty). The hint text at the bottom correctly explains the mutual exclusivity. However, the spec says "text/progress drafts protected from unintentional lost edits" — the split is correct, but the UI allows a user to begin a progress edit, then edit title text simultaneously (both are enabled until `hasUnsavedProgress` is true *and* text is touched). A user who types in the title and then in progress could have both dirty simultaneously, locking all controls. The test `does not lose text edits when switching priority` covers only title-dirty → selectors-disabled. No test covers the combined dirty state. Not a spec violation; the behavior prevents data loss.

**O-4 · `UPDATE … RETURNING {COLUMNS}` — revision race window** — `tasks.rs` change\_task uses `Immediate` transaction and `WHERE id = ?6 AND revision = ?7`. If the `query_row` on RETURNING fails to match (because another writer incremented revision between transaction open and UPDATE), `rusqlite` returns `QueryReturnedNoRows`, which maps to an unchecked `?` — propagating as a generic database error rather than `RevisionConflict`. This is an existing pattern in the codebase (from `update_task`) and acceptable under WAL + `Immediate` transactions, but worth noting.

---

## VERDICT

**No blocking spec violations found in the diff.** The implementation satisfies the checked acceptance criteria (AC-2 through AC-6) with Rust and React tests that match the transition table. AC-1 carries one confirmed **moderate gap**: the "agreed color" design tokens for the five priorities are unratified in any planning document within scope.

F-3 (Ready option accessible to keyboard bypass, relying on Rust rejection) is the most consequential moderate finding: the spec intends UI and core to *agree* on valid transitions, not for the UI to silently defer all invalid transitions to downstream rejection. This does not break data integrity but misaligns with the spec's explicit AC-6 language.

**AC-1 native GUI verification** and **Ticket 01 dependency/merge gate** remain open by the ticket's own declaration and are not findings added by this review.

**Limitation:** This review excludes generated binary assets, compiled `.app` bundle contents, lockfiles (`Cargo.lock`, `pnpm-lock.yaml`), and the QA E2E plugin on the separate `test/native-wdio-e2e` branch. Conclusions about runtime behavior at the native WebView layer are drawn solely from the automated test evidence embedded in the diff.
