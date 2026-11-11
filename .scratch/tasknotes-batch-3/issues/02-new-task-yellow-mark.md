# 02: New Task yellow mark

**What to build:** A pale-yellow mark on Tasks just added, by the owner in the window or by an AI through the CLI or MCP; the new Task toast stays 10 seconds with an X; opening a Task from the toast shows no outline or flash (spec, "New Task yellow mark", Q4–Q13).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — reviewed in PR #39; merge after green CI (owner Q19); the owner's native check remains

## Acceptance criteria

- [x] The toast stays 10 seconds, held while the pointer or focus is anywhere on it, including moving focus between its two buttons. — `src/new-task-toast.test.tsx` ("shows for 10 seconds…", "pauses while hovered or focused…": 4 s, held, then 6 s left).
- [x] Its X ("Dismiss") closes it at once, without a fade, and opens nothing; a later add brings it back with its full time. — `src/new-task-toast.test.tsx` ("goes at once from its X…").
- [x] Opening a Task from the toast lays no outline and no flash over it; the open Task keeps its usual open-row yellow. — App test "clicking it scrolls to the new Task and opens it, with no outline or flash"; stylesheet test: no `.task-slot--flash` rule and no outline on `.task-slot`.
- [x] The X is readable on the toast's hover-gray box (3:1 for an icon-only control). — `src/a11y-contrast.test.tsx`, `.task-toast-close` and its hover.
- [x] A Task the owner adds in the window is marked (pale-yellow fill on its row, no border, kept under the pointer) until any Task is opened, including from the toast; several adds in a row are all marked; nothing is stored, so quitting clears them. — `src/new-task-mark.test.tsx` ("is marked, with every add in a row…", "loses its mark when the toast opens it", "forgets a Task moved to Removed, and stores nothing"). Switching List keeps them by construction: the marks live in App state, not in the view.
- [x] A Task created through the CLI or MCP while the window is open is marked the same way, with no toast; its mark goes 10 seconds after its row is half on screen with the window in front, or when it is opened; the others stay. — core `tests/audit.rs` ("the_window_learns_which_tasks_were_created_outside": only CLI/MCP `task.create`, in order, once); `src/new-task-mark.test.tsx` ("is marked with no toast, and goes 10 seconds after it is seen", "waits while the window is not in front", "keeps the others marked when one of them is opened").
- [x] A Task created while the window was closed gets no mark (Q15). — core test (the first look reports none); App test "gets no mark when it was added before the window opened".
- [x] Text and marks on the row keep WCAG AA on the mark: title, subtitle, progress, metadata, priority and deadline colours, tags, and the status ring, dot and disc. — `src/a11y-contrast.test.tsx`, surface `new-mark`. Measured by running this test at each mix during development: the full sidebar hover yellow failed nine pairs (dark: Overdue red, the "+N" tag; light: High and Very High orange, Overdue, Due today, the In Progress ring and dot, the Completed disc); 60% still failed five, 52% one; 50% passes all. The opened Task never carries the mark, so the red Move to Removed never sits on it.
- [x] Screen readers hear "Just added" on a marked row, and as the description of its open button when moving between titles. — `src/new-task-mark.test.tsx`.
- [x] A Task open already when the check names it gets no mark; opening any Task, even the one open already, ends the owner's own marks. — App test "gets no mark when it is open already…" (fails without the fix); every open goes through `openTask` in `App.tsx`.
- [ ] The owner sees the toast, the X and the mark in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md` and `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner decisions (Han), Q4–Q13:** see the spec's "New Task yellow mark" section. Q4 replaces the 2026-11-09 decision (batch 2 Ticket 07: "toast instead of a Recently added section or a lasting yellow mark"): the toast stays and a lasting mark is added. Out of this ticket: the reorder notice above the toast keeps its 2 seconds and has no X; the completion Undo bar keeps its 6 seconds.

**2026-11-11 — owner decisions (Han), Q14–Q16:** an AI-added Task's 10 seconds start when it is seen (window in front, row on screen); opening it first ends it at once. Tasks added while the app is closed get no mark, and nothing is stored ("my earlier decision", read as Q6). The owner's own adds keep Q6 (until any Task is opened), not 10 seconds.

**2026-11-11 — colour (technical call, reported to the owner):** Q7 asked for the sidebar hover yellow; the mark uses half of it over the canvas (dark `#353121`, light `#fdf6d0`), the strongest mix that keeps every row text and the status ring at WCAG AA.

**2026-11-11 — review on PR #39:** fixed: the core bounds both outside queries by `latest`, so a Task created between its reads is named once (it could have been marked a second time); every way of opening a Task goes through `openTask`, so re-opening the Task already open also ends the owner's marks; a Task open already when the check names it is not marked; "Just added" also describes the open button; `rowProps` keeps the row wiring in the hook; `createdTasks` is required (test mocks send it); `.task-toast--closable` replaces `.task-toast--new`, which read like the mark; DESIGN.md's Sidebar Hover Yellow names its new use. Accepted: a Task the CLI creates and removes before the next check keeps a waiting mark in memory; if it is restored, it shows the mark until seen for 10 seconds. Raised to the owner as Q17 (below).

**2026-11-11 — owner decisions (Han), Q17–Q19:** Q17: with the menu-bar item on, closing the window only hides Tasknotes; a Task an AI adds then is marked and counts its 10 seconds once seen; Q15 (no mark) applies to quitting with ⌘Q. Q18: the mark stays at half the sidebar hover yellow. Q19: merge after green CI.
