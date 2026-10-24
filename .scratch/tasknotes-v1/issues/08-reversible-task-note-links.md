# 08: Reversible Task–Note links

**What to build:** Link one or more Notes to a Task and one Note to multiple Tasks, navigate the relationship from each view and unlink without destroying either resource. Test link integrity after edits.

**Blocked by:** 04 — Inbox, Lists, tags, deadlines, 06 — Standalone Markdown Notes

**Status:** ready-for-agent

**Implementation:** in progress — implemented, awaiting independent review (2026-10-25). [Plan](../../../docs/plans/ticket-08-task-note-links-2026-10-25.md).

## Acceptance criteria

- [ ] One Note can be linked to several Tasks, and one Task can link to several Notes.
- [ ] Users can navigate relationships from both Task and Note views, with stable IDs and no duplicate associations.
- [ ] Unlinking never removes or modifies either linked Task or Note.
- [ ] Deleting a relationship is recoverable where Undo is offered, and invalid references cannot introduce dangling links.
- [ ] Relationships survive app restart and work across different Task Lists.

## Verification

- [ ] Run core link-integrity tests and desktop cross-navigation/link/unlink scenarios.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

## Implementation checkpoint — slices 1–3 (2026-10-25)

Not yet reviewed: the acceptance boxes above stay unticked until the independent review (`/code-review`) and the owner's native check.

**Decisions made by the implementer** (in the plan; the owner can change them): unlinking keeps the row (`unlinked_at`), and linking the pair again, including Undo, brings back the same link ID and its place in the list; linking an already linked pair returns that link; linking and unlinking never write the Task or the Note (revision and edit time unchanged); lists are most recently linked first in keyset pages; links are not filtered by Removed (left to Ticket 09).

**Slice 1, core links (schema v7), `5eb32b0`:** `task_note_links(id, task_id, note_id, linked_at, unlinked_at)`, `UNIQUE(task_id, note_id)`, both foreign keys `ON DELETE RESTRICT`, partial indexes per side. `link_note`, `unlink_note`, `list_task_notes`, `list_note_tasks` in `crates/tasknotes-core/src/links.rs`. 10 tests in `crates/tasknotes-core/tests/links.rs`:
`a_note_links_to_several_tasks_and_a_task_to_several_notes`, `linking_the_same_pair_again_keeps_one_link_with_its_id`, `unlinking_removes_only_the_relationship_and_undo_brings_back_the_same_link`, `unlinking_a_link_that_is_not_active_is_refused`, `a_link_to_a_missing_task_or_note_is_refused_and_nothing_is_stored`, `the_database_refuses_dangling_links_from_any_writer`, `links_survive_reopening_and_join_tasks_in_different_lists`, `linked_notes_come_in_bounded_pages_without_repeats`, `a_v6_database_gains_links_without_touching_its_tasks_or_notes`, `a_failed_v7_migration_rolls_back_and_keeps_the_v6_database_usable`. Seven mutation checks each make a test fail: no existence checks, no Note foreign key, unlink deleting the row, inclusive cursor, unlinked links still listed, relink with a new ID, unlinking twice allowed. Older migration tests now expect version 7 and drop the links table in their pre-v6 fixtures.

**Slice 2, Task → Note, `a636907`:** Tauri `link_note`, `unlink_note`, `list_task_notes`, `list_note_tasks`; the expanded Task page lists its Notes, links one from the recent Notes (already linked ones are not offered), opens a linked Note in Notes, and unlinks with Undo.
**Slice 3, Note → Task, `59dbf4a`, `6bd3bdf`:** an open (stored) Note lists its Tasks with work status above the text, links a Task found through the core Task search, unlinks with Undo; opening a linked Task shows only that Task, expanded, under "Showing one Task linked to a Note" with "Show all tasks". Each side reads its links again when shown, so a change made from the other side is visible on return.

App tests in `src/links.test.tsx` through the mocked Tauri seam (each RED first): lists the Task's linked Notes and opens one in Notes; links a Note picked from the recent Notes, once; unlinks a Note without touching it, and Undo brings the same link back; lists the Note's linked Tasks and opens one on its own, expanded; links a Task found by search, once; unlinks a Task without touching either side, and Undo brings the same link back; shows a link change made on one side when returning to the other. Eight UI mutation checks each make a test fail (linked Notes offered again, Undo doing nothing, open request ignored, picker left open, focus filter not sent, Task not expanded, linked Tasks offered again, search ignoring the text).

**Also fixed:** `notes.test.tsx` "loads older Notes beyond the first page" failed on the integration tip with a day-first date locale ("Note 11 Jan"); it now finds the Note by its title (`8c9fa0a`).

**Gates at `6bd3bdf`:** `pnpm test` 135 tests in 11 files; `pnpm typecheck`; `pnpm build` (main bundle 272 kB, Notes chunk 321 kB); core `cargo test` 70 tests; Tauri `cargo test` 1 test; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` for core and Tauri. All green. No native GUI QA was run (owner checklist below); the owner's database was never opened.

**Owner click-checklist (native, on an isolated QA data dir, e.g. `TASKNOTES_DEV_DATA_DIR=<scratch dir> pnpm tauri dev`):**

| Step | Expected |
| --- | --- |
| Create two Tasks in different Lists and two Notes. Open Task A, click **Link a Note**, pick Note 1 | Note 1 appears under "Notes" on Task A; the picker closes; Note 1 is no longer offered |
| Link Note 2 to Task A as well, then open Task B and link Note 1 | Task A lists two Notes; Task B lists Note 1 |
| On Task A click Note 1 | Notes opens with Note 1 selected; above its text "Tasks" lists Task A and Task B with their status |
| Click Task B in that list | The Task view shows only Task B, expanded, under "Showing one Task linked to a Note"; **Show all tasks** brings back every Task |
| On Note 1 click the "x" beside Task A | Task A leaves the list; "Unlinked "Task A". The Task itself is unchanged." with **Undo**; Task A's title, status and progress are unchanged; Note 1's text is unchanged |
| Click **Undo** | Task A is back in the same place in the list |
| Go to All tasks and open Task A | Its "Notes" list matches what Note 1 shows (unlinking on one side shows on the other) |
| **Link a Task** on a Note, type part of a title | Matching Tasks appear, already linked ones left out; picking one links it |
| Quit (Cmd+Q) and relaunch | Every link is still there, both ways |
| Check both appearances (sidebar toggle) | Link lists follow DESIGN.md "Task–Note Links": no shadows, yellow only for the Link/Undo text actions |

