//! Pomodoro Sessions (Ticket 02): the timer's rules and the stored sessions.
//!
//! The timer is a pure state machine: every step takes the time it happens
//! at, so the desktop shell drives it with the real clock and the tests with
//! made-up times. It never switches phase on its own except when a planned
//! end passes; every other switch waits for the person's click.
//!
//! A session is stored once it ends (finished or stopped), append-only. A
//! finished session tied to a Task adds one Progress Entry with a note and
//! never changes the Task's status or progress.

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::tasks::{ChangeTask, Task, TaskError, TaskStore, current_millis};

/// The adjustable lengths, in whole minutes, and how many finished sessions
/// earn a long break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PomodoroDurations {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sessions_before_long_break: u32,
}

impl Default for PomodoroDurations {
    /// 25 / 5 / 15, a long break after every 4 sessions.
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            sessions_before_long_break: 4,
        }
    }
}

/// The longest focus a session may plan, in minutes.
pub const MAX_FOCUS_MINUTES: u32 = 180;
/// The longest break, in minutes.
pub const MAX_BREAK_MINUTES: u32 = 60;
/// The most sessions before a long break.
pub const MAX_SESSIONS_BEFORE_LONG_BREAK: u32 = 12;

impl PomodoroDurations {
    pub fn check(self) -> Result<Self, TaskError> {
        let fits = (1..=MAX_FOCUS_MINUTES).contains(&self.focus_minutes)
            && (1..=MAX_BREAK_MINUTES).contains(&self.short_break_minutes)
            && (1..=MAX_BREAK_MINUTES).contains(&self.long_break_minutes)
            && (1..=MAX_SESSIONS_BEFORE_LONG_BREAK).contains(&self.sessions_before_long_break);
        if fits {
            Ok(self)
        } else {
            Err(TaskError::InvalidPomodoroDurations)
        }
    }
}

/// How a session ended. Only a session that reached its planned end is
/// Finished and counts toward focus time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    Finished,
    Stopped,
}

impl SessionOutcome {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Stopped => "stopped",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "finished" => Some(Self::Finished),
            "stopped" => Some(Self::Stopped),
            _ => None,
        }
    }
}

/// A session the timer has ended, ready to be stored. Times are Unix
/// milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndedSession {
    pub task_id: Option<String>,
    pub started_at: i64,
    pub planned_seconds: i64,
    /// Counted from the planned end while the person kept focusing; zero
    /// for a Stopped session and when the break started right away.
    pub overtime_seconds: i64,
    pub outcome: SessionOutcome,
    pub ended_at: i64,
}

/// A stored Pomodoro Session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroSession {
    pub id: String,
    pub task_id: Option<String>,
    pub started_at: i64,
    pub planned_seconds: i64,
    pub overtime_seconds: i64,
    pub outcome: SessionOutcome,
    pub ended_at: i64,
}

/// What recording a session did: the stored session and, when it was for a
/// Task, that Task as it is now (one revision on when a Progress Entry was
/// added).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedSession {
    pub session: PomodoroSession,
    pub task: Option<Task>,
}

/// The Progress Entry note of a finished session: "Pomodoro: 25 min focus",
/// or "Pomodoro: 25 + 7 min focus" with Overtime, in whole minutes rounded
/// to the nearest.
pub fn progress_note(planned_seconds: i64, overtime_seconds: i64) -> String {
    let planned = (planned_seconds + 30) / 60;
    let overtime = (overtime_seconds + 30) / 60;
    if overtime > 0 {
        format!("Pomodoro: {planned} + {overtime} min focus")
    } else {
        format!("Pomodoro: {planned} min focus")
    }
}

impl EndedSession {
    fn check(&self) -> Result<(), TaskError> {
        let max_planned = i64::from(MAX_FOCUS_MINUTES) * 60;
        let sound = (60..=max_planned).contains(&self.planned_seconds)
            && self.overtime_seconds >= 0
            && self.ended_at >= self.started_at
            && (self.outcome == SessionOutcome::Finished || self.overtime_seconds == 0);
        if sound {
            Ok(())
        } else {
            Err(TaskError::InvalidPomodoroSession)
        }
    }
}

