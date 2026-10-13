> **Review archive:** Ticket 01, independent AI STANDARDS axis, Claude Opus 4.6 (Thinking), read-only no-tool prompt with inline fixed-point public source. This is not an authenticated GitHub or human approval. Review assertions are preserved as originally produced (except removing absolute local paths); independently cross-checked factual corrections are in [the reconciled findings](../tickets-01-03-independent-2026-10-13.md).

## Ticket 01 / STANDARDS — Independent Code Review

**Base:** `e4c728c2`, **Head:** `b799ccec`, three-dot diff. **Scope exclusions:** generated binaries, lockfiles, icon assets, QA-only branch code.

---

### HARD FINDINGS

**None proven.** The diff adheres to all documented invariants I can verify against the embedded standards:

- `WorkStatus` enum (`tasks.rs` L48–52) keeps exactly Ready/InProgress/Completed, with Removed absent — satisfying AGENTS.md invariant and ADR-0002.
- UI delegates all mutations to the Rust core via Tauri IPC (`task-api.ts` L37–47, `src-tauri/src/lib.rs` L4–23) — no competing business rules in TypeScript, per ADR-0001.
- `update_task` (`tasks.rs` L246–271) enforces optimistic concurrency via atomic `UPDATE … WHERE revision = ?4 RETURNING`, satisfying AGENTS.md revision-check invariant for the critical write path.
- Synthetic-only test fixtures (`task_store.rs`), no user data in diff. `#[cfg(debug_assertions)]` gating on `TASKNOTES_DEV_DATA_DIR` (`src-tauri/src/lib.rs` L34–37) keeps dev override out of release builds.
- New Task defaults: Ready, Medium, 0% progress (`tasks.rs` L238–240) — matches GLOSSARY.md and spec exactly.

---

### MODERATE (nonblocking, recommend addressing before merge)

1. **Stringified error identity at Tauri boundary** — `src-tauri/src/lib.rs` L10, L18, L23: all three commands use `.map_err(|e| e.to_string())`. The spec requires "stable identifiers (e.g., revision conflict, invalid input, unavailable storage)" in a versioned error envelope. The current pattern discards the `TaskError` variant, making it impossible for the frontend to distinguish conflict from validation failure programmatically. Ticket 01 only displays the message, so no user-facing bug, but the pattern must be retrofitted before CLI/MCP tickets consume it.

2. **TOCTOU in update\_task fallback** — `tasks.rs` L263–271: after the `UPDATE … RETURNING` yields zero rows, a separate `SELECT EXISTS` determines NotFound vs. RevisionConflict. No explicit transaction wraps both statements. A concurrent delete between them would produce a misleading `NotFound` instead of `RevisionConflict`. Data integrity is unaffected (the write already failed); only the error label is at risk. Low severity, but AGENTS.md says "transactions to protect against concurrent edits."

---

### OBSERVATIONS (Fowler heuristic smells — not hard violations)

- **Primitive Obsession (mild):** `TaskRow` in `App.tsx` L84–85 hardcodes `"Ready"` and `"Medium"` display strings instead of deriving from `task.status` / `task.priority`. Safe within Ticket 01's Ready-only scope; becomes a false-display bug when status/priority changes land.
- **Data Clumps:** `CreateTask` and `UpdateTask` (`tasks.rs` L120–131) share title+description fields without a shared validation struct. Idiomatic Rust; worth revisiting if more shared fields appear.
- **Speculative Generality: absent** — positive signal; no unnecessary adapter layers or premature abstractions.

---

### VERDICT

**PASS with two moderate recommendations.** No blocking standard violation found. The structured error identity gap (M1) and the hardcoded status display (observation) should be addressed before downstream tickets consume these interfaces, but neither blocks Ticket 01's stated scope. Native GUI acceptance remains open per the ticket's own evidence and is outside this code-standards axis.
