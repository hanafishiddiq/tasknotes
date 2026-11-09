//! What the desktop shell does with the Pomodoro timer between events: the
//! core's timer (`tasknotes_core::pomodoro`), plus whether the notification
//! is silent and which Task the next focus is for. `pomodoro` connects this to
//! the clock, the menu bar, the notification and the core; everything here is
//! tested without them, through `Effects`.

use serde::{Deserialize, Serialize};
use tasknotes_core::pomodoro::{
    EndedSession, PomodoroDurations, PomodoroPhase, PomodoroTimer, TimerEvent,
};

/// The outside world the timer reaches: the notification and the store.
pub trait Effects {
    /// The one notification at the planned end of a focus, with Start break
    /// and Keep focusing. Silent: no sound.
    fn notify_focus_end(&mut self, silent: bool);
    /// Stores an ended session in the core. Returns the id of a Task whose
    /// Progress Entries changed, so the window can show it.
    fn record(&mut self, session: EndedSession) -> Result<Option<String>, String>;
}

/// A click, from the window, the menu bar or the notification.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// From Idle or after a break. `task_id: None` starts without a Task.
    StartFocus {
        task_id: Option<String>,
    },
    /// The menu bar's Start focus: the Task of the last focus, if any.
    StartNextFocus,
    KeepFocusing,
    StartBreak,
    Stop,
}

/// What a click did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The phase changed; a click that did not fit changes nothing.
    pub changed: bool,
    /// A Task whose Progress Entries changed.
    pub task_changed: Option<String>,
}

/// The menu bar item's choices for each phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Focus,
    FocusDone,
    Overtime,
    Break,
}

/// What the window shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroView {
    #[serde(flatten)]
    pub phase: PomodoroPhase,
    /// The menu bar text, e.g. "🍅 12:34".
    pub title: Option<String>,
    pub durations: PomodoroDurations,
    /// The Task the menu bar's Start focus would use.
    pub last_task_id: Option<String>,
    /// The shell's clock when this was read, Unix milliseconds.
    pub now: i64,
}

pub struct Pomodoro {
    timer: PomodoroTimer,
    silent: bool,
    last_task_id: Option<String>,
}

impl Pomodoro {
    pub fn new(durations: PomodoroDurations) -> Self {
        Self {
            timer: PomodoroTimer::new(durations),
            silent: false,
            last_task_id: None,
        }
    }

    /// Silent mode: the notification makes no sound.
    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn set_durations(&mut self, durations: PomodoroDurations) {
        self.timer.set_durations(durations);
    }

    /// Called every second. Returns true when the phase changed.
    pub fn tick(&mut self, now: i64, effects: &mut impl Effects) -> bool {
        match self.timer.tick(now) {
            Some(TimerEvent::FocusEnded) => {
                effects.notify_focus_end(self.silent);
                true
            }
            Some(TimerEvent::BreakEnded) => true,
            None => false,
        }
    }

    /// A click that does not fit where the timer is now (a stale
    /// notification, a double click) is ignored. When storing an ended
    /// session fails, the timer has still moved on and the error is returned.
    pub fn apply(
        &mut self,
        command: Command,
        now: i64,
        effects: &mut impl Effects,
    ) -> Result<Applied, String> {
        // A planned end that passed since the last tick needs no notification:
        // the person is already answering (the timer moves past it first).
        let ended = match command {
            Command::StartFocus { task_id } => {
                let fits = self.timer.start_focus(now, task_id.clone()).is_ok();
                if fits {
                    self.last_task_id = task_id;
                }
                return Ok(Applied {
                    changed: fits,
                    task_changed: None,
                });
            }
            Command::StartNextFocus => {
                let fits = self
                    .timer
                    .start_focus(now, self.last_task_id.clone())
                    .is_ok();
                return Ok(Applied {
                    changed: fits,
                    task_changed: None,
                });
            }
            Command::KeepFocusing => {
                return Ok(Applied {
                    changed: self.timer.keep_focusing(now).is_ok(),
                    task_changed: None,
                });
            }
            Command::StartBreak => match self.timer.start_break(now) {
                Ok(ended) => Some(ended),
                Err(_) => return Ok(Applied::default()),
            },
            Command::Stop => {
                let was_active = self.timer.active();
                match self.timer.stop(now) {
                    Some(ended) => Some(ended),
                    None => {
                        return Ok(Applied {
                            changed: was_active,
                            task_changed: None,
                        });
                    }
                }
            }
        };
        let task_changed = match ended {
            Some(session) => effects.record(session)?,
            None => None,
        };
        Ok(Applied {
            changed: true,
            task_changed,
        })
    }