/// Schema v22: the sessions, append-only like Progress Entries (ADR-0005).
/// Self-contained: it touches no table but its own.
pub(crate) fn migrate_v22(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE pomodoro_sessions (
            sequence INTEGER PRIMARY KEY AUTOINCREMENT,
            id TEXT NOT NULL UNIQUE,
            task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
            started_at INTEGER NOT NULL,
            planned_seconds INTEGER NOT NULL CHECK (planned_seconds BETWEEN 60 AND 10800),
            overtime_seconds INTEGER NOT NULL DEFAULT 0 CHECK (overtime_seconds >= 0),
            outcome TEXT NOT NULL CHECK (outcome IN ('finished', 'stopped')),
            ended_at INTEGER NOT NULL CHECK (ended_at >= started_at),
            created_at INTEGER NOT NULL,
            CHECK (outcome = 'finished' OR overtime_seconds = 0)
         );
         CREATE INDEX pomodoro_sessions_started ON pomodoro_sessions(started_at, sequence);
         CREATE INDEX pomodoro_sessions_task ON pomodoro_sessions(task_id, started_at)
           WHERE task_id IS NOT NULL;
         CREATE TRIGGER pomodoro_sessions_append_only_update BEFORE UPDATE ON pomodoro_sessions
         BEGIN SELECT RAISE(ABORT, 'pomodoro sessions are append-only'); END;
         CREATE TRIGGER pomodoro_sessions_append_only_delete BEFORE DELETE ON pomodoro_sessions
         BEGIN SELECT RAISE(ABORT, 'pomodoro sessions are append-only'); END;
         PRAGMA user_version = 22;",
    )?;
    Ok(())
}

const SESSION_COLUMNS: &str =
    "id, task_id, started_at, planned_seconds, overtime_seconds, outcome, ended_at";

/// The most sessions one listing returns; a day holds far fewer.
const MAX_LISTED: i64 = 1000;

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<PomodoroSession> {
    let outcome: String = row.get(5)?;
    Ok(PomodoroSession {
        id: row.get(0)?,
        task_id: row.get(1)?,
        started_at: row.get(2)?,
        planned_seconds: row.get(3)?,
        overtime_seconds: row.get(4)?,
        outcome: SessionOutcome::from_sql(&outcome)
            .ok_or_else(|| crate::tasks::invalid_db_text(5, outcome))?,
        ended_at: row.get(6)?,
    })
}

