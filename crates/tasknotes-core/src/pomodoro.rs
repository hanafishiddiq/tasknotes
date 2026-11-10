//! Pomodoro Sessions (Ticket 02): the timer's rules and the stored sessions.
//!
//! The timer is a pure state machine: every step takes the [`Moment`] it
//! happens at, so the desktop shell drives it with the real clocks and the
//! tests with made-up ones. It never switches phase on its own except when a
//! planned end passes; every other switch waits for the person's click.
//! Lengths run on a clock that stops while the computer sleeps, so sleep
//! never counts as focus, break or Overtime.
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

/// The smallest and largest value one length may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Limit {
    pub min: u32,
    pub max: u32,
}

impl Limit {
    fn fits(self, value: u32) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

/// What each of the [`PomodoroDurations`] may be. The window shows these and
/// checks a draft against them; [`PomodoroDurations::check`] decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationLimits {
    pub focus_minutes: Limit,
    pub short_break_minutes: Limit,
    pub long_break_minutes: Limit,
    pub sessions_before_long_break: Limit,
}

impl DurationLimits {
    pub const CURRENT: Self = Self {
        focus_minutes: Limit {
            min: 1,
            max: MAX_FOCUS_MINUTES,
        },
        short_break_minutes: Limit {
            min: 1,
            max: MAX_BREAK_MINUTES,
        },
        long_break_minutes: Limit {
            min: 1,
            max: MAX_BREAK_MINUTES,
        },
        sessions_before_long_break: Limit {
            min: 1,
            max: MAX_SESSIONS_BEFORE_LONG_BREAK,
        },
    };
}

impl PomodoroDurations {
    pub fn check(self) -> Result<Self, TaskError> {
        let limits = DurationLimits::CURRENT;
        let fits = limits.focus_minutes.fits(self.focus_minutes)
            && limits.short_break_minutes.fits(self.short_break_minutes)
            && limits.long_break_minutes.fits(self.long_break_minutes)
            && limits
                .sessions_before_long_break
                .fits(self.sessions_before_long_break);
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
    /// Counted from the planned end up to the person's Count overtime
    /// click; zero otherwise (Start break, Stop, quitting, Stopped).
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
    /// keeps the session but gets no entry. A Task that no longer exists
    /// (the person restored a backup from before it during the session) does
    /// not cost the session: it is kept without a Task. Desktop only: the
    /// timer runs in the window's shell.
    pub fn record_pomodoro_session(
        &self,
        input: EndedSession,
    ) -> Result<RecordedSession, TaskError> {
        self.require_desktop()?;
        input.check()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let current: Option<Task> = match &input.task_id {
            Some(id) => match self.get_task_in(&tx, id) {
                Ok(task) => Some(task),
                Err(TaskError::NotFound) => None,
                Err(error) => return Err(error),
            },
            None => None,
        };
        let task_id = current.as_ref().map(|task| task.id.clone());
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
                task_id,
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

/// One reading of the two clocks the timer runs on, in milliseconds.
///
/// Every length (focus, break, Overtime) is measured on `awake`, a clock that
/// stops while the computer sleeps, so sleep never counts as any of them. The
/// stored session's start and end are `wall` times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    /// Unix milliseconds.
    pub wall: i64,
    /// Milliseconds on a clock that does not advance during sleep; only
    /// differences between readings mean anything.
    pub awake: i64,
}

/// Where the timer is. `started_at` is a wall time; `ends_at` is on the
/// awake clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PomodoroPhase {
    Idle,
    /// Counting down to the planned end.
    Focus {
        task_id: Option<String>,
        started_at: i64,
        planned_seconds: i64,
        ends_at: i64,
    },
    /// The planned end passed: Overtime runs from `ends_at` until Start break
    /// (not counted), Count overtime (counted) or Stop (not counted).
    FocusDone {
        task_id: Option<String>,
        started_at: i64,
        planned_seconds: i64,
        ends_at: i64,
    },
    Break {
        long: bool,
        ends_at: i64,
    },
    /// The break is over; waiting for the next focus to be started.
    BreakDone {
        long: bool,
    },
}

/// What the timer shows at one moment: the phase with the whole seconds left
/// or of Overtime, so the window only formats numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TimerState {
    Idle,
    Focus {
        task_id: Option<String>,
        /// Rounded up, so 00:00 never shows before the end.
        remaining_seconds: i64,
    },
    FocusDone {
        task_id: Option<String>,
        /// Since the planned end, rounded down.
        overtime_seconds: i64,
    },
    Break {
        long: bool,
        remaining_seconds: i64,
    },
    BreakDone {
        long: bool,
    },
}

