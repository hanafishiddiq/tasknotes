//! The Pomodoro daily history (Ticket 14): the stored sessions grouped by
//! the local calendar day they started on, newest day first, with each day's
//! count and focus minutes and its sessions in the order they started. The
//! one rule the window, the CLI and MCP read:
//!
//! - Only Finished sessions count as sessions. A Stopped one is listed,
//!   never counted.
//! - A Finished session's focus is its planned length plus the Overtime the
//!   person counted with "Count +mm:ss", each rounded to the nearest whole
//!   minute as its Progress Entry says it ("25 + 7 min"); an Ended Early
//!   one's is its focus before Start break, rounded the same way ("10 min";
//!   batch 3, Q43–Q44). A day's focus minutes, and the all-time totals, are
//!   the sum of what its sessions show.
//! - A session belongs to the day it started on, on the wall clock it
//!   started on: the offset from UTC the Mac had at its start (schema v23),
//!   so travelling or a daylight-saving change later never moves it, and
//!   one that runs past midnight stays on its first day. A session stored
//!   before v23 kept no offset; it goes on the reading zone at its start.
//!
//! Sessions hold no user text; the only text here is a Task's title, and
//! Tasks are never hidden from AI interfaces (only Notes are, ADR 0004). A
//! Task in Removed, or in a Removed List, shows no title.

use std::collections::BTreeMap;

use rusqlite::params;
use serde::Serialize;

use crate::calendar::{DAY_MS, TimeZone, day_at, format_date, parse_date};
use crate::pomodoro::{
    PomodoroSession, SESSION_COLUMNS, SessionOutcome, read_session, whole_minutes,
};
use crate::tasks::{TaskError, TaskStore};

/// Which days to read: `from` and `to` are local dates (`YYYY-MM-DD`),
/// both included, each session on its own wall clock; either left out
/// leaves that end open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRange {
    pub from: Option<String>,
    pub to: Option<String>,
    /// The wall clock of the sessions stored before schema v23, which kept
    /// no offset of their own: the Mac's own ([`TimeZone::Local`]) for every
    /// interface.
    pub zone: TimeZone,
}

/// Days, newest first; `next_cursor` reads the days before the last one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroHistoryPage {
    pub items: Vec<PomodoroDay>,
    pub next_cursor: Option<String>,
}

/// One local calendar day that has at least one session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroDay {
    /// `YYYY-MM-DD` on the wall clock its sessions started on.
    pub date: String,
    /// Finished sessions: the ones that count.
    pub finished_sessions: i64,
    /// Planned plus counted Overtime of the Finished sessions, and the focus
    /// of the Ended Early ones, in the whole minutes each session shows.
    pub focus_minutes: i64,
    /// Listed, never counted.
    pub stopped_sessions: i64,
    /// Every session that started this day, in the order they started.
    pub sessions: Vec<HistorySession>,
}

/// A stored session as the history shows it, in the whole minutes it shows
/// and with its Task's title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySession {
    pub id: String,
    pub task_id: Option<String>,
    /// Unix milliseconds.
    pub started_at: i64,
    pub ended_at: i64,
    /// The offset east of UTC, in seconds, of the wall clock this session's
    /// day and start time are on: the Mac's when it started, or for a
    /// session stored before schema v23 the reading zone's at its start.
    pub utc_offset_seconds: i32,
    pub outcome: SessionOutcome,
    /// The planned length in whole minutes, as shown ("25").
    pub planned_minutes: i64,
    /// The counted Overtime in whole minutes ("7" in "25 + 7 min"); zero
    /// for a Stopped session.
    pub overtime_minutes: i64,
    /// A Stopped session's focus before Stop ("Stopped after 12 min"), or an
    /// Ended Early one's before Start break ("10 min"), in whole minutes,
    /// sleep and pauses never included; `None` for a Finished session and
    /// for a Stopped one stored before schema v23, which kept no time.
    pub stopped_after_minutes: Option<i64>,
    /// The Task's title while it is shown; `None` without a Task or while
    /// it is in Removed (`task_removed`).
    pub task_title: Option<String>,
    /// It was for a Task that is now in Removed or in a Removed List.
    pub task_removed: bool,
}