    pub fn title(&self, now: i64) -> Option<String> {
        self.timer.title(now)
    }

    /// None while Idle: the menu bar item is not there then.
    pub fn menu(&self) -> Option<MenuKind> {
        Some(match self.timer.phase() {
            PomodoroPhase::Idle => return None,
            PomodoroPhase::Focus { .. } => MenuKind::Focus,
            PomodoroPhase::FocusDone { .. } => MenuKind::FocusDone,
            PomodoroPhase::Overtime { .. } => MenuKind::Overtime,
            PomodoroPhase::Break { .. } | PomodoroPhase::BreakDone { .. } => MenuKind::Break,
        })
    }

    pub fn view(&self, now: i64) -> PomodoroView {
        PomodoroView {
            phase: self.timer.phase().clone(),
            title: self.title(now),
            durations: self.timer.durations(),
            last_task_id: self.last_task_id.clone(),
            now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tasknotes_core::pomodoro::SessionOutcome;

    const T0: i64 = 1_800_000_000_000;
    const MIN: i64 = 60_000;

    /// Stands in for the notification and the core: records what reached them.
    #[derive(Default)]
    struct Fake {
        notifications: Vec<bool>,
        recorded: Vec<EndedSession>,
        failing: bool,
    }

    impl Effects for Fake {
        fn notify_focus_end(&mut self, silent: bool) {
            self.notifications.push(silent);
        }
        fn record(&mut self, session: EndedSession) -> Result<Option<String>, String> {
            if self.failing {
                return Err("Local storage failed: synthetic".into());
            }
            let task = session
                .task_id
                .clone()
                .filter(|_| session.outcome == SessionOutcome::Finished);
            self.recorded.push(session);
            Ok(task)
        }
    }

    fn start(task: Option<&str>) -> (Pomodoro, Fake) {
        let mut pomodoro = Pomodoro::new(PomodoroDurations::default());
        let mut fake = Fake::default();
        let applied = pomodoro
            .apply(
                Command::StartFocus {
                    task_id: task.map(str::to_owned),
                },
                T0,
                &mut fake,
            )
            .unwrap();
        assert!(applied.changed);
        (pomodoro, fake)
    }

    #[test]
    fn the_focus_end_sends_exactly_one_notification() {
        let (mut pomodoro, mut fake) = start(None);
        for second in 0..(30 * 60) {
            pomodoro.tick(T0 + second * 1000, &mut fake);
        }
        assert_eq!(fake.notifications, vec![false]);
        assert_eq!(pomodoro.title(T0 + 30 * MIN).as_deref(), Some("🍅 Done"));
        assert_eq!(pomodoro.menu(), Some(MenuKind::FocusDone));
    }

    #[test]
    fn in_silent_mode_the_notification_has_no_sound() {
        let (mut pomodoro, mut fake) = start(None);
        pomodoro.set_silent(true);
        pomodoro.tick(T0 + 25 * MIN, &mut fake);
        assert_eq!(fake.notifications, vec![true]);
    }

    #[test]
    fn start_break_stores_a_finished_session_and_reports_the_task() {
        let (mut pomodoro, mut fake) = start(Some("task-1"));
        pomodoro.tick(T0 + 25 * MIN, &mut fake);
        let applied = pomodoro
            .apply(Command::StartBreak, T0 + 26 * MIN, &mut fake)
            .unwrap();
        assert_eq!(
            applied,
            Applied {
                changed: true,
                task_changed: Some("task-1".into())
            }
        );
        assert_eq!(fake.recorded.len(), 1);
        assert_eq!(fake.recorded[0].outcome, SessionOutcome::Finished);
        assert_eq!(fake.recorded[0].overtime_seconds, 0);
        assert_eq!(pomodoro.menu(), Some(MenuKind::Break));
    }

    #[test]
    fn keep_focusing_then_start_break_stores_the_overtime() {
        let (mut pomodoro, mut fake) = start(None);
        pomodoro.tick(T0 + 25 * MIN, &mut fake);
        pomodoro
            .apply(Command::KeepFocusing, T0 + 25 * MIN + 5_000, &mut fake)
            .unwrap();
        assert_eq!(pomodoro.menu(), Some(MenuKind::Overtime));
        assert_eq!(
            pomodoro.title(T0 + 27 * MIN + 15_000).as_deref(),
            Some("🍅 +02:15")
        );
        pomodoro
            .apply(Command::StartBreak, T0 + 30 * MIN, &mut fake)
            .unwrap();
        assert_eq!(fake.recorded[0].overtime_seconds, 300);
    }

    #[test]
    fn stop_before_the_end_stores_a_stopped_session_and_leaves_the_menu_bar() {
        let (mut pomodoro, mut fake) = start(Some("task-1"));
        let applied = pomodoro
            .apply(Command::Stop, T0 + 3 * MIN, &mut fake)
            .unwrap();
        assert_eq!(
            applied,
            Applied {
                changed: true,
                task_changed: None
            }
        );
        assert_eq!(fake.recorded[0].outcome, SessionOutcome::Stopped);
        assert_eq!(pomodoro.menu(), None);
        assert_eq!(pomodoro.title(T0 + 3 * MIN), None);
        assert!(fake.notifications.is_empty());
    }

    #[test]
    fn a_stale_notification_click_changes_nothing() {
        let (mut pomodoro, mut fake) = start(None);
        pomodoro.tick(T0 + 25 * MIN, &mut fake);
        pomodoro
            .apply(Command::StartBreak, T0 + 25 * MIN, &mut fake)
            .unwrap();
        let applied = pomodoro
            .apply(Command::KeepFocusing, T0 + 26 * MIN, &mut fake)
            .unwrap();
        assert_eq!(applied, Applied::default());
        let applied = pomodoro
            .apply(Command::StartBreak, T0 + 26 * MIN, &mut fake)
            .unwrap();
        assert!(!applied.changed);
        assert_eq!(fake.recorded.len(), 1);
    }

    #[test]
    fn the_menu_bar_start_focus_keeps_the_last_task() {
        let (mut pomodoro, mut fake) = start(Some("task-1"));
        pomodoro.tick(T0 + 25 * MIN, &mut fake);
        pomodoro
            .apply(Command::StartBreak, T0 + 25 * MIN, &mut fake)
            .unwrap();
        pomodoro.tick(T0 + 30 * MIN, &mut fake);
        pomodoro
            .apply(Command::StartNextFocus, T0 + 31 * MIN, &mut fake)
            .unwrap();
        let view = pomodoro.view(T0 + 31 * MIN);
        assert_eq!(view.last_task_id.as_deref(), Some("task-1"));
        assert!(matches!(
            view.phase,
            PomodoroPhase::Focus { task_id: Some(ref id), .. } if id == "task-1"
        ));
    }

    #[test]
    fn a_session_that_could_not_be_stored_is_reported_and_the_timer_moves_on() {
        let (mut pomodoro, mut fake) = start(None);
        fake.failing = true;
        let error = pomodoro
            .apply(Command::Stop, T0 + MIN, &mut fake)
            .unwrap_err();
        assert!(error.contains("synthetic"));
        assert_eq!(pomodoro.menu(), None);
    }

    #[test]
    fn the_view_names_the_phase_for_the_window() {
        let (pomodoro, _) = start(Some("task-1"));
        let json = serde_json::to_value(pomodoro.view(T0 + MIN)).unwrap();
        assert_eq!(json["phase"], "focus");
        assert_eq!(json["taskId"], "task-1");
        assert_eq!(json["endsAt"], T0 + 25 * MIN);
        assert_eq!(json["title"], "🍅 24:00");
        assert_eq!(json["durations"]["focusMinutes"], 25);
    }

    #[test]
    fn commands_arrive_from_the_window_as_tagged_json() {
        let command: Command =
            serde_json::from_str(r#"{"action":"start_focus","taskId":"task-9"}"#).unwrap();
        assert_eq!(
            command,
            Command::StartFocus {
                task_id: Some("task-9".into())
            }
        );
        let command: Command = serde_json::from_str(r#"{"action":"keep_focusing"}"#).unwrap();
        assert_eq!(command, Command::KeepFocusing);
    }
}
