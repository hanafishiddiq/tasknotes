//! The Pomodoro daily history (Ticket 14): the stored sessions grouped by
//! the local calendar day they started on, newest day first, with each day's
//! count and focus minutes. The one rule the window, the CLI and MCP read:
//!
//! - Only Finished sessions count. A Stopped one is listed, never counted.
//! - A Finished session's focus is its planned length plus the Overtime the
//!   person counted with "Count +mm:ss", each rounded to the nearest whole
//!   minute as its Progress Entry says it ("25 + 7 min"); a day's focus
//!   minutes are the sum of what its sessions show.
//! - A session belongs to the day it started on, on the given wall clock,
//!   even when it runs past midnight.
//!
//! Sessions hold no user text; the only text here is a Task's title, and
//! Tasks are never hidden from AI interfaces (only Notes are, ADR 0004). A
//! Task in Removed, or in a Removed List, shows no title.

use std::collections::BTreeMap;

use rusqlite::params;
use serde::Serialize;

use crate::calendar::{DAY_MS, TimeZone, format_date, parse_date};
use crate::pomodoro::{SessionOutcome, whole_minutes};
use crate::tasks::{TaskError, TaskStore};

/// Which days to read: `from` and `to` are local dates (`YYYY-MM-DD`),
/// both included; either left out leaves that end open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRange {
    pub from: Option<String>,
    pub to: Option<String>,
    /// The wall clock that decides where a day starts: the Mac's own
    /// ([`TimeZone::Local`]) for every interface.
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
    /// `YYYY-MM-DD` on the history's wall clock.
    pub date: String,
    /// Finished sessions: the ones that count.
    pub finished_sessions: i64,
    /// Planned plus counted Overtime of the Finished sessions, in the whole
    /// minutes each session shows.
    pub focus_minutes: i64,
    /// Listed, never counted.
    pub stopped_sessions: i64,
    /// Every session that started this day, newest first.
    pub sessions: Vec<HistorySession>,
}

/// A stored session as the history shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySession {
    pub id: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub outcome: SessionOutcome,
    pub planned_seconds: i64,
    pub overtime_seconds: i64,
    /// The planned length in whole minutes, as shown ("25").
    pub planned_minutes: i64,
    /// The counted Overtime in whole minutes ("7" in "25 + 7 min"); zero
    /// for a Stopped session.
    pub overtime_minutes: i64,
    /// The Task the session was for; `None` for a session without one.
    pub task_id: Option<String>,
    /// The Task's title while it is shown; `None` without a Task or while
    /// it is in Removed (`task_removed`).
    pub task_title: Option<String>,
    /// It was for a Task that is now in Removed or in a Removed List.
    pub task_removed: bool,
}

/// The most days one page may hold.
const MAX_DAYS_PER_PAGE: u32 = 100;
const CURSOR_PREFIX: &str = "p1|";

/// Every session in a range of start times, newest first, with its Task's
/// title only while that Task is shown.
const HISTORY_SQL: &str = concat!(
    "SELECT s.id, s.task_id, s.started_at, s.planned_seconds, s.overtime_seconds,
            s.outcome, s.ended_at, t.title,
            CASE WHEN t.id IS NULL THEN 0 ELSE ",
    task_visible!("t"),
    " END
     FROM pomodoro_sessions s LEFT JOIN tasks t ON t.id = s.task_id
     WHERE s.started_at >= ?1 AND s.started_at < ?2
     ORDER BY s.started_at DESC, s.sequence DESC"
);

impl TaskStore {
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
                .map(|text| parse_date(text).ok_or(TaskError::InvalidHistoryRange))
                .transpose()
        };
        let (from, to) = (date(&range.from)?, date(&range.to)?);
        if from.zip(to).is_some_and(|(from, to)| from > to) {
            return Err(TaskError::InvalidHistoryRange);
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
        // A local day lies within a day of its UTC namesake, so these start
        // times hold every session of the range; the day decides the rest.
        let lower = from.map_or(i64::MIN, |day| (day - 1) * DAY_MS);
        let upper = last.map_or(i64::MAX, |day| (day + 2) * DAY_MS);

        let conn = self.connect()?;
        let mut statement = conn.prepare(HISTORY_SQL)?;
        let mut rows = statement.query(params![lower, upper])?;
        let mut days: BTreeMap<i64, Vec<HistorySession>> = BTreeMap::new();
        let mut more = false;
        while let Some(row) = rows.next()? {
            let started_at: i64 = row.get(2)?;
            let day = range.zone.day_of(started_at);
            // Going back in time, the local date never steps forward by more
            // than one day (a daylight-saving change is under a day), so no
            // later row is on a day after this.
            let latest_to_come = day + 1;
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
            days.entry(day)
                .or_default()
                .push(read_history_session(row)?);
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
            .then(|| {
                items
                    .last()
                    .map(|day| format!("{CURSOR_PREFIX}{}", day.date))
            })
            .flatten();
        Ok(PomodoroHistoryPage { items, next_cursor })
    }
}

fn read_history_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistorySession> {
    let outcome: String = row.get(5)?;
    let outcome = SessionOutcome::from_sql(&outcome)
        .ok_or_else(|| crate::tasks::invalid_db_text(5, outcome))?;
    let task_id: Option<String> = row.get(1)?;
    let shown: bool = row.get(8)?;
    let planned_seconds: i64 = row.get(3)?;
    let overtime_seconds: i64 = row.get(4)?;
    Ok(HistorySession {
        id: row.get(0)?,
        started_at: row.get(2)?,
        ended_at: row.get(6)?,
        outcome,
        planned_seconds,
        overtime_seconds,
        planned_minutes: whole_minutes(planned_seconds),
        overtime_minutes: whole_minutes(overtime_seconds),
        task_title: if shown { row.get(7)? } else { None },
        task_removed: task_id.is_some() && !shown,
        task_id,
    })
}

/// The day's count and focus minutes, from its sessions alone.
fn summarize(day: i64, sessions: Vec<HistorySession>) -> PomodoroDay {
    let finished = || {
        sessions
            .iter()
            .filter(|session| session.outcome == SessionOutcome::Finished)
    };
    PomodoroDay {
        date: format_date(day),
        finished_sessions: finished().count() as i64,
        focus_minutes: finished()
            .map(|session| session.planned_minutes + session.overtime_minutes)
            .sum(),
        stopped_sessions: sessions
            .iter()
            .filter(|session| session.outcome == SessionOutcome::Stopped)
            .count() as i64,
        sessions,
    }
}
