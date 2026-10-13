> **Review archive:** Ticket 03, independent AI SPEC axis, Claude Sonnet 4.6, read-only no-tool prompt with inline fixed-point public source. This is not an authenticated GitHub or human approval. Review assertions are preserved as originally produced (except removing absolute local paths); independently cross-checked factual corrections are in [the reconciled findings](../tickets-01-03-independent-2026-10-13.md).

## Ticket 03 / SPEC — Independent Code Review

**Scope limit:** Review covers only the unified diff `bc8ac501...2937c9a0` against the approved spec and ticket acceptance criteria. Binary assets, lockfiles, QA-only branches (PR14, Issue19), and generated build artifacts are excluded. Native GUI click/type/restart acceptance is outside automated-test coverage by the ticket's own admission.

---

### HARD FINDINGS

**None proven blocking from the diff alone.** Rationale per criterion follows.

---

### NONBLOCKING OBSERVATIONS

#### OBS-1 · `saveProgressUpdate` splits routes on a non-spec condition — moderate
**File:** [`src/components/TaskRow.tsx`](../../../src/components/TaskRow.tsx), hunk `+async function saveProgressUpdate()` (~line 302–316 in diff context).

```ts
// percentage-only path bypasses record_progress
if (!hasUnsavedNextStep && !hasUnsavedNote) {
  await onChange({ progress: numericProgress });
  return;
}
await onRecordProgress({ ... });
```

A **progress-percentage-only change** routes to `change_task` (not `record_progress`). The spec (user story 19, ADR-0005) says *"every substantive progress/status change creates an append-only dated Progress Entry."* A bare percentage change — e.g., 0% → 60% — is substantive. The Rust `apply_task_change` does insert a `progress_entries` row when routed via `change_task` (the `substantive` flag covers status/progress/next_step/note changes), so the entry **is** written at the core level; the issue is that the UI silently routes the same gesture to a different command. The React test suite exercises only the `record_progress` path for percentage changes with notes; there is no React test asserting that a bare percentage input via "Apply progress" also produces a visible history entry. The actual Rust core will write it, but the UI's fork makes it harder to reason about — and any future UI-level filtering of what to send would be exposed at the wrong abstraction level.

**Severity:** Moderate (behavior is correct at the core seam today; the split is a fragility, not a current data-loss bug).

---

#### OBS-2 · History reload on `task.revision` — observation
**File:** [`src/components/TaskProgressHistory.tsx`](../../../src/components/TaskProgressHistory.tsx), `useEffect` deps `[taskId, revision, retry]`.

The history component re-fetches when `revision` increments, which is the correct trigger after a `record_progress` or `change_task` saves. However, if a concurrent (e.g., CLI or MCP) write increments the revision without the UI reflecting it in the task list, the history panel will not refresh until a `list_tasks` poll or user action delivers the new revision. The spec (story 24) requires displayed progress and status to remain consistent under both UI and AI edits. This is not a new gap introduced by this ticket — it was already present for Ticket 02 fields — but the append-only history panel is now a more visible target for staleness. No automated test covers this cross-adapter scenario.

**Severity:** Observation (pre-existing limitation; no regression introduced here).

---

#### OBS-3 · `ProgressEntry.previousStatus` missing from history display — observation
**File:** [`src/components/TaskProgressHistory.tsx`](../../../src/components/TaskProgressHistory.tsx), `entries.map` render block.

The rendered timeline shows `entry.status` (current status label) and the progress delta, but omits `entry.previousStatus`. The spec (story 19) says users want to see "what changed and when"; a transition from `ready → in_progress` is only visible through the progress delta, not a status label change. This is a UX gap against the spirit of story 19, though the spec does not prescribe the exact display format.

**Severity:** Observation.

---

#### OBS-4 · `COLUMNS` column index offset — observation
**File:** [`crates/tasknotes-core/src/tasks.rs`](../../../crates/tasknotes-core/src/tasks.rs), `read_task` hunk:

```rust
next_step: row.get(9)?,
revision: row.get(6)?,
```

`COLUMNS` is redefined to append `next_step` as the tenth column (index 9). The `previous_incomplete_progress` column is fetched separately via `row.get(10)` in the one place that needs it (the `apply_task_change` query). This is correct but relies on the append ordering being stable; any future column addition to `COLUMNS` requires updating both `read_task` indices and the `previous_incomplete_progress` fetch index. The pattern works, but it is a fragility the spec does not require — a named struct deserializer would be safer. No bug in current state.

**Severity:** Observation.

---

#### OBS-5 · Progress note acceptance via native GUI — open spec gap
Per the ticket itself (native QA 2026-10-12): three history events reached SQLite with empty `note` fields. The first acceptance criterion — *"editable progress 0–100%, Next Step, and dated progress notes without leaving the expanded Task"* — remains **partially unverified** for the note-entry path specifically. The E2E WKWebView automated suite (`docs/qa/native-webdriver-results-2026-10-12.md`) confirms persistence for status/priority/progress/Next Step and reopen, but not note text entry. The ticket explicitly records this; it is not a fabricated finding.

**Severity:** Blocking to marking the criterion `[x]` — already acknowledged as open in the ticket.

---

### VERDICT

No **new blocking spec violations** are introduced by the diff. The core invariants — append-only `progress_entries`, atomic transaction, reopen to previous incomplete percentage, pagination, oversized-text rejection, rollback on failure, v2→v3 migration — are implemented and covered by the Rust integration tests cited in evidence. The React command-contract tests cover the `record_progress` path, history rendering, error/retry, and pagination.

The one **acknowledged open gap** (progress note native GUI entry unverified, OBS-5) is correctly documented by the ticket and must remain open. The dual-route UI pattern (OBS-1) is the most actionable code-level finding: it is not a current data-loss defect but reduces spec traceability.

**Overall:** Implementation is consistent with spec for the verified seams. The two open acceptance checkboxes in the ticket accurately reflect actual status. Do not mark the note-entry criterion complete until native GUI evidence is collected.