impl PomodoroHistoryPage {
    /// This page cut to at most `max_sessions` sessions in whole days, for
    /// an interface whose answers must stay small (MCP). The first day is
    /// always whole, however many sessions it has; when days are cut,
    /// `next_cursor` continues right after the last day kept.
    pub fn capped(mut self, max_sessions: usize) -> Self {
        let mut held = 0;
        let fit = self
            .items
            .iter()
            .take_while(|day| {
                held += day.sessions.len();
                held <= max_sessions
            })
            .count()
            .max(1);
        if fit < self.items.len() {
            self.items.truncate(fit);
            self.next_cursor = self.items.last().map(|day| cursor_after(&day.date));
        }
        self
    }
}

/// The most days one page may hold.
const MAX_DAYS_PER_PAGE: u32 = 100;
const CURSOR_PREFIX: &str = "p1|";

/// Going back in time, a session's local date never steps forward by more
/// than this many days: every offset (stored, or the reading zone's) lies
/// within 18 hours of UTC, so two wall clocks are under 36 hours apart (a
/// daylight-saving change, or a flight across the date line).
const MAX_DAYS_FORWARD: i64 = 2;

fn cursor_after(date: &str) -> String {
    format!("{CURSOR_PREFIX}{date}")
}

/// Every session in a range of start times, newest first, with its Task's
/// title only while that Task is shown. The first nine columns are
/// `read_session`'s.
const HISTORY_SQL: &str = concat!(
    "SELECT s.id, s.task_id, s.started_at, s.planned_seconds, s.overtime_seconds,
            s.outcome, s.ended_at, s.utc_offset_seconds, s.stopped_after_seconds, t.title,
            CASE WHEN t.id IS NULL THEN 0 ELSE ",
    task_visible!("t"),
    " END
     FROM pomodoro_sessions s LEFT JOIN tasks t ON t.id = s.task_id
     WHERE s.started_at >= ?1 AND s.started_at < ?2
     ORDER BY s.started_at DESC, s.sequence DESC"
);

/// Every session ever stored, counted as the days are: the Finished
/// sessions, and the focus minutes of the Finished and Ended Early ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroTotals {
    pub finished_sessions: i64,
    pub focus_minutes: i64,
}

impl TaskStore {
    /// The all-time totals the Pomodoro page shows (batch 3, Q42). Read
    /// only; every interface may read them.
    pub fn pomodoro_totals(&self) -> Result<PomodoroTotals, TaskError> {
        let conn = self.connect()?;
        let mut statement =
            conn.prepare(&format!("SELECT {SESSION_COLUMNS} FROM pomodoro_sessions"))?;
        let mut totals = PomodoroTotals {
            finished_sessions: 0,
            focus_minutes: 0,
        };
        for session in statement.query_map([], read_session)? {
            let session = session?;
            totals.finished_sessions += i64::from(session.outcome == SessionOutcome::Finished);
            totals.focus_minutes += counted_minutes(
                session.outcome,
                whole_minutes(session.planned_seconds),
                whole_minutes(session.overtime_seconds),
                session.stopped_after_seconds.map(whole_minutes),
            );
        }
        Ok(totals)
    }