impl TaskStore {
    /// Stores a session the desktop timer ended. A finished one tied to a
    /// Task adds one Progress Entry ("Pomodoro: 25 + 7 min focus") in the
    /// same transaction and never changes its status; a Task in Removed
    /// keeps the session but gets no entry. Desktop only: the timer runs in
    /// the window's shell.
    pub fn record_pomodoro_session(
        &self,
        input: EndedSession,
    ) -> Result<RecordedSession, TaskError> {
        self.require_desktop()?;
        input.check()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let current: Option<Task> = match &input.task_id {
            Some(id) => Some(self.get_task_in(&tx, id)?),
            None => None,
        };
        let id = Uuid::new_v4().to_string();
        let session = tx.query_row(
            &format!(
                "INSERT INTO pomodoro_sessions
                   (id, task_id, started_at, planned_seconds, overtime_seconds, outcome,
                    ended_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 RETURNING {SESSION_COLUMNS}"
            ),
            params![
                id,
                input.task_id,
                input.started_at,
                input.planned_seconds,
                input.overtime_seconds,
                input.outcome.as_sql(),
                input.ended_at,
                current_millis()?
            ],
            read_session,
        )?;
        let task = match current {
            Some(task)
                if input.outcome == SessionOutcome::Finished
                    && crate::removed::require_writable_task(&tx, &task.id).is_ok() =>
            {
                Some(self.apply_task_change_in(
                    &tx,
                    AuditAction::TaskProgress,
                    ChangeTask {
                        id: task.id.clone(),
                        expected_revision: task.revision,
                        ..ChangeTask::default()
                    },
                    None,
                    progress_note(input.planned_seconds, input.overtime_seconds),
                )?)
            }
            other => other,
        };
        self.audit_applied(&tx, AuditAction::PomodoroRecord, &session.id, None)?;
        tx.commit()?;
        Ok(RecordedSession { session, task })
    }

    /// Sessions that started in `[from, to)` (Unix milliseconds), oldest
    /// first. Session rows hold no user text, so every interface may read
    /// them.
    pub fn list_pomodoro_sessions(
        &self,
        from: i64,
        to: i64,
    ) -> Result<Vec<PomodoroSession>, TaskError> {
        let conn = self.connect()?;
        let mut statement = conn.prepare(&format!(
            "SELECT {SESSION_COLUMNS} FROM pomodoro_sessions
             WHERE started_at >= ?1 AND started_at < ?2
             ORDER BY started_at, sequence LIMIT ?3"
        ))?;
        let sessions = statement
            .query_map(params![from, to, MAX_LISTED], read_session)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(sessions)
    }

    /// One stored session by id.
    pub fn get_pomodoro_session(&self, id: &str) -> Result<Option<PomodoroSession>, TaskError> {
        let conn = self.connect()?;
        Ok(conn
            .query_row(
                &format!("SELECT {SESSION_COLUMNS} FROM pomodoro_sessions WHERE id = ?1"),
                params![id],
                read_session,
            )
            .optional()?)
    }
}

// ---------------------------------------------------------------------------
// The timer
// ---------------------------------------------------------------------------

/// Where the timer is. Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PomodoroPhase {
    Idle,
    /// Counting down to the planned end.
    Focus {
        task_id: Option<String>,
        started_at: i64,
        ends_at: i64,
    },
    /// The planned end passed; waiting for Start break or Keep focusing.
    FocusDone {
        task_id: Option<String>,
        started_at: i64,
        ends_at: i64,
    },
    /// Keep focusing: Overtime counts from the planned end `ends_at`.
    Overtime {
        task_id: Option<String>,
        started_at: i64,
        ends_at: i64,
    },
    Break {
        long: bool,
        started_at: i64,
        ends_at: i64,
    },
    /// The break is over; waiting for the next focus to be started.
    BreakDone {
        long: bool,
    },
}

/// Something the shell tells the person about, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    /// The planned focus ended: one notification.
    FocusEnded,
    BreakEnded,
}

/// A click that does not fit where the timer is now, e.g. a notification's
/// Keep focusing after the break was already started from the menu bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotNow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PomodoroTimer {
    durations: PomodoroDurations,
    phase: PomodoroPhase,
    /// Finished sessions since the last long break was earned.
    finished_in_cycle: u32,
}

const MILLIS_PER_MINUTE: i64 = 60_000;

impl PomodoroTimer {
    pub fn new(durations: PomodoroDurations) -> Self {
        Self {
            durations,
            phase: PomodoroPhase::Idle,
            finished_in_cycle: 0,
        }
    }

    pub fn phase(&self) -> &PomodoroPhase {
        &self.phase
    }

    pub fn durations(&self) -> PomodoroDurations {
        self.durations
    }

    /// New lengths apply from the next focus or break; one under way keeps
    /// the end it was given.
    pub fn set_durations(&mut self, durations: PomodoroDurations) {
        self.durations = durations;
    }

    /// Whether a session or break is under way or waiting for a click.
    pub fn active(&self) -> bool {
        self.phase != PomodoroPhase::Idle
    }

    /// Moves past a planned end that `now` has reached. Called every second
    /// by the shell; returns what to tell the person, exactly once.
    pub fn tick(&mut self, now: i64) -> Option<TimerEvent> {
        match &self.phase {
            PomodoroPhase::Focus {
                task_id,
                started_at,
                ends_at,
            } if now >= *ends_at => {
                self.phase = PomodoroPhase::FocusDone {
                    task_id: task_id.clone(),
                    started_at: *started_at,
                    ends_at: *ends_at,
                };
                Some(TimerEvent::FocusEnded)
            }
            PomodoroPhase::Break { long, ends_at, .. } if now >= *ends_at => {
                self.phase = PomodoroPhase::BreakDone { long: *long };
                Some(TimerEvent::BreakEnded)
            }
            _ => None,
        }
    }

