# 04: Complete Pomodoro page

**What to build:** The Pomodoro page in layout A (spec, "Complete Pomodoro page", Q25–Q44): the timer alone on the first screen (Task line, ring, cycle beads, the buttons the timer waits for), today as a day rail, the focus record below with all-time totals, and the lengths behind an icon; plus Pause / Resume and an early Start break in the core timer and the shell.

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR #41 on `feat/batch3-pomodoro-page`; two-axis review done and its findings fixed; merge waits for the owner

## Acceptance criteria

- [x] Pause / Resume during a focus or a break: paused time counts as neither focus, break nor Overtime, and a planned end never passes while paused; Stop while paused is Stop at the pause. The menu bar title shows "⏸ 12:34" and its menu offers Resume (Pause while running); the sidebar block shows the paused state.
- [x] Start break while a focus runs or is paused ends it early: its minutes count as focus time for the day, the all-time totals and the Task's Progress Entry, but not as a session nor toward the long break (Q43–Q44); under a minute it is stored as Stopped. The menu bar offers it too. CLI and MCP history name such a session as ended early.
- [x] Past the planned end, Stop keeps storing a Finished session without Overtime (unchanged).
- [x] The first screen of the page: the Task line (a link that opens the picker while idle; the Task, or "Next: …" during a break, otherwise), the ring (fills as focus is spent; grey in a break; dim while paused; a second inner lap for Overtime), the time in large tabular numerals with a one-word state, four cycle beads (screen readers hear "Session 2 of 4"), and one button that is yellow only while the timer waits for the owner, grey Pause while running, a small Stop icon, and a quiet second choice (Count +07:12, Start break, Skip to focus).
- [x] The Task picker is a searchable sheet in the Task menu's look: "No Task" and every unfinished Task in every List, In Progress first, with the List name; it opens on the chosen Task; keyboard and pointer as the Task menu.
- [x] The lengths (Focus, Break, Long break, Long break after) and Silent sit in a sheet behind an icon at the top right, saved with Save; Settings keeps one line linking to the page.
- [x] Today's day rail: one mark per session at its time (Finished and ended early filled, Stopped hollow, the running focus dim), now as a thin line, today's counted focus at its end; each mark names its times, Task and outcome to the pointer and screen readers; the record below says the same for the keyboard.
- [x] The focus record below: all-time sessions and focus from the core in its heading, then each day as today's history does (newest first, its sessions oldest first), with "Load more days".
- [x] Every text keeps WCAG AA in both themes; the paused pulse and the ring's colour fades stop under Reduce Motion.
- [ ] The owner checks it in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md`, ADRs and `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; merge needs the owner's confirmation.
- Timer rules live in the core's Pomodoro timer and the shell; the window only shows what the shell sends.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner decisions (Han), Q25–Q30:** see the spec. Q25 asked to see the page first; Q28 asked for a dropdown in the Task menu's look.

**2026-11-11 — owner feedback on the first preview (Q25):** too many elements and too much text; the owner showed a reference Pomodoro screen (two panes: a timer with a ring, a Task link above it and one large button; an overview of four numbers and a focus record by day) and asked for something that simple. A second preview in Tasknotes' own look was shown with: the ring (revisiting Q30's thin line), the lengths behind an icon (revisiting Q27's section), four numbers (today's and total sessions and focus), and a focus record by day. Conflict raised to the owner: DESIGN.md refuses cards ("no hero, no cards, no gradients, no colored badges"); the preview offers the four numbers as cards or flat.

**2026-11-11 — third preview, three layouts (owner asked for a "very creative and good, but still simple" design):** one interactive page with three layouts in Tasknotes' tokens, every state (idle, focus, paused, focus done with Overtime, break, break over), the Task picker and the lengths sheet, in dark and light. Shared ideas: the ring fills as focus is spent and draws a second, inner lap for Overtime; four beads under the timer show the cycle instead of "Session 2 of 4"; a button is yellow only while the timer waits for the owner (Start, Resume, Start break, Start focus), grey while it runs (Pause); Stop is a small icon. Layouts: A, the timer alone on the first screen with today drawn as a day rail (one mark per session at its time), the focus record below; B, the owner's reference in two panes (timer left, four flat numbers and the record right); C, one column like the Task pages, big digits over a thin line, the running session as a live row at the end of Today. Not yet chosen.

