# Tasknotes — Specification, batch 3

**Status:** in progress. Each feature is settled with the owner in its own grilling round, then added here; the owner asked to take the features one at a time (2026-11-11).
**Source:** the owner's "Tasknotes Features" List, Tasks added on 2026-11-11 after using the batch-2 build.
**Issue tracker:** `.scratch/tasknotes-batch-3/issues/`. Book: `progress/tasknotes-batch-3.md`.
**Delivery:** as in batch 2 (Q1, 2026-11-11): one Draft PR per ticket with an AI review gate; merging, the single install build, real data and cache removal each need the owner's confirmation. One install build after every ticket is on `main`; the batch-2 build caches stay until then.

## Problem Statement

Using the batch-2 build, the owner found that Move to Removed looks like any other button, the yellow mark on a Task opened from the new Task toast is too loud and does not last, a Task has no right-click menu, and the Pomodoro page only shows history while the controls and lengths live elsewhere.

## Solution

Five tickets, one at a time, each settled with the owner before it is built: the two yellow-mark Tasks from the List become one ticket, and the sidebar counts were asked for during Ticket 04.

## Features, in working order

1. **Red Move to Removed** — Ticket 01. Settled (Q3, 2026-11-11); merged in PR #38.
2. **New Task yellow mark** — Ticket 02, settled (Q4–Q16, 2026-11-11), replacing the 2026-11-09 decision (Q3/Q14/Q25: a toast instead of a lasting yellow mark). Whatever replaces the toast's flash must keep the red Move to Removed readable: on today's flash (`--selection` over the open Task) it measures about 2.66:1 dark and 4.29:1 light (PR #38 review).
3. **Task right-click menu** — Ticket 03, settled (Q20–Q24, 2026-11-11).
4. **Complete Pomodoro page** — Ticket 04, settled (Q25–Q44 and Q47, 2026-11-11).
5. **Sidebar counts of unfinished Tasks** — Ticket 05, asked during Ticket 04 (Q45–Q46); worked after Ticket 04.

## User Stories

1. As the owner, I want Move to Removed at the end of an open Task to read red (text and icon), with a faint red fill under the pointer, so it stands apart from Save; it stays a reversible move with no dialog. Notes keep their toolbar trash icon as it is.

### New Task yellow mark (Ticket 02, settled)

Owner answers, 2026-11-11 (Q5–Q8):

- Q5: opening a Task from the toast's "Click to edit" shows no outline and no flash over the open Task; the open Task keeps its usual open-row yellow.
- Q6: the mark on a Task the owner added in the window goes when any Task is opened (the toast, a click or the keyboard); several Tasks added in a row are all marked until then; switching List keeps it; quitting the app clears it (view state, never stored).
- Q7: the mark is the sidebar hover yellow (`--side-hover`) as a fill on the Task row, no border; it stays under the pointer.
- Q8 (changed from the recommendation): Tasks added by AI are marked too, so the owner sees what an AI added when looking at the List.
- Q4 (after a labelled picture of both): the toast stays alongside the mark, replacing the 2026-11-09 decision of a toast instead of a lasting mark. The toast now stays 10 seconds (was 6) and has an X that closes it at once.
- Q10: Tasks created through the CLI or MCP count as added by AI (the audit log names the interface and the Task; the app cannot tell an AI from the owner typing in Terminal); same yellow, no extra icon; Tasks an AI only changes are not marked.
- Q13: no toast for Tasks added by AI.
- Q11/Q14: an AI-added Task's mark goes 10 seconds after it is seen (window in front, row on screen), or when it is opened; the others stay.
- Q12/Q15: nothing is stored: a Task added while the app is closed gets no mark, and quitting clears every mark.
- Q17: a window hidden to the menu bar is not closed: a Task an AI adds then is marked when the window comes back. Q15 is about quitting (⌘Q).
- Q16: the owner's own adds keep Q6, not 10 seconds.
- Colour (Q18, owner-approved after a side-by-side picture): half the sidebar hover yellow over the canvas, the strongest mix that keeps row text and the status ring at WCAG AA.

### Task right-click menu (Ticket 03, settled)

Owner answers, 2026-11-11 (Q20–Q24, after a sketch):

- Q20: the menu holds Open (Close on the open Task); Ready, In Progress, Completed, Won't Do with the current one ticked and the ones the status control refuses dimmed; a Priority submenu with the five levels; Move to Removed. Lists, Difficulty, Tags and Deadline stay in the open Task.
- Q21 (changed from the recommendation): a custom menu in the app's own look, not the macOS one, so it matches the design (and Move to Removed can be red).
- Q22: each choice is the existing control's change: Completed on a Ready or In Progress Task is the round check (completion sound and Undo bar); Move to Removed is the red button (Undo, restorable); choices that change the Task are dimmed while it has unsaved edits, like the check.
- Q23: only on Task rows in the Task view; not in Removed, Pomodoro history, Notes or the sidebar.
- Q24: merge after green CI without asking again, then the Pomodoro page.

### Complete Pomodoro page (Ticket 04)

Owner answers, 2026-11-11 (Q25–Q30, after a sketch):

- Q25 (open): three sections, Timer (large time, status, the Task picker and the buttons the timer waits for, a line with the session in the cycle and today's count), Lengths, then the daily History; the owner asked to see it first, so an interactive preview was shown.
- Q26: the Pomodoro block at the foot of the sidebar stays as it is.
- Q27: the lengths and Silent move from Settings to the Pomodoro page; Settings keeps one line linking there.
- Q28: the Task picker is a searchable dropdown of every unfinished Task in every List (In Progress first, then Ready), "No Task" included, in the same look as the Task menu.
- Q29: a new Pause / Resume for focus and breaks: paused time never counts; the menu bar shows "⏸ 12:34" and offers Pause or Resume.
- Q30: the time in large numerals over a thin yellow progress line, no ring; the line does not animate under Reduce Motion.

Owner answers, 2026-11-11, after three previews (the first two were "too many elements, too much text"; the owner showed a two-pane reference screen):

- Q39: layout A. The first screen is the timer alone: the Task line above a ring (it fills as focus is spent; Overtime draws a second, inner lap), four cycle beads, one large button and a small Stop icon; today as a day rail under it (one mark per session at its time, today's focus at its end); the focus record below, with all-time totals in its heading. The lengths and Silent sit behind an icon at the top right. This replaces Q25's three sections, Q27's lengths section and Q30's thin line without a ring.
- Stop after the planned end (asked by the owner, answered from the core): the session is Finished with its planned length, Overtime is not counted, and no break starts. Unchanged.
- Q43 (changed from the recommendation): a quiet "Start break" under Pause while a focus runs or is paused; a break started before the planned end still counts the minutes focused. Stop before the planned end stays Stopped and uncounted.
- Q40: a button is yellow only while the timer waits for the owner (Start, Resume, Start break at the planned end, Start focus after a break); while it runs, Pause is a grey pill and Stop a small icon.
- Q41: Overtime draws a second, inner lap of the ring; four beads (the sessions before the long break) replace a "Session 2 of 4" line.
- Q42: all-time totals (sessions and focus minutes) come from a new read-only core query, with the same counting rule as each day.
- Q44: a focus ended early by Start break adds its minutes only: to the day's and the all-time focus, and as the Task's Progress Entry ("Pomodoro: 10 min focus"); it is not a session in the counts and does not move the cycle toward the long break. Under a minute, nothing is counted, as with Stop.
- Q47 (raised by the review, owner answer 2026-11-11 "Keep it"): while a focus or break is paused, the sidebar block shows a Resume button, as it shows every choice the timer waits for. Q26's "stays as it is" keeps the block's rules; it does not forbid that button.

### Sidebar counts (Ticket 05)

Owner request and answers, 2026-11-11, during Ticket 04:

- The number beside a sidebar folder should count only active Tasks, not every Task.
- Q45: active means unfinished: Ready and In Progress. Completed (Won't Do included) and Removed do not count.
- Q46 (changed from the recommendation, every folder): every folder shows its count, and Settings can change it; the option's form is asked when the ticket starts.
- Q48 (owner answer 2026-11-11, "Three choices", changed from the recommended on/off switch): Settings offers where counts show: every folder (the default, Q46), only the open folder, or none.
- A List counts only its own Tasks, never those of the Lists inside it, as GLOSSARY "List" already says; All tasks counts every unfinished Task not in Removed.

## Implementation Decisions

- Alert Red is the existing `--red` token (DESIGN.md), already readable in both appearances. The hover fill is the most red that keeps the label at AA in light appearance (numbers in Ticket 01).

## Testing Decisions

- Stylesheet rules and contrast are checked on App.css (`src/stylesheet-a11y.test.tsx`, `src/a11y-contrast.test.tsx`); the component wiring by an App test. The look is confirmed in the owner's click check after the install build.

## Acceptance Criteria

- Each ticket's criteria pass with recorded evidence, `/code-review` findings addressed, and its Draft PR reviewed.
- After all tickets merge: `pnpm test`, typecheck, build and `cargo test` green on `main`; one install build passes an owner click check on the real app.
- No permanent deletion, no network exposure, no cloud sync introduced; Exclude from AI still holds at the CLI and MCP.

## Out of Scope

- Assign Task to People (waits for multi-user work).
- Cloudflare D1 and web access.