    /// Starts a focus, with or without a Task: from Idle, or after a break
    /// (cutting a running break short). Refused while a focus is under way.
    pub fn start_focus(&mut self, now: i64, task_id: Option<String>) -> Result<(), NotNow> {
        self.tick(now);
        match self.phase {
            PomodoroPhase::Idle | PomodoroPhase::Break { .. } | PomodoroPhase::BreakDone { .. } => {
                self.phase = PomodoroPhase::Focus {
                    task_id,
                    started_at: now,
                    ends_at: now + i64::from(self.durations.focus_minutes) * MILLIS_PER_MINUTE,
                };
                Ok(())
            }
            _ => Err(NotNow),
        }
    }

    /// Keep focusing after the planned end: Overtime counts from the
    /// planned end, not from this click.
    pub fn keep_focusing(&mut self, now: i64) -> Result<(), NotNow> {
        self.tick(now);
        match &self.phase {
            PomodoroPhase::FocusDone {
                task_id,
                started_at,
                ends_at,
            } => {
                self.phase = PomodoroPhase::Overtime {
                    task_id: task_id.clone(),
                    started_at: *started_at,
                    ends_at: *ends_at,
                };
                Ok(())
            }
            _ => Err(NotNow),
        }
    }

    /// Ends the session as Finished and starts the break. Straight from the
    /// planned end the waiting time is not Overtime; after Keep focusing,
    /// the Overtime runs to `now`.
    pub fn start_break(&mut self, now: i64) -> Result<EndedSession, NotNow> {
        self.tick(now);
        let ended = self.finish(now).ok_or(NotNow)?;
        let long = self.finished_in_cycle >= self.durations.sessions_before_long_break;
        if long {
            self.finished_in_cycle = 0;
        }
        let minutes = if long {
            self.durations.long_break_minutes
        } else {
            self.durations.short_break_minutes
        };
        self.phase = PomodoroPhase::Break {
            long,
            started_at: now,
            ends_at: now + i64::from(minutes) * MILLIS_PER_MINUTE,
        };
        Ok(ended)
    }

    /// Stops whatever runs and goes back to Idle. A focus stopped before its
    /// planned end is Stopped; one that reached it is Finished, with any
    /// Overtime. Stopping a break ends nothing worth storing.
    pub fn stop(&mut self, now: i64) -> Option<EndedSession> {
        self.tick(now);
        let ended = match &self.phase {
            PomodoroPhase::Focus {
                task_id,
                started_at,
                ends_at,
            } => Some(EndedSession {
                task_id: task_id.clone(),
                started_at: *started_at,
                planned_seconds: (ends_at - started_at) / 1000,
                overtime_seconds: 0,
                outcome: SessionOutcome::Stopped,
                ended_at: now.max(*started_at),
            }),
            _ => self.finish(now),
        };
        self.phase = PomodoroPhase::Idle;
        ended
    }

    /// The Finished session of a focus that reached its planned end, counted
    /// toward the long break.
    fn finish(&mut self, now: i64) -> Option<EndedSession> {
        let (task_id, started_at, ends_at, overtime) = match &self.phase {
            PomodoroPhase::FocusDone {
                task_id,
                started_at,
                ends_at,
            } => (task_id.clone(), *started_at, *ends_at, 0),
            PomodoroPhase::Overtime {
                task_id,
                started_at,
                ends_at,
            } => (
                task_id.clone(),
                *started_at,
                *ends_at,
                (now - ends_at).max(0) / 1000,
            ),
            _ => return None,
        };
        self.finished_in_cycle += 1;
        Some(EndedSession {
            task_id,
            started_at,
            planned_seconds: (ends_at - started_at) / 1000,
            overtime_seconds: overtime,
            outcome: SessionOutcome::Finished,
            ended_at: now.max(ends_at),
        })
    }

