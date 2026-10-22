> **Review archive:** Ticket 03, independent AI STANDARDS axis, Claude Opus 4.6 (Thinking), read-only no-tool prompt with inline fixed-point public source. This is not an authenticated GitHub or human approval. Review assertions are preserved as originally produced (except removing absolute local paths); independently cross-checked factual corrections are in [the reconciled findings](../tickets-01-03-independent-2026-10-13.md).

# Ticket 03 / STANDARDS — Code Review

## Scope & Limitations

Reviewed the embedded three-dot diff `bc8ac50...2937c9a` covering `crates/tasknotes-core/src/tasks.rs`, `src-tauri/src/lib.rs`, `src/App.tsx`, `src/components/TaskRow.tsx`, `src/components/TaskProgressHistory.tsx`, `src/lib/task-api.ts`, `src/App.css`, and all test files. **Excluded:** generated binary assets, lockfiles, `package.json` changes, and QA-only branch (`test/native-wdio-e2e`) code not in the diff scope.

---

## HARD FINDINGS

**None proven.** No file/hunk in the diff violates a documented AGENTS.md invariant, GLOSSARY.md term, approved spec decision, or ADR.

Specifically verified:
- **Concurrency invariant** (AGENTS.md §Invariants): `apply_task_change` wraps task UPDATE + `progress_entries` INSERT in one `transaction.commit()` with `WHERE revision = ?8` — optimistic revision check is intact. (`tasks.rs`, hunk near line 592–610.)
- **No-permanent-deletion** (AGENTS.md, Spec §Data): `ON DELETE RESTRICT` on `progress_entries.task_id` FK prevents orphan deletion. No `DELETE FROM` exists.
- **No duplicated business rules** (AGENTS.md §Engineering flow): Status-transition logic, progress clamping, and reopen-at-previous-percentage live exclusively in `apply_task_change`; React calls `taskApi.change` / `taskApi.recordProgress` without re-encoding rules.
- **Boundary validation** (Clean Code Rule 3): Note ≤10 000 chars, Next Step ≤4 000 chars, progress 0–100, history page 1–50, cursor >0 — all checked before any write. SQL uses parameterized queries throughout.
- **Append-only history** (ADR-0005): `progress_entries` is INSERT-only; no UPDATE or DELETE path exists.
- **Idle tasks stay In Progress** (ADR-0006): No automatic Ready transition; a note-only `record_progress` on a Ready task keeps it Ready at 0%.

---

## NONBLOCKING OBSERVATIONS (Fowler heuristic smells)

| # | Smell | File / Hunk | Detail |
|---|-------|-------------|--------|
| 1 | **Dead Code (Speculative Generality)** | `tasks.rs` L28 `EmptyChange` | `change_task` now delegates to `apply_task_change` which returns `EmptyProgressUpdate`, never `EmptyChange`. The variant remains defined but is unreachable — dead enum arm. Harmless today; remove or unify before CLI/MCP adapters pattern-match on it. |
| 2 | **Data Clumps** | `TaskRow.tsx` L35–37 | `draftProgress`, `draftNextStep`, `draftNote` are always reset, validated, and discarded together. A single `ProgressDraftState` object would reduce the six correlated `useState` sites to one. |
| 3 | **Feature Envy (mild)** | `TaskRow.tsx` `saveProgressUpdate` ~L90–100 | Component decides whether to route through `onChange` (→`change_task`) or `onRecordProgress` (→`record_progress`) based on which fields changed. The branching is thin but the decision "which API path?" leans toward the API layer. |
| 4 | **Mysterious Name (minor)** | `tasks.rs` `apply_task_change` | Name doesn't convey that it also creates a `ProgressEntry`. Consider `apply_task_mutation` or `apply_task_update_with_history`. |

None of these override repo rules or rise above heuristic observation.

---

## VERDICT

**PASS on STANDARDS axis — no blocking findings.** The diff enforces all documented invariants (transactions, revision checks, append-only history, no permanent deletion, no duplicated rules, validated boundaries). Four nonblocking heuristic smells are noted for cleanup at discretion. The dead `EmptyChange` variant should be removed before Ticket 04+ adapters consume the `TaskError` enum.

**Gaps:** This review cannot independently verify generated binary assets, lockfile integrity, or behavior in the QA-only native WebDriver branch outside the diff scope.
