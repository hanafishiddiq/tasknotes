# 02: Work status, priority, and fixed ordering

**What to build:** Change Task status and all five priority levels from UI, see intuitive symbols/colors and predictable In Progress → Ready → Completed grouping. Enforce 100%-is-Completed and progress-driven work status behavior in the core, not only React.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** review — native GUI priority/status/progress/reopen verified; sorting toggle acceptance and independent review remain pending; stacked merge blocked by Ticket 01.

## Acceptance criteria

- [x] Users can choose Ready, In Progress, or Completed and all five priorities; every priority has its agreed color, icon and accessible text label. **Evidence:** native user selected In Progress/Completed and Very High; Rust/React behavior tests cover all five priority options and accessible labels; native screenshot showed selected priority/status/progress.
- [x] All Tasks always renders In Progress above Ready above Completed, regardless of chosen secondary sort. **Evidence:** native Rust sorted pagination tests (priority/newest) and React grouping/sort tests.
- [x] Ready is for not-yet-started Tasks; once a Task has started, becoming idle does not change status or its saved progress. **Evidence:** Rust lifecycle/restart test; idle In Progress remains unchanged.
- [x] Progress 1–99% on a Ready Task moves it to In Progress; 100% moves it to Completed; contradictory status/progress writes are rejected, not silently coerced. **Evidence:** Rust transition-table/negative controls and React 60%→100% test.
- [x] Switching a partly completed Task back to Ready may not silently discard progress; such a request requires an explicit reset/validation path, not an inactivity trigger. **Evidence:** rejected status mutations leave data unchanged; no implicit reset operation or inactivity timer.
- [x] UI and Rust core tests agree on ordering and transitions, including Tasks sharing the same priority. **Evidence:** 13 React and 11 Rust integration tests, tie-breaking stable across pages/restarts.

## Verification

- [x] Test state-transition table via Rust core and verify ordering, icon labels, and validation at the React/Tauri command seam.
- [ ] Native user exercised priority, status, 60% progress, completion/reopen and app restart with persisted SQLite and screenshot. **Still unverified:** manually switching the sorting dropdown and observing the ordered groups in native WebView; see [QA report](../../../docs/qa/native-acceptance-2026-10-12.md).
- [x] Automated tests pass: 13 React tests; 11 Rust core integration tests; TypeScript typecheck; Clippy with `-D warnings`; rustfmt; native unsigned debug `.app` build.
- [x] Native app launched against synthetic data on macOS and created schema v2 with both ordering indexes, without accessing private user data.
- [x] Document two-axis Standards/Spec review at `docs/reviews/ticket-02.md` (manual review without independent subagents). Native GUI verification and Ticket 01 dependency remain open.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

## Dependency handling

- Explicit authorization to implement Ticket 02 received on 2026-10-11.
- Ticket 01 remains in review, not completed; this implementation is stacked on its verified code and cannot be merged until Ticket 01 acceptance and merge are resolved.
- Preserve the blocking edge in the tracker and keep PR targeting the Ticket 01 branch, not `main`.

## Verification evidence — 2026-10-11

- **TDD:** Initially failing `ChangeTask` and `SortMode` core tests; red React grouped-row/priority-change tests; later red migration regression and red write-success/refresh-error test, each followed by passing implementation.
- **Core:** `TaskStore::change_task` persists priority, status, and progress in a SQLite immediate transaction with optimistic revision checks. Completion sets 100%; reopening a Completed Task restores the previous incomplete percentage; idle In Progress does not become Ready. Inconsistent writes reject with descriptive errors and no mutation.
- **Migration:** Atomic schema v1→v2 migration adds last-incomplete progress and two grouped-sort indexes. A pre-existing task at 60% remains recoverable after completion/reopen; original title/description remain intact. Old binaries should not reopen the upgraded database; backup before deploying schema changes.
- **Sorting:** Keyset cursor pagination for Priority and Newest, grouped In Progress→Ready→Completed, stable tie-breaking by `created_at DESC, id DESC`. A cursor from another sorting mode is rejected.
- **Desktop:** React displays the five colored/icon-labeled priority badges and three status groups; inline controls call Rust through typed Tauri commands; text/progress drafts protected from unintentional lost edits; failed reads after successful writes are clearly distinguished.
- **Automated QA:** 13 React tests; 11 Rust integration tests; TS typecheck; frontend production build; Rust core Clippy with warnings denied; rustfmt; native Tauri debug bundle generated (about 31 MB). Actual `Tasknotes.app` launch initialized schema v2 and two ordering indexes in isolated synthetic data.
- **Remaining:** Ticket 01 manual native GUI create/edit/restart has not passed; this stacked change cannot be merged to main until its parent is accepted and merged. Ticket 02's manual native WebView workflow has also not yet been exercised by a GUI tester.

## Native acceptance update — 2026-10-12

- User selected status and priority through real native GUI. Stored `status=in_progress`, `priority=very_high`, `progress=60`, with Completed→In Progress recorded as 100→60; still present after native quit/restart.
- Native sorting dropdown change still lacks manual evidence. Independent Standards/Spec review has not been completed; Copilot review requests failed due quota. See [QA report](../../../docs/qa/native-acceptance-2026-10-12.md).
- User reports severe desktop lag, tracked in [Issue 19](19-native-interaction-lag.md) for a measured future fix before Ticket 17/18 acceptance.
