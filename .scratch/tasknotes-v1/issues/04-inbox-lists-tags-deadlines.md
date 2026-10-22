# 04: Inbox, Lists, tags, deadlines

**What to build:** Organize Tasks in Lists/Inbox, add tags and optional deadlines, show overdue indicators, and persist CRUD changes across restart. Test List ownership and relative date display.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** completed — owner approved the merge on 2026-10-22: PR #16 (merge commit `5d76baf`) and the redesign UI in PR #17. Schema v4 is forward-only: before installing a build from `main` over the owner's real database, take a verified backup.

## Acceptance criteria

- [x] Tasks can be created in Inbox or moved to a named List and still appear in All Tasks without duplication. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Users can add/remove tags and optional deadlines and see overdue indicators without violating the fixed status order. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] List/Task metadata and tag associations survive restart; nonexistent List references cannot orphan a Task. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Overdue behavior is deterministic with a controlled local clock/time zone in tests; no forced reminder is introduced. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Large Lists are browsed through a bounded query/viewport, not an unbounded render. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).

## Verification

- [x] Native GUI CRUD/quit/restart on Inbox, Lists, tags and deadlines passed T1–T9 on the redesign QA build with synthetic data (see "Native GUI acceptance on the redesign build — 2026-10-22" below).
- [x] Frontend and Rust test suites pass, including v1/v2/v3 to v4 migration and locale-boundary controls; evidence [here](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Independent Standards and Spec reviews: REQUEST CHANGES at `c8be24c` (2026-10-14), then two independent re-reviews on 2026-10-22 (`bbce8af`, `bbce8af..8f0280e`). Every proven finding was fixed with a RED→GREEN test. The last fixes (`fe50c2c` Unicode Tag fold and the redesign QA findings) were verified by tests and native QA, not by a further independent review.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments


## Implementation checkpoint — 2026-10-13

The user authorized Ticket04 implementation through the ongoing `/ask-matt` workflow after Ticket01–03 source merges. This is a TDD implementation branch **not** a deployed macOS app. The new local SQLite **schema v4** is forward-only, so no upgrade against real user Task data is permitted without a recoverable backup and separate owner approval.

**One-variable TDD history:** List schema/mutations, then Inbox/scoped Task views, then tag+deadline metadata, then React/Tauri command adapter and local-calendar overdue labels. The independent Issue19 QA lag investigation was **not** reopened. Removing a List is reversible and hides its Task children without deleting or mutating their work status/progress/history; the unified Removed view remains Ticket09's scope.

**Acceptance limitations:** The React mocked Tauri boundary and temporary SQLite Rust core behaviors are covered. A successful unsigned test-only Tauri bundle proves native compilation, **not** owner-operated GUI create/move/restart acceptance. A real native interaction with synthetic data and independent code-review findings remain explicit gates. No source merge/release authorized.


## Ticket04 follow-up — viewport and reviewer gate (2026-10-13)

The initial source candidate 286482fb60e04dad8d0628372efc981215882cb2 created Draft PR #16. Subsequent bounded-viewport RED→GREEN changed the old accumulating "Load more" behavior into **at most 50 visible Task cards per page**, reversible Next/Previous, and scope-reset protection. A 120-Task synthetic Tauri contract test proves the rendered count stays 50/50/20/50/50. Additional RED→GREEN test confirms a rejected List rename retains its unsaved name draft.

Current positive tests: **30 Rust integrations**, **30 React**, TypeScript typecheck and frontend production build; strict Tauri and Rust Clippy/fmt previously passed with unchanged native source. See [acceptance report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md) and [non-independent Standards/Spec self-review](../../../docs/reviews/ticket-04.md).

**Blocked gates:** AGY independent Standards and Spec reviewers both returned RESOURCE_EXHAUSTED (no actual review); the isolated native QA app rendered its GUI but WKWebView fields were inaccessible to AX, so actual click/type/reopen was not proven. Ticket stays **Implementation: review**; PR remains Draft and unmerged. Never migrate the owner's live SQLite without separately authorized recoverable backup and deployment.


## Independent reviewer follow-up — 2026-10-14

An independent reviewer checked Draft PR #16 at SHA c8be24c440c7b5612e3e47a3d45b01ec84bffc2b and returned **REQUEST CHANGES**: S-01 (old pagination reply overwrites a new Task), S-02 (old sort on create completion), and SPEC-01 (Overdue label does not advance at local midnight).

All three were reproduced as RED React/Tauri tests then fixed to GREEN. Further regression tests cover reverse-order responses, Task move while paging, and resumed app clock changes. The requested migration rollback fault injection passed using synthetic v3 SQLite. See the review response in docs/reviews/ticket04-independent-review-remediation-2026-10-14.md.

**Remaining:** independent re-review of the new SHA and actual macOS native GUI click/edit/restore/restart acceptance. Implementation remains in review; Draft PR stays unmerged. The ordinary user app and database are unchanged.


## Independent re-review of bbce8af — 2026-10-22

An independent reviewer confirmed S-01, S-02 and SPEC-01 are fixed at `bbce8af`, but returned **REQUEST CHANGES** for new findings. All were remediated with RED→GREEN tests:

| Finding | Severity | Remediation | Evidence |
| --- | --- | --- | --- |
| Unrelated list refresh wiped unsaved Task drafts (effect keyed on `task.tags` array identity) | Major | Key on tag contents (`7f44feb`) | R5/R5b RED on `bbce8af`, GREEN after |
| Same Tag stored with different casing per Task | Minor | Owner decision 2026-10-22: **first spelling wins**; `task_tags` stores canonical `tags.name` | `first_tag_spelling_is_canonical_across_tasks` |
| Case-only Tag edit / same-List move returned the Progress error message | Minor | New `TaskUnchanged` error | `case_only_tag_edit_and_same_list_move_report_no_change` |
| Task in a List beyond the loaded sidebar page displayed as Inbox | Minor | Disabled "Current List (not loaded)" option (`13ec175`) | R7 RED→GREEN |
| Any save jumped back to page 1 | Nit | Refresh the visible page cursor; fall back to page 1 only if it is empty | R6 RED→GREEN, R6b guard |

Second re-review of the remediation (`bbce8af..8f0280e`) confirmed every fix fails without its change, then found that SQLite `NOCASE` folds only ASCII: non-ASCII Tags (`Ärger`/`ärger`) escaped the canonical rule and save/read order differed. Fixed by a Rust-folded, UNIQUE `tags.fold` key in the **unmerged** v4 schema plus a canonical-name join on reads (tests `non_ascii_tags_share_one_canonical_spelling_and_order`, `reads_show_the_canonical_spelling_for_older_task_tag_rows`, both RED→GREEN). Any synthetic QA database already created at v4 by an earlier branch build lacks this column and must be recreated; the owner's database was never migrated. The reviewer's unconfirmed note about Tasks in removed Lists does not apply: those Tasks are hidden from every view until restore (Ticket 09 owns the Removed view).

Local checks after remediation: 41 React tests, typecheck, 35 Rust core tests, core and Tauri Clippy `-D warnings`, Rust fmt — all PASS (synthetic data only). **Remaining gates:** re-review of the remediation commits, native GUI click/type/restart acceptance, and owner merge approval (schema v4 is forward-only).


## Native GUI acceptance on the redesign build — 2026-10-22

The owner chose to accept Ticket 04 on the redesigned UI (PR #17, rebased onto the remediated PR #16; the pre-redesign UI is preserved as tag `archive/ui-legacy-ticket04`). Build: unsigned debug **Tasknotes Redesign QA.app** from `03b4c8f`, identifier `store.adamaja.tasknotes.redesignqa`, executable SHA-256 prefix `f784e79df7920dbe`, seeded with 59 synthetic Tasks / 2 Lists in its own app-data directory. The owner's database (`store.adamaja.tasknotes`) size and mtime were unchanged before and after.

Driven by the assistant through macOS computer use (real clicks, typing and native pop-up menus in the WKWebView window), then cross-checked with a read-only SQLite query:

| Case | Result |
| --- | --- |
| T1 initial render: dark theme, 50-row first page, High/Overdue/#Work pills | PASS |
| T2 create in Inbox, move to List, appears once in All tasks | PASS (DB: List=Home Errands, revision 2) |
| T3 unsaved Progress note and Tags drafts survive creating another Task | PASS |
| T4 typed `work, café` resolves to canonical `Work`, `Café` | PASS |
| T5 case-only Tag edit shows "Nothing to save…" | PASS |
| T6 deadline Overdue → Due today → cleared | PASS |
| T7 Priority change on page 2 stays on page 2 | PASS |
| T8 List rename; remove hides its Tasks; restore returns them unchanged | PASS |
| T9 quit and relaunch: Lists, Tasks, Tags, deadline, theme persist | PASS (DB integrity ok, 62 Tasks) |

**Findings for follow-up (not blockers of the behaviors above):** the sidebar count beside "All tasks" shows the current view's visible count (also present in the archived UI); Task rows show at most two Tags without a "+N" hint; after a "Nothing to save" error the Tags/Deadline draft has no discard control.

**Remediated the same day (owner-requested, TDD):** the count now sits beside the active view only; rows show a `+N` pill for hidden Tags; Tags/Deadline drafts have a "Discard tag and deadline changes" button; Task details say "List not loaded" instead of "Inbox" for an unloaded List. Four new RED→GREEN tests in `src/notes-redesign.test.tsx`; 50 React tests, typecheck and build pass. Rebuilt QA app checked visually (count beside Home, `+1` on a three-Tag row, discard restores saved Tags and unlocks fields; QA DB revision unchanged).