**2026-11-11 — owner chose layout A (Q39)** and asked for a "Start Break" button (where exactly is being confirmed, Q43) and whether the square (Stop) still counts after a focus ends. Answer from the core (`PomodoroTimer::stop`, test `stopping_after_the_planned_end_finishes_without_overtime`): past the planned end, Stop stores the session as Finished with its planned length (it counts toward the day, the totals and the long break), Overtime is not counted, and no break starts; only Count +mm:ss counts Overtime. The preview now does the same (it had shown such a Stop as Stopped). Proposal shown in the preview for Q43: a quiet "Start break" under Pause while a focus runs or is paused, the mirror of "Skip to focus" during a break; a break taken before the planned end stores the focus as Stopped with its minutes (not counted), as Stop does. Q40–Q42 still open.

**2026-11-11 — Q43 answered: "Tetap dihitung"** (changed from the recommendation): a break started early still counts the minutes focused. Open as Q44: whether such a focus also counts as a session and toward the long break, or adds its minutes only (the preview shows minutes only). Storing it needs a schema change, since a Finished session today must have reached its planned length.

**2026-11-11 — Q40–Q42 and Q44 answered "ya semua"**: the design is settled (see the spec). Storing a focus ended early needs a third session outcome in the store (schema change).

**2026-11-11 — built (evidence):** core `cargo test` with new timer tests (pause, resume, break early, the cycle, the length under way) and store tests (Ended Early Progress Entry, v23→v24 migration keeping every row, all-time totals counted as the days are); shell tests for the menu (Pause / Resume / Start break) and the view; window tests for the page (timer, picker, lengths, rail, record), contrast for every new colour in both themes (three light-theme pairs on the sheet's gray failed at 2.9:1 and now take accent text), axe and keyboard scenes for the timer, the picker and the lengths sheet. Seen in the debug app on synthetic data: idle, picker, focus, paused and the lengths sheet. One fix from that check: the main button kept the previous yellow while it faded to gray, so it is now a new button per choice with no colour transition. Not decided by this ticket and left as it was: the sidebar keeps All tasks highlighted on other pages; in the debug window the Pomodoro folder also showed no current-page gray, although the code and a browser check give it one (to look at in the installed app).

**2026-11-11 — two-axis review (Standards, Spec) and what changed:**
- The counting rule lived twice (Rust for the days, SQL for the all-time totals); the totals now count each stored session with the same Rust rule as the days.
- The contrast check did not see strokes, fills, inset outlines or opacity; it does now, and measures the break knob, the "now" bead, the Stopped marks and the picker's search underline. It caught one real miss: a paused time breathed down to 2.8:1 in light appearance, so it now fades to 60% (3:1 at its faintest) instead of 45%.
- The paused ring had been kept bright for contrast, which departed from the preview the owner chose; it is dim again (arc 45% into the canvas, hollow knob), recorded as a mark said in words (the time and "Paused") rather than measured.
- The day rail drew the focus under way from "now minus the seconds focused", so a pause or sleep moved its start; the shell now sends when the focus started (`focusStartedAt`), and the rail reads the wall clock once a minute for "now" (it had frozen while paused).
- The Task picker read only the first 50 Tasks of each group; it now reads page after page (as the list is scrolled or the highlight reaches its end, and on until the chosen Task is shown), and the highlight stays in view.
- The beads' long-break rule moved from the page into the core (`finished_in_cycle` keeps the closed cycle during a long break).
- An early Start break after four Finished sessions with no break between (the fourth was stopped past its end) starts the long break: the four earned it, and Ended Early still never moves the cycle (test `a_break_started_early_is_the_long_one_once_the_cycle_has_earned_it`).
- CLI and MCP tests now show a focus ended early as `ended_early` with its minutes counted.
- Smaller tidy-ups: one module for the Pomodoro formatting the sidebar block, page, rail and record share; one placement helper for the Task menu and the page's sheets; the history-era names (`openPomodoroHistory` …) renamed for the page; DESIGN.md records the exceptions it lacked (accent text on the gray sheets and the Overtime lap, inset outlines, the 1.6s breathing, the 14px pill).
- Left as is, by judgement: the page, the sidebar block and the menu bar each lay out the timer's choices themselves (each shows a different set, as DESIGN.md says); `stopped_after_seconds` keeps its name, as it is part of the CLI and MCP output.
- For the owner: the sidebar block shows a Resume button while paused, which the spec did not ask for (Q26 says the block stays as it is).

