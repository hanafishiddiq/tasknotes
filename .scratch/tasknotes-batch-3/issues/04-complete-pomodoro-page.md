# 04: Complete Pomodoro page

**What to build:** The Pomodoro page in layout A (spec, "Complete Pomodoro page", Q25–Q44): the timer alone on the first screen (Task line, ring, cycle beads, the buttons the timer waits for), today as a day rail, the focus record below with all-time totals, and the lengths behind an icon; plus Pause / Resume and an early Start break in the core timer and the shell.

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — design settled 2026-11-11 (Q39–Q44)

## Acceptance criteria

- [ ] Pause / Resume during a focus or a break: paused time counts as neither focus, break nor Overtime, and a planned end never passes while paused; Stop while paused is Stop at the pause. The menu bar title shows "⏸ 12:34" and its menu offers Resume (Pause while running); the sidebar block shows the paused state.
- [ ] Start break while a focus runs or is paused ends it early: its minutes count as focus time for the day, the all-time totals and the Task's Progress Entry, but not as a session nor toward the long break (Q43–Q44); under a minute it is stored as Stopped. The menu bar offers it too. CLI and MCP history name such a session as ended early.
- [ ] Past the planned end, Stop keeps storing a Finished session without Overtime (unchanged).
- [ ] The first screen of the page: the Task line (a link that opens the picker while idle; the Task, or "Next: …" during a break, otherwise), the ring (fills as focus is spent; grey in a break; dim while paused; a second inner lap for Overtime), the time in large tabular numerals with a one-word state, four cycle beads (screen readers hear "Session 2 of 4"), and one button that is yellow only while the timer waits for the owner, grey Pause while running, a small Stop icon, and a quiet second choice (Count +07:12, Start break, Skip to focus).
- [ ] The Task picker is a searchable sheet in the Task menu's look: "No Task" and every unfinished Task in every List, In Progress first, with the List name; it opens on the chosen Task; keyboard and pointer as the Task menu.
- [ ] The lengths (Focus, Break, Long break, Long break after) and Silent sit in a sheet behind an icon at the top right, saved with Save; Settings keeps one line linking to the page.
- [ ] Today's day rail: one mark per session at its time (Finished and ended early filled, Stopped hollow, the running focus dim), now as a thin line, today's counted focus at its end; each mark names its time, Task and length to pointer, keyboard and screen readers.
- [ ] The focus record below: all-time sessions and focus from the core in its heading, then each day as today's history does (newest first, its sessions oldest first), with "Load more days".
- [ ] Every text keeps WCAG AA in both themes; the paused pulse and the ring's colour fades stop under Reduce Motion.
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