    /// The menu bar text: time left ("🍅 12:34"), "🍅 Done" after the
    /// planned end, Overtime ("🍅 +02:15"), a break ("☕ 04:59", "☕ Done"),
    /// or nothing while Idle.
    pub fn title(&self, now: i64) -> Option<String> {
        Some(match &self.phase {
            PomodoroPhase::Idle => return None,
            PomodoroPhase::Focus { ends_at, .. } => format!("🍅 {}", clock(left(now, *ends_at))),
            PomodoroPhase::FocusDone { .. } => "🍅 Done".into(),
            PomodoroPhase::Overtime { ends_at, .. } => {
                format!("🍅 +{}", clock((now - ends_at).max(0) / 1000))
            }
            PomodoroPhase::Break { ends_at, .. } => format!("☕ {}", clock(left(now, *ends_at))),
            PomodoroPhase::BreakDone { .. } => "☕ Done".into(),
        })
    }
}

/// Whole seconds left, rounded up so "00:00" never shows before the end.
fn left(now: i64, ends_at: i64) -> i64 {
    ((ends_at - now).max(0) + 999) / 1000
}

/// mm:ss, with minutes past 59 kept as minutes ("90:00").
fn clock(seconds: i64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000_000;
    const MIN: i64 = 60_000;
    const SEC: i64 = 1_000;

    fn timer() -> PomodoroTimer {
        PomodoroTimer::new(PomodoroDurations::default())
    }

    fn focused(task: Option<&str>) -> PomodoroTimer {
        let mut t = timer();
        t.start_focus(T0, task.map(str::to_owned)).unwrap();
        t
    }

    #[test]
    fn defaults_are_25_5_15_with_a_long_break_after_4() {
        let d = PomodoroDurations::default();
        assert_eq!(
            (
                d.focus_minutes,
                d.short_break_minutes,
                d.long_break_minutes,
                d.sessions_before_long_break
            ),
            (25, 5, 15, 4)
        );
        assert!(d.check().is_ok());
    }

    #[test]
    fn durations_outside_the_limits_are_refused() {
        for bad in [
            PomodoroDurations {
                focus_minutes: 0,
                ..Default::default()
            },
            PomodoroDurations {
                focus_minutes: 181,
                ..Default::default()
            },
            PomodoroDurations {
                short_break_minutes: 0,
                ..Default::default()
            },
            PomodoroDurations {
                long_break_minutes: 61,
                ..Default::default()
            },
            PomodoroDurations {
                sessions_before_long_break: 0,
                ..Default::default()
            },
        ] {
            assert!(bad.check().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_focus_counts_down_in_the_menu_bar_and_shows_done_at_its_end() {
        let mut t = focused(None);
        assert_eq!(t.title(T0).as_deref(), Some("🍅 25:00"));
        assert_eq!(
            t.title(T0 + 12 * MIN + 26 * SEC).as_deref(),
            Some("🍅 12:34")
        );
        // Rounded up: the last part of a second still shows 00:01.
        assert_eq!(t.title(T0 + 25 * MIN - 1).as_deref(), Some("🍅 00:01"));
        assert_eq!(t.tick(T0 + 25 * MIN - 1), None);
        assert_eq!(t.tick(T0 + 25 * MIN), Some(TimerEvent::FocusEnded));
        assert_eq!(t.title(T0 + 25 * MIN).as_deref(), Some("🍅 Done"));
    }

    #[test]
    fn the_focus_end_is_announced_once_and_then_waits_for_a_click() {
        let mut t = focused(None);
        assert_eq!(t.tick(T0 + 25 * MIN), Some(TimerEvent::FocusEnded));
        for later in [1, 5, 60] {
            assert_eq!(t.tick(T0 + 25 * MIN + later * MIN), None);
            assert!(matches!(t.phase(), PomodoroPhase::FocusDone { .. }));
        }
        assert_eq!(t.title(T0 + 90 * MIN).as_deref(), Some("🍅 Done"));
    }

    #[test]
    fn a_late_tick_after_sleep_still_ends_the_focus_once() {
        let mut t = focused(None);
        assert_eq!(t.tick(T0 + 3 * 60 * MIN), Some(TimerEvent::FocusEnded));
        assert_eq!(t.tick(T0 + 3 * 60 * MIN + SEC), None);
    }

    #[test]
    fn start_break_straight_from_the_end_counts_no_waiting_time() {
        let mut t = focused(Some("task-1"));
        t.tick(T0 + 25 * MIN);
        // The person answers ten minutes later.
        let ended = t.start_break(T0 + 35 * MIN).unwrap();
        assert_eq!(
            ended,
            EndedSession {
                task_id: Some("task-1".into()),
                started_at: T0,
                planned_seconds: 1500,
                overtime_seconds: 0,
                outcome: SessionOutcome::Finished,
                ended_at: T0 + 35 * MIN,
            }
        );
        assert_eq!(
            t.phase(),
            &PomodoroPhase::Break {
                long: false,
                started_at: T0 + 35 * MIN,
                ends_at: T0 + 40 * MIN
            }
        );
    }

    #[test]
    fn keep_focusing_counts_overtime_from_the_planned_end() {
        let mut t = focused(None);
        t.tick(T0 + 25 * MIN);
        // Clicked a minute after the end: that minute is Overtime too.
        t.keep_focusing(T0 + 26 * MIN).unwrap();
        assert_eq!(
            t.title(T0 + 27 * MIN + 15 * SEC).as_deref(),
            Some("🍅 +02:15")
        );
        let ended = t.start_break(T0 + 32 * MIN + 10 * SEC).unwrap();
        assert_eq!(ended.overtime_seconds, 7 * 60 + 10);
        assert_eq!(ended.outcome, SessionOutcome::Finished);
        assert_eq!(ended.ended_at, T0 + 32 * MIN + 10 * SEC);
    }

    #[test]
    fn stopping_during_overtime_keeps_the_session_finished_with_its_overtime() {
        let mut t = focused(None);
        t.tick(T0 + 25 * MIN);
        t.keep_focusing(T0 + 25 * MIN).unwrap();
        let ended = t.stop(T0 + 28 * MIN).unwrap();
        assert_eq!(ended.outcome, SessionOutcome::Finished);
        assert_eq!(ended.overtime_seconds, 180);
        assert_eq!(t.phase(), &PomodoroPhase::Idle);
        assert_eq!(t.title(T0 + 28 * MIN), None);
    }

    #[test]
    fn stopping_at_done_finishes_the_session_without_overtime() {
        let mut t = focused(None);
        t.tick(T0 + 25 * MIN);
        let ended = t.stop(T0 + 40 * MIN).unwrap();
        assert_eq!(ended.outcome, SessionOutcome::Finished);
        assert_eq!(ended.overtime_seconds, 0);
    }

    #[test]
    fn stopping_before_the_planned_end_is_stopped() {
        let mut t = focused(Some("task-1"));
        let ended = t.stop(T0 + 10 * MIN).unwrap();
        assert_eq!(
            ended,
            EndedSession {
                task_id: Some("task-1".into()),
                started_at: T0,
                planned_seconds: 1500,
                overtime_seconds: 0,
                outcome: SessionOutcome::Stopped,
                ended_at: T0 + 10 * MIN,
            }
        );
        assert!(!t.active());
    }

    #[test]
    fn stopping_after_a_missed_end_counts_as_finished() {
        // The shell's tick had not run yet when Stop was clicked.
        let mut t = focused(None);
        let ended = t.stop(T0 + 25 * MIN + SEC).unwrap();
        assert_eq!(ended.outcome, SessionOutcome::Finished);
    }

    #[test]
    fn stopping_a_break_stores_nothing() {
        let mut t = focused(None);
        t.tick(T0 + 25 * MIN);
        t.start_break(T0 + 25 * MIN).unwrap();
        assert_eq!(t.stop(T0 + 26 * MIN), None);
        assert!(!t.active());
    }

    #[test]
    fn every_switch_waits_for_a_click() {
        let mut t = focused(None);
        t.tick(T0 + 25 * MIN);
        t.start_break(T0 + 25 * MIN).unwrap();
        assert_eq!(t.title(T0 + 26 * MIN).as_deref(), Some("☕ 04:00"));
        assert_eq!(t.tick(T0 + 30 * MIN), Some(TimerEvent::BreakEnded));
        assert_eq!(t.tick(T0 + 90 * MIN), None);
        assert_eq!(t.phase(), &PomodoroPhase::BreakDone { long: false });
        assert_eq!(t.title(T0 + 90 * MIN).as_deref(), Some("☕ Done"));
        t.start_focus(T0 + 91 * MIN, None).unwrap();
        assert!(matches!(t.phase(), PomodoroPhase::Focus { .. }));
    }

    #[test]
    fn clicks_that_do_not_fit_are_refused_and_change_nothing() {
        let mut t = focused(None);
        assert_eq!(t.keep_focusing(T0 + MIN), Err(NotNow));
        assert_eq!(t.start_break(T0 + MIN), Err(NotNow));
        assert_eq!(t.start_focus(T0 + MIN, None), Err(NotNow));
        t.tick(T0 + 25 * MIN);
        t.start_break(T0 + 25 * MIN).unwrap();
        // The notification's Keep focusing, clicked after the menu bar's Start break.
        assert_eq!(t.keep_focusing(T0 + 26 * MIN), Err(NotNow));
        assert!(matches!(t.phase(), PomodoroPhase::Break { .. }));
        assert_eq!(timer().stop(T0), None);
    }

    #[test]
    fn every_fourth_finished_session_earns_a_long_break() {
        let mut t = timer();
        let mut now = T0;
        let mut longs = Vec::new();
        for _ in 0..8 {
            t.start_focus(now, None).unwrap();
            now += 25 * MIN;
            t.tick(now);
            t.start_break(now).unwrap();
            let PomodoroPhase::Break { long, ends_at, .. } = *t.phase() else {
                panic!("a break")
            };
            longs.push(long);
            assert_eq!(ends_at - now, if long { 15 * MIN } else { 5 * MIN });
            now = ends_at;
            t.tick(now);
        }
        assert_eq!(
            longs,
            [false, false, false, true, false, false, false, true]
        );
    }

    #[test]
    fn stopped_sessions_do_not_count_toward_the_long_break() {
        let mut t = timer();
        let mut now = T0;
        for round in 0..4 {
            t.start_focus(now, None).unwrap();
            if round == 1 {
                t.stop(now + MIN);
                now += MIN;
                continue;
            }
            now += 25 * MIN;
            t.tick(now);
            t.start_break(now).unwrap();
            assert_eq!(
                t.phase(),
                &PomodoroPhase::Break {
                    long: false,
                    started_at: now,
                    ends_at: now + 5 * MIN
                }
            );
            t.stop(now);
        }
    }

    #[test]
    fn adjusted_durations_apply_from_the_next_focus() {
        let mut t = focused(None);
        t.set_durations(PomodoroDurations {
            focus_minutes: 50,
            short_break_minutes: 10,
            ..Default::default()
        });
        assert_eq!(t.tick(T0 + 25 * MIN), Some(TimerEvent::FocusEnded));
        t.start_break(T0 + 25 * MIN).unwrap();
        assert_eq!(t.title(T0 + 25 * MIN).as_deref(), Some("☕ 10:00"));
        t.start_focus(T0 + 30 * MIN, None).unwrap();
        assert_eq!(t.title(T0 + 30 * MIN).as_deref(), Some("🍅 50:00"));
    }

    #[test]
    fn long_focus_keeps_counting_minutes_past_an_hour() {
        let mut t = timer();
        t.set_durations(PomodoroDurations {
            focus_minutes: 90,
            ..Default::default()
        });
        t.start_focus(T0, None).unwrap();
        assert_eq!(t.title(T0).as_deref(), Some("🍅 90:00"));
    }

    #[test]
    fn the_progress_note_names_planned_and_overtime_minutes() {
        assert_eq!(progress_note(1500, 0), "Pomodoro: 25 min focus");
        assert_eq!(progress_note(1500, 29), "Pomodoro: 25 min focus");
        assert_eq!(
            progress_note(1500, 7 * 60 + 10),
            "Pomodoro: 25 + 7 min focus"
        );
        assert_eq!(
            progress_note(1500, 7 * 60 + 30),
            "Pomodoro: 25 + 8 min focus"
        );
    }
}
