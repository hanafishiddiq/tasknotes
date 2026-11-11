# 05: Sidebar counts of unfinished Tasks

**What to build:** The number beside All tasks, Inbox and every List in the sidebar counts that place's unfinished Tasks (Ready and In Progress), shown on every folder, not only the open one; a Settings option controls it (spec, "Sidebar counts", Q45–Q46).

**Blocked by:** None (Ticket 04 merged in PR #41)

**Status:** ready-for-agent

**Implementation:** in progress on `feat/batch3-sidebar-counts`

## Acceptance criteria

- [x] Each count is the core's number of that place's Ready and In Progress Tasks: Completed (Won't Do included) and Removed never count; the same rule for All tasks, Inbox and each List (a List counts only its own Tasks, as GLOSSARY "List" says).
- [x] Every folder shows its count, kept current after changes made here and from the CLI or MCP, without reading every Task.
- [x] A Settings choice says where counts show: every folder (the default), only the open folder, or none (Q48).
- [x] Counts keep the folder's tabular tertiary style and WCAG AA on every sidebar fill.
- [ ] The owner checks it in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md`, ADRs and `AGENTS.md` before changing behavior.
- Own branch, Draft PR, `/tdd`, then `/code-review`; merge needs the owner's confirmation.
- The count rule lives in the core, shared by the desktop, CLI and MCP.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner request (Han), during Ticket 04:** "the number in the sidebar should not show the total Tasks, only the active (Ready) ones". Today the number is the Tasks loaded on the open page (`tasks.length`, "+" when more pages wait), shown on the open folder only, Completed included. Q45: count Ready and In Progress (the recommendation). Q46 (changed from the recommendation): every folder shows its count, but it can be set from Settings; whether that is an on/off switch or a choice of where counts show is asked when the ticket starts.

**2026-11-11 — Q48 answered "Three choices"** (changed from the recommended on/off switch): Settings offers where counts show, every folder (the default), only the open folder, or none. Whether a List counts its sub-Lists needed no question: GLOSSARY "List" already says a List shows only its own Tasks.

**2026-11-11 — built (evidence):** core `TaskStore::unfinished_counts` is one grouped count with the same visibility rule as every view (`task_visible!`), so no Task is read; tests `each_place_counts_only_its_ready_and_in_progress_tasks` (Completed and Won't Do left out, a List without its sub-List's Task, an empty List left out) and `removed_tasks_and_the_tasks_of_a_removed_list_do_not_count` failed before the query existed. The shell answers `unfinished_counts`; the window reads it again whenever the Tasks or Lists on hand change or something is restored or changed outside, and keeps the choice in this window's storage, like the appearance. Window tests: every folder by default, "2 unfinished" heard after the name, nothing on an empty List; Open folder only and None, kept for the next launch; read again after a Task is added here and after the CLI adds one to another place while the page showing stays the same. Mutation checks: reading the counts once only, or ignoring the Settings choice, each fails a test. Chosen without asking: a place with none shows no number rather than "0"; the old page count's "+" is gone, since the core's number is exact. The look and a large real store wait for the batch-3 install build.

