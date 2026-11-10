# 14: Pomodoro daily history

**What to build:** A Pomodoro page with daily history (spec story 21).

**Blocked by:** 02 — Pomodoro timer with notifications and Overtime

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] A Pomodoro page lists days with session count, total focus minutes (planned + Overtime of finished sessions) and each session's time and Task. _Evidence: one core rule, `TaskStore::pomodoro_history` (`crates/tasknotes-core/src/pomodoro_history.rs`): local days newest first, keyset pages of days (cursor `p1|YYYY-MM-DD`), each day's `finished_sessions`, `focus_minutes` (planned plus counted Overtime of Finished sessions, in the whole minutes each session shows, the Progress Entry's rounding) and every session newest first. `tests/pomodoro_history.rs` (12): `a_day_counts_its_finished_sessions_and_their_planned_plus_counted_overtime` (25 + 7:12, 25, Stopped → 2 sessions, 57 min), `days_follow_the_wall_clock_of_the_owners_time_zone`, `a_session_that_runs_past_midnight_belongs_to_the_day_it_started`, `the_ends_of_a_range_are_local_midnights_not_utc_ones`, `the_day_total_is_the_sum_of_the_minutes_its_sessions_show`, `each_session_names_its_task_no_task_or_a_task_in_removed` (renamed title shows; Task or List in Removed → no title, `task_removed`), paging, empty, refusals, same answer for every interface. Page (`src/components/PomodoroHistoryView.tsx`, quiet sidebar row "Pomodoro history" under the Pomodoro block and ⌘K): `src/pomodoro-history.test.tsx` (12) shows "2 sessions · 57 min focus · 1 stopped", rows "9:00 AM · Synthetic plan · 25 + 7 min · Finished", "No Task", "Task removed", the midnight/time-zone line, Load more days, the Task link opening the Task alone, an empty state without numbers, an unreadable history; reads again when the shell says `sessionStored` (not on ticks) and after outside changes, keeps loaded days and never skips a day when a read overtakes Load more. Axe and keyboard scenes "Pomodoro history: days/empty"; contrast table updated. **Open: a look at the page in the real app window (checked only in a browser with a fake Tauri seam, dark, light and 400 px).**_
- [x] Stopped sessions are listed as Stopped and not counted. _Evidence: `a_day_of_only_stopped_sessions_is_listed_with_nothing_counted` (0 sessions, 0 min, 1 stopped) and the first core test; CLI `history_lists_days_newest_first_with_what_each_counts`; the page lists "25 min planned · Stopped" and leaves it out of the count._
- [x] CLI and MCP can read the history by day range. _Evidence: `tasknotes pomodoro history [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--limit DAYS] [--cursor C]` and the read-only MCP tool `pomodoro_history` (`from`, `to`, `limit`, `cursor`), both the contract's page envelope (`contract::pomodoro_day`) on `TimeZone::Local`. `crates/tasknotes-cli/tests/pomodoro.rs` (7, `TZ` pinned per process): local days in UTC+7 and UTC-12, `--from/--to`, paging, refusals (`invalid_input`, `invalid_cursor`), grant required, a daylight-saving change whose clock goes back past midnight (`XST5XDT,M3.2.0,M11.1.0/0:30`), and no Note text (an Exclude-from-AI Note linked to the Task never appears). `crates/tasknotes-mcp/tests/pomodoro.rs` (5): read-only annotation, the same counts, range and paging, refusals, no Note text; `protocol.rs` lists 48 tools._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han), Q9/Q19.**

**2026-11-11 — implementation notes (AI), Draft PR:**
- No schema change: the history reads Ticket 02's `pomodoro_sessions` (index on `started_at`) with the Task's title only while the Task is shown (`task_visible`). Sessions hold no user text; Task titles go to AI interfaces like everywhere else (Tasks are never Exclude from AI, ADR 0004).
- Days come from the Mac's clock (`localtime_r`) at each session's start; the window's command and the CLI/MCP use `TimeZone::Local`, tests use fixed offsets and POSIX `TZ` strings. The SQL reads a day's margin around the range and the day is decided in Rust, so a local midnight is never mistaken for a UTC one.
- The shell's Pomodoro event now carries `sessionStored`, so the page reads again only when a session was stored, not on the one-second ticks.
- Decisions made without the owner (say if you prefer otherwise): (1) the day total is the **sum of the minutes each session shows** (25 + 7, rounded per session like the Progress Entry), so the numbers on the page add up; the unrounded seconds can differ by under a minute per session. (2) A Stopped session shows its **planned** length ("25 min planned"), not the time until Stop, since wall time may include sleep. (3) The day line also says "· 1 stopped" when there were Stopped sessions. (4) The sidebar row is named **"Pomodoro history"** (the control above it is already "Pomodoro"); the page headline is "Pomodoro". (5) A ⌘K "Pomodoro history" command. (6) Sessions within a day are newest first, like the days.
- Known limit: a session whose Task no longer existed when it was stored (restored away, Ticket 02) has no Task id, so it shows "No Task", not "Task removed".
- Still open: the look of the page in the real app window with synthetic data (the screen awake).