/// Something the shell tells the person about, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    /// The planned focus ended.
    FocusEnded,
    /// The break ended.
    BreakEnded,
}

/// A click that does not fit where the timer is now, e.g. a notification's
/// Count overtime after the break was already started from the menu bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotNow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PomodoroTimer {
    durations: PomodoroDurations,
    phase: PomodoroPhase,
    /// Finished sessions since the last long break was earned.
    finished_in_cycle: u32,
}

const MILLIS_PER_SECOND: i64 = 1000;
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

    /// Moves past a planned end that `now` has reached. Called by the shell
    /// when the shown time changes; returns what to tell the person, once.
    pub fn tick(&mut self, now: Moment) -> Option<TimerEvent> {
        match &self.phase {
            PomodoroPhase::Focus {
                task_id,
                started_at,
                planned_seconds,
                ends_at,
            } if now.awake >= *ends_at => {
                self.phase = PomodoroPhase::FocusDone {
                    task_id: task_id.clone(),
                    started_at: *started_at,
                    planned_seconds: *planned_seconds,
                    ends_at: *ends_at,
                };
                Some(TimerEvent::FocusEnded)
            }
            PomodoroPhase::Break { long, ends_at } if now.awake >= *ends_at => {
                self.phase = PomodoroPhase::BreakDone { long: *long };
                Some(TimerEvent::BreakEnded)
            }
            _ => None,
        }
    }

    /// Starts a focus, with or without a Task: from Idle, or after a break
    /// (cutting a running break short). Refused while a focus is under way.
    pub fn start_focus(&mut self, now: Moment, task_id: Option<String>) -> Result<(), NotNow> {
        self.tick(now);
        match self.phase {
            PomodoroPhase::Idle | PomodoroPhase::Break { .. } | PomodoroPhase::BreakDone { .. } => {
                let minutes = i64::from(self.durations.focus_minutes);
                self.phase = PomodoroPhase::Focus {
                    task_id,
                    started_at: now.wall,
                    planned_seconds: minutes * 60,
                    ends_at: now.awake + minutes * MILLIS_PER_MINUTE,
                };
                Ok(())
            }
            _ => Err(NotNow),
        }
    }

    /// After the planned end: the session is Finished without Overtime (the
    /// time since the end is not counted) and the break starts.
    pub fn start_break(&mut self, now: Moment) -> Result<EndedSession, NotNow> {
        self.tick(now);
        let ended = self.finish(now, false).ok_or(NotNow)?;
        self.begin_break(now);
        Ok(ended)
    }

    /// After the planned end, when the person kept focusing: the Overtime up
    /// to `now` is counted, the session is Finished with it, and the break
    /// starts, all in one click.
    pub fn count_overtime(&mut self, now: Moment) -> Result<EndedSession, NotNow> {
        self.tick(now);
        let ended = self.finish(now, true).ok_or(NotNow)?;
        self.begin_break(now);
        Ok(ended)
    }

    /// Stops whatever runs and goes back to Idle; also what quitting does. A
    /// focus stopped before its planned end is Stopped; one that reached it
    /// is Finished without Overtime. Stopping a break ends nothing to store.
    pub fn stop(&mut self, now: Moment) -> Option<EndedSession> {
        self.tick(now);
        let ended = match &self.phase {
            PomodoroPhase::Focus {
                task_id,
                started_at,
                planned_seconds,
                ..
            } => Some(EndedSession {
                task_id: task_id.clone(),
                started_at: *started_at,
                planned_seconds: *planned_seconds,
                overtime_seconds: 0,
                outcome: SessionOutcome::Stopped,
                ended_at: now.wall.max(*started_at),
            }),
            _ => self.finish(now, false),
        };
        self.phase = PomodoroPhase::Idle;
        ended
    }

    /// The Finished session of a focus that reached its planned end, counted
    /// toward the long break; with its Overtime when `count` is set.
    fn finish(&mut self, now: Moment, count: bool) -> Option<EndedSession> {
        let PomodoroPhase::FocusDone {
            task_id,
            started_at,
            planned_seconds,
            ends_at,
        } = &self.phase
        else {
            return None;
        };
        let overtime_seconds = if count { overtime(now, *ends_at) } else { 0 };
        let ended = EndedSession {
            task_id: task_id.clone(),
            started_at: *started_at,
            planned_seconds: *planned_seconds,
            overtime_seconds,
            outcome: SessionOutcome::Finished,
            ended_at: now.wall.max(*started_at),
        };
        self.finished_in_cycle += 1;
        Some(ended)
    }

    fn begin_break(&mut self, now: Moment) {
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
            ends_at: now.awake + i64::from(minutes) * MILLIS_PER_MINUTE,
        };
    }

    /// The phase with the seconds to show at `now`.
    pub fn state(&self, now: Moment) -> TimerState {
        match &self.phase {
            PomodoroPhase::Idle => TimerState::Idle,
            PomodoroPhase::Focus {
                task_id, ends_at, ..
            } => TimerState::Focus {
                task_id: task_id.clone(),
                remaining_seconds: remaining(now, *ends_at),
            },
            PomodoroPhase::FocusDone {
                task_id, ends_at, ..
            } => TimerState::FocusDone {
                task_id: task_id.clone(),
                overtime_seconds: overtime(now, *ends_at),
            },
            PomodoroPhase::Break { long, ends_at } => TimerState::Break {
                long: *long,
                remaining_seconds: remaining(now, *ends_at),
            },
            PomodoroPhase::BreakDone { long } => TimerState::BreakDone { long: *long },
        }
    }

    /// The menu bar text: time left ("🍅 12:34"), the running Overtime after
    /// the planned end ("🍅 +07:12"), a break ("☕ 04:59", "☕ Done"), or
    /// nothing while Idle.
    pub fn title(&self, now: Moment) -> Option<String> {
        Some(match self.state(now) {
            TimerState::Idle => return None,
            TimerState::Focus {
                remaining_seconds, ..
            } => format!("🍅 {}", clock(remaining_seconds)),
            TimerState::FocusDone {
                overtime_seconds, ..
            } => format!("🍅 +{}", clock(overtime_seconds)),
            TimerState::Break {
                remaining_seconds, ..
            } => format!("☕ {}", clock(remaining_seconds)),
            TimerState::BreakDone { .. } => "☕ Done".into(),
        })
    }

    /// Milliseconds until the shown time next changes (or a planned end is
    /// due), so the shell wakes only then; `None` while nothing changes
    /// until a click (Idle, "☕ Done").
    pub fn next_change_in(&self, now: Moment) -> Option<i64> {
        match &self.phase {
            PomodoroPhase::Focus { ends_at, .. } | PomodoroPhase::Break { ends_at, .. } => {
                let left = ends_at - now.awake;
                Some(if left <= 0 {
                    0
                } else {
                    (left - 1) % MILLIS_PER_SECOND + 1
                })
            }
            PomodoroPhase::FocusDone { ends_at, .. } => {
                let over = (now.awake - ends_at).max(0);
                Some(MILLIS_PER_SECOND - over % MILLIS_PER_SECOND)
            }
            PomodoroPhase::Idle | PomodoroPhase::BreakDone { .. } => None,
        }
    }
}