    /// Up to `limit` days of the history in `range`, newest first, after the
    /// day a `cursor` from an earlier page names. Every interface may read it.
    pub fn pomodoro_history(
        &self,
        range: &HistoryRange,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<PomodoroHistoryPage, TaskError> {
        if !(1..=MAX_DAYS_PER_PAGE).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let date = |text: &Option<String>| {
            text.as_deref()
                .map(|text| parse_date(text).ok_or(TaskError::InvalidPomodoroRange))
                .transpose()
        };
        let (from, to) = (date(&range.from)?, date(&range.to)?);
        if from.zip(to).is_some_and(|(from, to)| from > to) {
            return Err(TaskError::InvalidPomodoroRange);
        }
        let before = cursor
            .map(|raw| {
                raw.strip_prefix(CURSOR_PREFIX)
                    .and_then(parse_date)
                    .ok_or(TaskError::InvalidHistoryCursor)
            })
            .transpose()?;
        // The newest day this page may show.
        let last = match (to, before) {
            (Some(to), Some(before)) => Some(to.min(before - 1)),
            (to, before) => to.or(before.map(|day| day - 1)),
        };
        if from.zip(last).is_some_and(|(from, last)| last < from) {
            return Ok(PomodoroHistoryPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        // Every offset lies within a day of UTC, so these start times hold
        // every session of the range; the day decides the rest.
        let lower = from.map_or(i64::MIN, |day| (day - 1) * DAY_MS);
        let upper = last.map_or(i64::MAX, |day| (day + 2) * DAY_MS);

        let conn = self.connect()?;
        let mut statement = conn.prepare(HISTORY_SQL)?;
        let mut rows = statement.query(params![lower, upper])?;
        let mut days: BTreeMap<i64, Vec<HistorySession>> = BTreeMap::new();
        let mut more = false;
        while let Some(row) = rows.next()? {
            let session = read_session(row)?;
            let offset = session
                .utc_offset_seconds
                .unwrap_or_else(|| range.zone.offset_seconds(session.started_at));
            let day = day_at(session.started_at, offset);
            // No row still to come lies on a day after this.
            let latest_to_come = day + MAX_DAYS_FORWARD;
            if from.is_some_and(|from| latest_to_come < from) {
                break;
            }
            if last.is_some_and(|last| day > last) || from.is_some_and(|from| day < from) {
                continue;
            }
            let limit = limit as usize;
            if days.len() >= limit
                && days
                    .keys()
                    .rev()
                    .nth(limit - 1)
                    .is_some_and(|&oldest_shown| latest_to_come < oldest_shown)
            {
                more = true;
                break;
            }
            let shown: bool = row.get(10)?;
            let title = if shown { row.get(9)? } else { None };
            days.entry(day)
                .or_default()
                .push(history_session(session, offset, title));
            if days.len() > limit {
                days.pop_first();
                more = true;
            }
        }
        let items: Vec<PomodoroDay> = days
            .into_iter()
            .rev()
            .map(|(day, sessions)| summarize(day, sessions))
            .collect();
        let next_cursor = more
            .then(|| items.last().map(|day| cursor_after(&day.date)))
            .flatten();
        Ok(PomodoroHistoryPage { items, next_cursor })
    }
}

/// `task_title` is the title read with the session, `None` while its Task
/// is not shown.
fn history_session(
    session: PomodoroSession,
    utc_offset_seconds: i32,
    task_title: Option<String>,
) -> HistorySession {
    HistorySession {
        task_removed: session.task_id.is_some() && task_title.is_none(),
        task_title,
        planned_minutes: whole_minutes(session.planned_seconds),
        overtime_minutes: whole_minutes(session.overtime_seconds),
        stopped_after_minutes: session.stopped_after_seconds.map(whole_minutes),
        utc_offset_seconds,
        id: session.id,
        task_id: session.task_id,
        started_at: session.started_at,
        ended_at: session.ended_at,
        outcome: session.outcome,
    }
}

/// The focus a session counts, in the whole minutes it shows: a Finished
/// one its planned length and the Overtime counted, an Ended Early one its
/// focus before the break, a Stopped one nothing. The days and the all-time
/// totals both count with this.
fn counted_minutes(
    outcome: SessionOutcome,
    planned_minutes: i64,
    overtime_minutes: i64,
    stopped_after_minutes: Option<i64>,
) -> i64 {
    match outcome {
        SessionOutcome::Finished => planned_minutes + overtime_minutes,
        SessionOutcome::EndedEarly => stopped_after_minutes.unwrap_or(0),
        SessionOutcome::Stopped => 0,
    }
}

/// The day's count and focus minutes, from its sessions alone; `newest_first`
/// is the order they were read in.
fn summarize(day: i64, mut newest_first: Vec<HistorySession>) -> PomodoroDay {
    newest_first.reverse();
    let sessions = newest_first;
    let finished = || {
        sessions
            .iter()
            .filter(|entry| entry.outcome == SessionOutcome::Finished)
    };
    PomodoroDay {
        date: format_date(day),
        finished_sessions: finished().count() as i64,
        focus_minutes: sessions
            .iter()
            .map(|entry| {
                counted_minutes(
                    entry.outcome,
                    entry.planned_minutes,
                    entry.overtime_minutes,
                    entry.stopped_after_minutes,
                )
            })
            .sum(),
        stopped_sessions: sessions
            .iter()
            .filter(|entry| entry.outcome == SessionOutcome::Stopped)
            .count() as i64,
        sessions,
    }
}