/// Whole seconds left, rounded up so "00:00" never shows before the end.
fn remaining(now: Moment, ends_at: i64) -> i64 {
    ((ends_at - now.awake).max(0) + MILLIS_PER_SECOND - 1) / MILLIS_PER_SECOND
}

/// Whole seconds of Overtime since the planned end, rounded down.
fn overtime(now: Moment, ends_at: i64) -> i64 {
    (now.awake - ends_at).max(0) / MILLIS_PER_SECOND
}

/// mm:ss, with minutes past 59 kept as minutes ("90:00").
fn clock(seconds: i64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000_000;
    /// The awake clock starts anywhere; only its differences matter.
    const A0: i64 = 5_000;
    const MIN: i64 = 60_000;
    const SEC: i64 = 1_000;

    /// `offset` ms after the start on both clocks: the Mac stayed awake.
    fn at(offset: i64) -> Moment {
        Moment {
            wall: T0 + offset,
            awake: A0 + offset,
        }
    }

    /// `offset` ms of awake time after the start, after `slept` ms of sleep.
    fn after_sleep(offset: i64, slept: i64) -> Moment {
        Moment {
            wall: T0 + offset + slept,
            awake: A0 + offset,
        }
    }

    fn timer() -> PomodoroTimer {
        PomodoroTimer::new(PomodoroDurations::default())
    }

    fn focused(task: Option<&str>) -> PomodoroTimer {
        let mut t = timer();
        t.start_focus(at(0), task.map(str::to_owned)).unwrap();
        t
    }

    /// A focus that reached its planned end at 25 minutes.
    fn at_the_end(task: Option<&str>) -> PomodoroTimer {
        let mut t = focused(task);
        assert_eq!(t.tick(at(25 * MIN)), Some(TimerEvent::FocusEnded));
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
    fn the_limits_the_window_shows_are_the_ones_checked() {
        let limits = DurationLimits::CURRENT;
        assert_eq!(limits.focus_minutes, Limit { min: 1, max: 180 });
        assert_eq!(limits.short_break_minutes, Limit { min: 1, max: 60 });
        assert_eq!(limits.long_break_minutes, Limit { min: 1, max: 60 });
        assert_eq!(limits.sessions_before_long_break, Limit { min: 1, max: 12 });
        let at_the_edges = PomodoroDurations {
            focus_minutes: limits.focus_minutes.max,
            short_break_minutes: limits.short_break_minutes.min,
            long_break_minutes: limits.long_break_minutes.max,
            sessions_before_long_break: limits.sessions_before_long_break.max,
        };
        assert!(at_the_edges.check().is_ok());
        let json = serde_json::to_value(limits).unwrap();
        assert_eq!(json["focusMinutes"]["max"], 180);
        assert_eq!(
            TaskError::InvalidPomodoroDurations.to_string(),
            "Pomodoro lengths: focus 1–180 minutes, breaks 1–60 minutes, a long break after 1–12 sessions"
        );
    }

    #[test]
    fn a_focus_counts_down_and_runs_into_overtime_at_its_end() {
        let mut t = focused(None);
        assert_eq!(t.title(at(0)).as_deref(), Some("🍅 25:00"));
        assert_eq!(
            t.state(at(12 * MIN + 26 * SEC)),
            TimerState::Focus {
                task_id: None,
                remaining_seconds: 12 * 60 + 34
            }
        );
        assert_eq!(
            t.title(at(12 * MIN + 26 * SEC)).as_deref(),
            Some("🍅 12:34")
        );
        // Rounded up: the last part of a second still shows 00:01.
        assert_eq!(t.title(at(25 * MIN - 1)).as_deref(), Some("🍅 00:01"));
        assert_eq!(t.tick(at(25 * MIN - 1)), None);
        assert_eq!(t.tick(at(25 * MIN)), Some(TimerEvent::FocusEnded));
        assert_eq!(t.title(at(25 * MIN)).as_deref(), Some("🍅 +00:00"));
        assert_eq!(
            t.title(at(32 * MIN + 12 * SEC)).as_deref(),
            Some("🍅 +07:12")
        );
        assert_eq!(
            t.state(at(32 * MIN + 12 * SEC)),
            TimerState::FocusDone {
                task_id: None,
                overtime_seconds: 7 * 60 + 12
            }
        );
    }

    #[test]
    fn the_focus_end_is_announced_once_and_then_waits_for_a_click() {
        let mut t = at_the_end(None);
        for later in [1, 5, 60] {
            assert_eq!(t.tick(at(25 * MIN + later * MIN)), None);
            assert!(matches!(t.phase(), PomodoroPhase::FocusDone { .. }));
        }
    }

    #[test]
    fn start_break_does_not_count_the_overtime() {
        let mut t = at_the_end(Some("task-1"));
        // The person answers ten minutes later.
        let ended = t.start_break(at(35 * MIN)).unwrap();
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
            t.state(at(35 * MIN)),
            TimerState::Break {
                long: false,
                remaining_seconds: 5 * 60
            }
        );
    }

    #[test]
    fn count_overtime_counts_up_to_the_click_and_starts_the_break_at_once() {
        let mut t = at_the_end(Some("task-1"));
        let ended = t.count_overtime(at(32 * MIN + 12 * SEC)).unwrap();
        assert_eq!(
            ended,
            EndedSession {
                task_id: Some("task-1".into()),
                started_at: T0,
                planned_seconds: 1500,
                overtime_seconds: 7 * 60 + 12,
                outcome: SessionOutcome::Finished,
                ended_at: T0 + 32 * MIN + 12 * SEC,
            }
        );
        assert_eq!(
            t.state(at(32 * MIN + 12 * SEC)),
            TimerState::Break {
                long: false,
                remaining_seconds: 5 * 60
            }
        );
    }

    #[test]
    fn overtime_is_counted_even_when_the_tick_had_not_run_yet() {
        let mut t = focused(None);
        let ended = t.count_overtime(at(26 * MIN)).unwrap();
        assert_eq!(ended.overtime_seconds, 60);
    }

    #[test]
    fn stopping_after_the_planned_end_finishes_without_overtime() {
        let mut t = at_the_end(None);
        let ended = t.stop(at(40 * MIN)).unwrap();
        assert_eq!(ended.outcome, SessionOutcome::Finished);
        assert_eq!(ended.overtime_seconds, 0);
        assert_eq!(ended.ended_at, T0 + 40 * MIN);
        assert_eq!(t.phase(), &PomodoroPhase::Idle);
        assert_eq!(t.title(at(40 * MIN)), None);
    }

    #[test]
    fn stopping_before_the_planned_end_is_stopped() {
        let mut t = focused(Some("task-1"));
        let ended = t.stop(at(10 * MIN)).unwrap();
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
        let ended = t.stop(at(25 * MIN + SEC)).unwrap();
        assert_eq!(ended.outcome, SessionOutcome::Finished);
        assert_eq!(ended.overtime_seconds, 0);
    }

    #[test]
    fn stopping_a_break_stores_nothing() {
        let mut t = at_the_end(None);
        t.start_break(at(25 * MIN)).unwrap();
        assert_eq!(t.stop(at(26 * MIN)), None);
        assert!(!t.active());
    }

    #[test]
    fn every_switch_waits_for_a_click() {
        let mut t = at_the_end(None);
        t.start_break(at(25 * MIN)).unwrap();
        assert_eq!(t.title(at(26 * MIN)).as_deref(), Some("☕ 04:00"));
        assert_eq!(t.tick(at(30 * MIN)), Some(TimerEvent::BreakEnded));
        assert_eq!(t.tick(at(90 * MIN)), None);
        assert_eq!(t.phase(), &PomodoroPhase::BreakDone { long: false });
        assert_eq!(t.state(at(90 * MIN)), TimerState::BreakDone { long: false });
        assert_eq!(t.title(at(90 * MIN)).as_deref(), Some("☕ Done"));
        t.start_focus(at(91 * MIN), None).unwrap();
        assert!(matches!(t.phase(), PomodoroPhase::Focus { .. }));
    }

    #[test]
    fn clicks_that_do_not_fit_are_refused_and_change_nothing() {
        let mut t = focused(None);
        assert_eq!(t.start_break(at(MIN)), Err(NotNow));
        assert_eq!(t.count_overtime(at(MIN)), Err(NotNow));
        assert_eq!(t.start_focus(at(MIN), None), Err(NotNow));
        t.tick(at(25 * MIN));
        t.start_break(at(25 * MIN)).unwrap();
        // The notification's Count overtime, clicked after the menu bar's Start break.
        assert_eq!(t.count_overtime(at(26 * MIN)), Err(NotNow));
        assert!(matches!(t.phase(), PomodoroPhase::Break { .. }));
        assert_eq!(timer().stop(at(0)), None);
    }

    #[test]
    fn every_fourth_finished_session_earns_a_long_break() {
        let mut t = timer();
        let mut now = 0;
        let mut longs = Vec::new();
        for round in 0..8 {
            t.start_focus(at(now), None).unwrap();
            now += 25 * MIN;
            t.tick(at(now));
            // Counting the Overtime finishes the session just the same.
            if round % 2 == 0 {
                t.start_break(at(now)).unwrap();
            } else {
                now += MIN;
                t.count_overtime(at(now)).unwrap();
            }
            let TimerState::Break {
                long,
                remaining_seconds,
            } = t.state(at(now))
            else {
                panic!("a break")
            };
            longs.push(long);
            assert_eq!(remaining_seconds, if long { 15 * 60 } else { 5 * 60 });
            now += remaining_seconds * SEC;
            t.tick(at(now));
        }
        assert_eq!(
            longs,
            [false, false, false, true, false, false, false, true]
        );
    }

    #[test]
    fn stopped_sessions_do_not_count_toward_the_long_break() {
        let mut t = timer();
        let mut now = 0;
        for round in 0..4 {
            t.start_focus(at(now), None).unwrap();
            if round == 1 {
                t.stop(at(now + MIN));
                now += MIN;
                continue;
            }
            now += 25 * MIN;
            t.tick(at(now));
            t.start_break(at(now)).unwrap();
            assert_eq!(
                t.state(at(now)),
                TimerState::Break {
                    long: false,
                    remaining_seconds: 5 * 60
                }
            );
            t.stop(at(now));
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
        assert_eq!(t.tick(at(25 * MIN)), Some(TimerEvent::FocusEnded));
        t.start_break(at(25 * MIN)).unwrap();
        assert_eq!(t.title(at(25 * MIN)).as_deref(), Some("☕ 10:00"));
        t.start_focus(at(30 * MIN), None).unwrap();
        assert_eq!(t.title(at(30 * MIN)).as_deref(), Some("🍅 50:00"));
    }

    #[test]
    fn long_focus_keeps_counting_minutes_past_an_hour() {
        let mut t = timer();
        t.set_durations(PomodoroDurations {
            focus_minutes: 90,
            ..Default::default()
        });
        t.start_focus(at(0), None).unwrap();
        assert_eq!(t.title(at(0)).as_deref(), Some("🍅 90:00"));
    }

    #[test]
    fn sleep_never_counts_as_focus() {
        let mut t = focused(Some("task-1"));
        let two_hours = 120 * MIN;
        // Ten minutes in, the Mac sleeps for two hours.
        assert_eq!(t.tick(after_sleep(10 * MIN, two_hours)), None);
        assert_eq!(
            t.title(after_sleep(10 * MIN, two_hours)).as_deref(),
            Some("🍅 15:00")
        );
        assert_eq!(t.tick(after_sleep(25 * MIN - 1, two_hours)), None);
        assert_eq!(
            t.tick(after_sleep(25 * MIN, two_hours)),
            Some(TimerEvent::FocusEnded)
        );
        let ended = t.start_break(after_sleep(25 * MIN, two_hours)).unwrap();
        // Stored on the wall clock: it started at T0 and ended after the sleep.
        assert_eq!(ended.started_at, T0);
        assert_eq!(ended.planned_seconds, 1500);
        assert_eq!(ended.ended_at, T0 + 25 * MIN + two_hours);
    }

    #[test]
    fn sleep_never_counts_as_overtime_or_break() {
        let mut t = at_the_end(None);
        let hour = 60 * MIN;
        // Two awake minutes past the end, around an hour of sleep.
        let ended = t.count_overtime(after_sleep(27 * MIN, hour)).unwrap();
        assert_eq!(ended.overtime_seconds, 120);
        assert_eq!(
            t.state(after_sleep(28 * MIN, 2 * hour)),
            TimerState::Break {
                long: false,
                remaining_seconds: 4 * 60
            }
        );
        assert_eq!(t.tick(after_sleep(32 * MIN - 1, 3 * hour)), None);
        assert_eq!(
            t.tick(after_sleep(32 * MIN, 3 * hour)),
            Some(TimerEvent::BreakEnded)
        );
    }

    #[test]
    fn the_next_change_of_the_shown_time_is_known_so_the_shell_can_sleep_until_then() {
        let mut t = timer();
        // Idle: nothing changes until a click.
        assert_eq!(t.next_change_in(at(0)), None);
        t.start_focus(at(0), None).unwrap();
        assert_eq!(t.next_change_in(at(0)), Some(1000));
        assert_eq!(t.next_change_in(at(300)), Some(700));
        assert_eq!(t.next_change_in(at(25 * MIN)), Some(0));
        t.tick(at(25 * MIN));
        // Overtime ticks up every second from the planned end.
        assert_eq!(t.next_change_in(at(25 * MIN + 250)), Some(750));
        t.start_break(at(25 * MIN + 250)).unwrap();
        assert_eq!(t.next_change_in(at(25 * MIN + 250)), Some(1000));
        t.tick(at(30 * MIN + 250));
        // "☕ Done" waits for a click without changing.
        assert_eq!(t.next_change_in(at(31 * MIN)), None);
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
