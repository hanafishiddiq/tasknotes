//! What the desktop shell does with the Pomodoro timer between events: the
//! core's timer (`tasknotes_core::pomodoro`), plus whether notifications are
//! silent, which Task the next focus is for, which notification is on screen
//! and which ended sessions still wait to be stored. `pomodoro` connects this
//! to the clocks, the menu bar, the notifications and the core.
//!
//! Every step returns a [`Step`]: what the shell must do next (notify,
//! withdraw a notification, store sessions), done with the timer's lock
//! released, so a slow database write never holds up the menu bar, the tick
//! or another click. Everything here is tested without Tauri.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tasknotes_core::pomodoro::{
    DurationLimits, EndedSession, Moment, PomodoroDurations, PomodoroTimer, TimerEvent, TimerState,
    clock,
};

/// A click, from the window, the menu bar or a notification.
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
    /// After a break: the Task of the last focus, as the shell remembers it,
    /// whether the click comes from the window, the menu bar or a notification.
    StartNextFocus,
    /// At the planned end: the break starts and the Overtime is not counted.
    StartBreak,
    /// At the planned end: the Overtime so far is counted as focus and the
    /// break starts.
    CountOvertime,
    /// Holds a running focus or break until Resume.
    Pause,
    Resume,
    /// Before the planned end: the focus ends early (its minutes count) and
    /// the break starts. Not offered by a notification, so a stale one can
    /// never end a newer focus.
    BreakEarly,
    Stop,
    /// Store again the sessions that could not be stored.
    RetrySave,
    /// Give up on them, as the person chose.
    DiscardUnsaved,
}

/// What a notification announces; each kind offers its own buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// Start break, Count overtime.
    FocusEnded,
    /// Start focus.
    BreakEnded,
}

/// One notification, numbered so a click on an older one is recognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notice {
    pub id: i32,
    pub kind: NoticeKind,
    /// No sound.
    pub silent: bool,
}

/// What the shell does after a step, with the timer's lock released.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Step {
    /// What the window and the menu bar show changed.
    pub changed: bool,
    /// Take the delivered Pomodoro notification away: it was answered, or
    /// the timer moved past it. Done before `notify`.
    pub withdraw: bool,
    pub notify: Option<Notice>,
    /// Sessions to store, in order; each result goes back through
    /// [`Pomodoro::saved`].
    pub record: Vec<EndedSession>,
}

/// One choice in the menu-bar item's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Pause,
    Resume,
    BreakEarly,
    StartBreak,
    CountOvertime,
    StartFocus,
    Stop,
    RetrySave,
    DiscardUnsaved,
}

const MENU_ACTIONS: [(MenuAction, &str); 9] = [
    (MenuAction::Pause, "tasknotes-pomodoro-pause"),
    (MenuAction::Resume, "tasknotes-pomodoro-resume"),
    (MenuAction::BreakEarly, "tasknotes-pomodoro-break-early"),
    (MenuAction::StartBreak, "tasknotes-pomodoro-start-break"),
    (
        MenuAction::CountOvertime,
        "tasknotes-pomodoro-count-overtime",
    ),
    (MenuAction::StartFocus, "tasknotes-pomodoro-start-focus"),
    (MenuAction::Stop, "tasknotes-pomodoro-stop"),
    (MenuAction::RetrySave, "tasknotes-pomodoro-retry-save"),
    (
        MenuAction::DiscardUnsaved,
        "tasknotes-pomodoro-discard-unsaved",
    ),
];

impl MenuAction {
    pub fn id(self) -> &'static str {
        MENU_ACTIONS
            .iter()
            .find(|(action, _)| *action == self)
            .map_or("", |(_, id)| id)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        MENU_ACTIONS
            .iter()
            .find(|(_, known)| *known == id)
            .map(|(action, _)| *action)
    }

    pub fn command(self) -> Command {
        match self {
            Self::Pause => Command::Pause,
            Self::Resume => Command::Resume,
            Self::BreakEarly => Command::BreakEarly,
            Self::StartBreak => Command::StartBreak,
            Self::CountOvertime => Command::CountOvertime,
            Self::StartFocus => Command::StartNextFocus,
            Self::Stop => Command::Stop,
            Self::RetrySave => Command::RetrySave,
            Self::DiscardUnsaved => Command::DiscardUnsaved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub action: MenuAction,
    pub label: String,
}

/// What the window shows: the phase with its seconds, worked out here so the
/// window only formats numbers, and the limits it checks a draft against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroView {
    #[serde(flatten)]
    pub state: TimerState,
    /// The menu bar text, e.g. "🍅 12:34".
    pub title: Option<String>,
    pub durations: PomodoroDurations,
    pub limits: DurationLimits,
    /// The Task the next focus after a break is for.
    pub last_task_id: Option<String>,
    /// Notifications make no sound.
    pub silent: bool,
    /// Why an ended session could not be stored; it is kept to try again.
    pub save_error: Option<String>,
}

pub struct Pomodoro {
    timer: PomodoroTimer,
    silent: bool,
    last_task_id: Option<String>,
    /// The notification on screen now, if any.
    notice: Option<Notice>,
    notices_sent: i32,
    /// Ended sessions the core refused to store, oldest first.
    unsaved: Vec<EndedSession>,
    save_error: Option<String>,
}

impl Pomodoro {
    pub fn new(durations: PomodoroDurations, silent: bool) -> Self {
        Self {
            timer: PomodoroTimer::new(durations),
            silent,
            last_task_id: None,
            notice: None,
            notices_sent: 0,
            unsaved: Vec::new(),
            save_error: None,
        }
    }

    /// Silent: notifications make no sound. Interaction sounds and Ambient
    /// rain have nothing to do with it.
    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn set_durations(&mut self, durations: PomodoroDurations) {
        self.timer.set_durations(durations);
    }

    /// Moves past a planned end that has passed: one notification for it.
    pub fn tick(&mut self, now: Moment) -> Step {
        let mut step = Step::default();
        let Some(event) = self.timer.tick(now) else {
            return step;
        };
        step.changed = true;
        // An older notification is out of date now.
        step.withdraw = self.notice.take().is_some();
        self.notices_sent = self.notices_sent.wrapping_add(1);
        let notice = Notice {
            id: self.notices_sent,
            kind: match event {
                TimerEvent::FocusEnded => NoticeKind::FocusEnded,
                TimerEvent::BreakEnded => NoticeKind::BreakEnded,
            },
            silent: self.silent,
        };
        self.notice = Some(notice);
        step.notify = Some(notice);
        step
    }

    /// A click from the window or the menu bar. One that does not fit where
    /// the timer is (a double click) changes nothing. When the timer moves,
    /// the notification it showed is withdrawn.
    pub fn apply(&mut self, command: Command, now: Moment) -> Step {
        // A planned end that passed since the last tick is handled as the
        // tick would have; a click that answers it right away means its
        // notification is never shown.
        let mut step = self.tick(now);
        let before = self.timer.phase().clone();
        match command {
            Command::StartFocus { task_id } => {
                if self.timer.start_focus(now, task_id.clone()).is_ok() {
                    self.last_task_id = task_id;
                }
            }
            Command::StartNextFocus => {
                let _ = self.timer.start_focus(now, self.last_task_id.clone());
            }
            Command::StartBreak => step.record.extend(self.timer.start_break(now).ok()),
            Command::CountOvertime => step.record.extend(self.timer.count_overtime(now).ok()),
            Command::Pause => {
                let _ = self.timer.pause(now);
            }
            Command::Resume => {
                let _ = self.timer.resume(now);
            }
            Command::BreakEarly => step.record.extend(self.timer.break_early(now).ok()),
            Command::Stop => step.record.extend(self.timer.stop(now)),
            Command::RetrySave => {
                step.changed |= !self.unsaved.is_empty();
                step.record.append(&mut self.unsaved);
                self.save_error = None;
            }
            Command::DiscardUnsaved => {
                step.changed |= !self.unsaved.is_empty();
                self.unsaved.clear();
                self.save_error = None;
            }
        }
        if self.timer.phase() != &before {
            step.changed = true;
            if let Some(notice) = self.notice.take() {
                if step.notify == Some(notice) {
                    step.notify = None;
                } else {
                    step.withdraw = true;
                }
            }
        }
        step
    }

    /// A click on a notification's button. Only the notification on screen
    /// now counts: one from an earlier focus or break, or one whose number
    /// cannot be read, changes nothing.
    pub fn answer(&mut self, notice_id: Option<i32>, command: Command, now: Moment) -> Step {
        match (self.notice, notice_id) {
            (Some(notice), Some(id)) if notice.id == id => self.apply(command, now),
            _ => Step::default(),
        }
    }

    /// Quitting: a focus is saved as Stop would save it (Stopped before the
    /// planned end, Finished without Overtime after it), with any session
    /// still waiting to be stored.
    pub fn quit(&mut self, now: Moment) -> Step {
        let mut record = std::mem::take(&mut self.unsaved);
        record.extend(self.timer.stop(now));
        self.save_error = None;
        Step {
            changed: true,
            withdraw: self.notice.take().is_some(),
            notify: None,
            record,
        }
    }

    /// How storing one session of a [`Step`] went. A refusal keeps the
    /// session to try again and says why in the window and the menu bar.
    /// Returns the Task whose Progress Entries changed.
    pub fn saved(
        &mut self,
        session: EndedSession,
        result: Result<Option<String>, String>,
    ) -> Option<String> {
        match result {
            Ok(task) => task,
            Err(error) => {
                self.unsaved.push(session);
                self.save_error = Some(error);
                None
            }
        }
    }

    /// Milliseconds until the shown time next changes; `None` while nothing
    /// counts, so the tick thread can sleep until a click.
    pub fn next_change_in(&self, now: Moment) -> Option<i64> {
        self.timer.next_change_in(now)
    }

    /// The menu-bar item's title, marked while a session waits to be stored.
    pub fn title(&self, now: Moment) -> Option<String> {
        let title = self.timer.title(now);
        if self.unsaved.is_empty() {
            return title;
        }
        Some(match title {
            Some(title) => format!("⚠️ {title}"),
            None => "⚠️ Not saved".into(),
        })
    }

    /// The menu-bar item's choices; `None` when there is no item, while Idle
    /// with nothing waiting to be stored.
    pub fn menu(&self, now: Moment) -> Option<Vec<MenuEntry>> {
        let entry = |action, label: &str| MenuEntry {
            action,
            label: label.to_owned(),
        };
        let mut entries = Vec::new();
        if !self.unsaved.is_empty() {
            entries.push(entry(MenuAction::RetrySave, "Try saving the session again"));
            entries.push(entry(
                MenuAction::DiscardUnsaved,
                "Discard the unsaved session",
            ));
        }
        let hold = |paused: bool| {
            if paused {
                entry(MenuAction::Resume, "Resume")
            } else {
                entry(MenuAction::Pause, "Pause")
            }
        };
        match self.timer.state(now) {
            TimerState::Idle => {}
            TimerState::Focus { paused, .. } => {
                entries.push(hold(paused));
                entries.push(entry(MenuAction::BreakEarly, "Start break"));
                entries.push(entry(MenuAction::Stop, "Stop Pomodoro"));
            }
            TimerState::FocusDone {
                overtime_seconds, ..
            } => {
                entries.push(entry(MenuAction::StartBreak, "Start break"));
                entries.push(entry(
                    MenuAction::CountOvertime,
                    &format!("Count +{}", clock(overtime_seconds)),
                ));
                entries.push(entry(MenuAction::Stop, "Stop Pomodoro"));
            }
            TimerState::Break { paused, .. } => {
                entries.push(hold(paused));
                entries.push(entry(MenuAction::StartFocus, "Start focus"));
                entries.push(entry(MenuAction::Stop, "Stop Pomodoro"));
            }
            TimerState::BreakDone { .. } => {
                entries.push(entry(MenuAction::StartFocus, "Start focus"));
                entries.push(entry(MenuAction::Stop, "Stop Pomodoro"));
            }
        }
        (!entries.is_empty()).then_some(entries)
    }

    pub fn view(&self, now: Moment) -> PomodoroView {
        PomodoroView {
            state: self.timer.state(now),
            title: self.title(now),
            durations: self.timer.durations(),
            limits: DurationLimits::CURRENT,
            last_task_id: self.last_task_id.clone(),
            silent: self.silent,
            save_error: self.save_error.clone(),
        }
    }
}

/// A little after the shown time changes, so the tick lands past it.
const SETTLE: Duration = Duration::from_millis(5);

/// The timer behind its lock, with what wakes the tick thread.
pub struct Shared {
    timer: Mutex<Pomodoro>,
    bell: Condvar,
    closed: AtomicBool,
}

impl Shared {
    pub fn new(pomodoro: Pomodoro) -> Self {
        Self {
            timer: Mutex::new(pomodoro),
            bell: Condvar::new(),
            closed: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Pomodoro> {
        self.timer.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// After a click: the tick thread works out again when it must wake.
    pub fn wake(&self) {
        self.bell.notify_all();
    }

    /// Ends [`Self::run_ticks`].
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.wake();
    }

    /// The tick thread: sleeps until the shown time next changes and, while
    /// nothing counts (Idle, "☕ Done"), until a click wakes it, so an idle
    /// timer costs no wakeups. Each tick's [`Step`] goes to `on_tick` with
    /// the lock released. Returns after [`Self::close`].
    pub fn run_ticks(&self, now: impl Fn() -> Moment, mut on_tick: impl FnMut(Step)) {
        let mut timer = self.lock();
        while !self.closed.load(Ordering::SeqCst) {
            let Some(wait) = timer.next_change_in(now()) else {
                timer = self
                    .bell
                    .wait(timer)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            let wait = Duration::from_millis(u64::try_from(wait).unwrap_or(0)) + SETTLE;
            let (guard, waited) = self
                .bell
                .wait_timeout(timer, wait)
                .unwrap_or_else(PoisonError::into_inner);
            timer = guard;
            // Woken by a click (or spuriously): that click already updated
            // the menu bar and the window; work out the wait again.
            if !waited.timed_out() || self.closed.load(Ordering::SeqCst) {
                continue;
            }
            let step = timer.tick(now());
            drop(timer);
            on_tick(step);
            timer = self.lock();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;
    use tasknotes_core::pomodoro::SessionOutcome;

    const T0: i64 = 1_800_000_000_000;
    const MIN: i64 = 60_000;
    const SEC: i64 = 1_000;

    fn at(offset: i64) -> Moment {
        Moment {
            wall: T0 + offset,
            awake: offset,
            utc_offset_seconds: 7 * 3600,
        }
    }

    fn pomodoro() -> Pomodoro {
        Pomodoro::new(PomodoroDurations::default(), false)
    }

    fn start(task: Option<&str>) -> Pomodoro {
        let mut pomodoro = pomodoro();
        let step = pomodoro.apply(
            Command::StartFocus {
                task_id: task.map(str::to_owned),
            },
            at(0),
        );
        assert!(step.changed);
        pomodoro
    }

    /// Reached the planned end; returns the notification it showed.
    fn at_the_end(pomodoro: &mut Pomodoro) -> Notice {
        let step = pomodoro.tick(at(25 * MIN));
        step.notify.expect("a notification at the planned end")
    }

    /// Stands in for the core: every session is stored, a finished one for a
    /// Task reports that Task.
    fn store(pomodoro: &mut Pomodoro, step: Step) -> Vec<Option<String>> {
        step.record
            .into_iter()
            .map(|session| {
                let task = session
                    .task_id
                    .clone()
                    .filter(|_| session.outcome == SessionOutcome::Finished);
                pomodoro.saved(session, Ok(task))
            })
            .collect()
    }

    #[test]
    fn the_focus_end_sends_exactly_one_notification() {
        let mut pomodoro = start(None);
        let mut notices = Vec::new();
        for second in 0..(30 * 60) {
            notices.extend(pomodoro.tick(at(second * SEC)).notify);
        }
        assert_eq!(
            notices,
            vec![Notice {
                id: 1,
                kind: NoticeKind::FocusEnded,
                silent: false
            }]
        );
        assert_eq!(pomodoro.title(at(30 * MIN)).as_deref(), Some("🍅 +05:00"));
    }

    #[test]
    fn silent_notifications_follow_the_pomodoro_setting_alone() {
        let mut pomodoro = Pomodoro::new(PomodoroDurations::default(), true);
        pomodoro.apply(Command::StartFocus { task_id: None }, at(0));
        assert!(at_the_end(&mut pomodoro).silent);
        pomodoro.apply(Command::StartBreak, at(25 * MIN));
        pomodoro.set_silent(false);
        let step = pomodoro.tick(at(30 * MIN));
        assert!(!step.notify.unwrap().silent);
        assert!(!pomodoro.view(at(30 * MIN)).silent);
    }

    #[test]
    fn the_break_end_sends_one_notification_offering_the_next_focus() {
        let mut pomodoro = start(Some("task-1"));
        at_the_end(&mut pomodoro);
        pomodoro.apply(Command::StartBreak, at(25 * MIN));
        let step = pomodoro.tick(at(30 * MIN));
        let notice = step.notify.unwrap();
        assert_eq!(notice.kind, NoticeKind::BreakEnded);
        assert_eq!(pomodoro.tick(at(31 * MIN)).notify, None);
        assert_eq!(pomodoro.title(at(40 * MIN)).as_deref(), Some("☕ Done"));
        // Its Start focus keeps the Task.
        pomodoro.answer(Some(notice.id), Command::StartNextFocus, at(41 * MIN));
        assert_eq!(
            pomodoro.view(at(41 * MIN)).state,
            TimerState::Focus {
                task_id: Some("task-1".into()),
                remaining_seconds: 25 * 60,
                paused: false
            }
        );
    }

    #[test]
    fn start_break_stores_a_finished_session_without_overtime() {
        let mut pomodoro = start(Some("task-1"));
        at_the_end(&mut pomodoro);
        let step = pomodoro.apply(Command::StartBreak, at(32 * MIN));
        assert!(step.changed);
        assert_eq!(step.record.len(), 1);
        assert_eq!(step.record[0].outcome, SessionOutcome::Finished);
        assert_eq!(step.record[0].overtime_seconds, 0);
        assert_eq!(store(&mut pomodoro, step), vec![Some("task-1".into())]);
        assert!(matches!(
            pomodoro.view(at(32 * MIN)).state,
            TimerState::Break { .. }
        ));
    }

    #[test]
    fn count_overtime_stores_it_and_starts_the_break_in_one_click() {
        let mut pomodoro = start(Some("task-1"));
        at_the_end(&mut pomodoro);
        let step = pomodoro.apply(Command::CountOvertime, at(32 * MIN + 12 * SEC));
        assert_eq!(step.record[0].overtime_seconds, 7 * 60 + 12);
        assert_eq!(step.record[0].outcome, SessionOutcome::Finished);
        assert_eq!(
            pomodoro.view(at(32 * MIN + 12 * SEC)).state,
            TimerState::Break {
                long: false,
                remaining_seconds: 5 * 60,
                paused: false
            }
        );
    }

    #[test]
    fn stop_before_the_end_is_stopped_and_after_it_finished_without_overtime() {
        let mut pomodoro = start(Some("task-1"));
        // Stopped three minutes in, after the Mac moved to New York: the
        // session keeps the Jakarta clock it started on.
        let in_new_york = Moment {
            utc_offset_seconds: -5 * 3600,
            ..at(3 * MIN)
        };
        let step = pomodoro.apply(Command::Stop, in_new_york);
        assert_eq!(step.record[0].outcome, SessionOutcome::Stopped);
        assert_eq!(step.record[0].stopped_after_seconds, Some(3 * 60));
        assert_eq!(step.record[0].utc_offset_seconds, 7 * 3600);
        assert_eq!(pomodoro.menu(at(3 * MIN)), None);
        assert_eq!(pomodoro.title(at(3 * MIN)), None);

        let mut pomodoro = start(None);
        at_the_end(&mut pomodoro);
        let step = pomodoro.apply(Command::Stop, at(40 * MIN));
        assert_eq!(step.record[0].outcome, SessionOutcome::Finished);
        assert_eq!(step.record[0].overtime_seconds, 0);
    }

    #[test]
    fn answering_from_the_menu_bar_or_the_window_withdraws_the_notification() {
        let mut pomodoro = start(None);
        at_the_end(&mut pomodoro);
        let step = pomodoro.apply(Command::StartBreak, at(26 * MIN));
        assert!(step.withdraw);
        // The break-end notification is withdrawn when the next focus starts.
        assert!(pomodoro.tick(at(31 * MIN)).notify.is_some());
        let step = pomodoro.apply(Command::StartNextFocus, at(32 * MIN));
        assert!(step.withdraw);
        // And Stop withdraws it too.
        let mut pomodoro = start(None);
        at_the_end(&mut pomodoro);
        assert!(pomodoro.apply(Command::Stop, at(27 * MIN)).withdraw);
        // A click that does not fit leaves the notification where it is.
        let mut pomodoro = start(None);
        at_the_end(&mut pomodoro);
        let step = pomodoro.apply(Command::StartFocus { task_id: None }, at(26 * MIN));
        assert_eq!(step, Step::default());
    }

    #[test]
    fn a_click_right_after_a_missed_end_never_shows_its_notification() {
        // The tick thread had not run yet when Count overtime was clicked.
        let mut pomodoro = start(None);
        let step = pomodoro.apply(Command::CountOvertime, at(25 * MIN + 400));
        assert_eq!(step.notify, None);
        assert!(!step.withdraw);
        assert_eq!(step.record.len(), 1);
    }

    #[test]
    fn a_click_on_an_older_or_unreadable_notification_changes_nothing() {
        let mut pomodoro = start(None);
        let first = at_the_end(&mut pomodoro);
        pomodoro.apply(Command::StartBreak, at(26 * MIN));
        // The focus-end notification's Count overtime, after the break started.
        assert_eq!(
            pomodoro.answer(Some(first.id), Command::CountOvertime, at(27 * MIN)),
            Step::default()
        );
        let second = pomodoro.tick(at(31 * MIN)).notify.unwrap();
        assert_ne!(second.id, first.id);
        assert_eq!(
            pomodoro.answer(None, Command::StartNextFocus, at(32 * MIN)),
            Step::default()
        );
        assert_eq!(
            pomodoro.answer(Some(first.id), Command::StartNextFocus, at(32 * MIN)),
            Step::default()
        );
        let step = pomodoro.answer(Some(second.id), Command::StartNextFocus, at(32 * MIN));
        assert!(step.changed);
    }

    #[test]
    fn the_next_focus_keeps_the_last_task_whoever_starts_it() {
        let mut pomodoro = start(Some("task-1"));
        at_the_end(&mut pomodoro);
        pomodoro.apply(Command::StartBreak, at(25 * MIN));
        // The window sends the same command as the menu bar, so a Task that is
        // not on the window's current page is kept.
        pomodoro.apply(Command::StartNextFocus, at(27 * MIN));
        let view = pomodoro.view(at(27 * MIN));
        assert_eq!(view.last_task_id.as_deref(), Some("task-1"));
        assert!(matches!(
            view.state,
            TimerState::Focus { task_id: Some(ref id), .. } if id == "task-1"
        ));
    }

    #[test]
    fn quitting_saves_what_runs() {
        // Before the planned end: Stopped.
        let mut pomodoro = start(Some("task-1"));
        let step = pomodoro.quit(at(10 * MIN));
        assert_eq!(step.record[0].outcome, SessionOutcome::Stopped);
        assert_eq!(step.record[0].ended_at, T0 + 10 * MIN);
        assert_eq!(step.record[0].stopped_after_seconds, Some(10 * 60));
        // After it: Finished without Overtime, and its notification goes.
        let mut pomodoro = start(Some("task-1"));
        at_the_end(&mut pomodoro);
        let step = pomodoro.quit(at(40 * MIN));
        assert_eq!(step.record[0].outcome, SessionOutcome::Finished);
        assert_eq!(step.record[0].overtime_seconds, 0);
        assert!(step.withdraw);
        // During a break there is nothing to store.
        let mut pomodoro = start(None);
        at_the_end(&mut pomodoro);
        let step = pomodoro.apply(Command::StartBreak, at(25 * MIN));
        store(&mut pomodoro, step);
        assert!(pomodoro.quit(at(26 * MIN)).record.is_empty());
    }

    #[test]
    fn a_session_that_could_not_be_stored_is_kept_shown_and_stored_again() {
        let mut pomodoro = start(Some("task-1"));
        at_the_end(&mut pomodoro);
        let mut step = pomodoro.apply(Command::StartBreak, at(26 * MIN));
        let session = step.record.remove(0);
        let task = pomodoro.saved(
            session.clone(),
            Err("The Pomodoro Session could not be saved: synthetic".into()),
        );
        assert_eq!(task, None);
        let view = pomodoro.view(at(27 * MIN));
        assert!(view.save_error.as_deref().unwrap().contains("synthetic"));
        assert_eq!(view.title.as_deref(), Some("⚠️ ☕ 04:00"));
        let menu = pomodoro.menu(at(27 * MIN)).unwrap();
        assert_eq!(menu[0].action, MenuAction::RetrySave);

        // Stopped, the menu-bar item stays to say so.
        pomodoro.apply(Command::Stop, at(28 * MIN));
        assert_eq!(
            pomodoro.title(at(28 * MIN)).as_deref(),
            Some("⚠️ Not saved")
        );
        let actions: Vec<_> = pomodoro
            .menu(at(28 * MIN))
            .unwrap()
            .iter()
            .map(|entry| entry.action)
            .collect();
        assert_eq!(actions, [MenuAction::RetrySave, MenuAction::DiscardUnsaved]);

        // Trying again hands the same session back to the store.
        let step = pomodoro.apply(Command::RetrySave, at(29 * MIN));
        assert!(step.changed);
        assert_eq!(step.record, vec![session]);
        assert_eq!(store(&mut pomodoro, step), vec![Some("task-1".into())]);
        let view = pomodoro.view(at(29 * MIN));
        assert_eq!(view.save_error, None);
        assert_eq!(view.title, None);
        assert_eq!(pomodoro.menu(at(29 * MIN)), None);
    }

    #[test]
    fn an_unsaved_session_is_dropped_only_when_the_person_discards_it() {
        let mut pomodoro = start(None);
        let mut step = pomodoro.apply(Command::Stop, at(MIN));
        pomodoro.saved(step.record.remove(0), Err("synthetic".into()));
        assert!(pomodoro.view(at(MIN)).save_error.is_some());
        let step = pomodoro.apply(Command::DiscardUnsaved, at(2 * MIN));
        assert!(step.changed && step.record.is_empty());
        assert_eq!(pomodoro.view(at(2 * MIN)).save_error, None);
        assert_eq!(pomodoro.menu(at(2 * MIN)), None);
        // Quitting tries the unsaved ones once more.
        let mut pomodoro = start(None);
        let mut step = pomodoro.apply(Command::Stop, at(MIN));
        let session = step.record.remove(0);
        pomodoro.saved(session.clone(), Err("synthetic".into()));
        assert_eq!(pomodoro.quit(at(2 * MIN)).record, vec![session]);
    }

    #[test]
    fn the_menu_offers_what_the_timer_waits_for_with_the_live_overtime() {
        let mut pomodoro = start(None);
        let labels = |pomodoro: &Pomodoro, offset| {
            pomodoro
                .menu(at(offset))
                .unwrap_or_default()
                .into_iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&pomodoro, MIN),
            ["Pause", "Start break", "Stop Pomodoro"]
        );
        at_the_end(&mut pomodoro);
        assert_eq!(
            labels(&pomodoro, 32 * MIN + 12 * SEC),
            ["Start break", "Count +07:12", "Stop Pomodoro"]
        );
        pomodoro.apply(Command::StartBreak, at(33 * MIN));
        assert_eq!(
            labels(&pomodoro, 34 * MIN),
            ["Pause", "Start focus", "Stop Pomodoro"]
        );
        pomodoro.apply(Command::Pause, at(34 * MIN));
        assert_eq!(
            labels(&pomodoro, 34 * MIN),
            ["Resume", "Start focus", "Stop Pomodoro"]
        );
        pomodoro.apply(Command::Stop, at(35 * MIN));
        assert!(labels(&pomodoro, 35 * MIN).is_empty());
        for (action, _) in MENU_ACTIONS {
            assert_eq!(MenuAction::from_id(action.id()), Some(action));
        }
        assert_eq!(MenuAction::StartFocus.command(), Command::StartNextFocus);
    }

    #[test]
    fn pause_holds_the_focus_in_the_window_and_the_menu_bar_until_resume() {
        let mut pomodoro = start(Some("task-1"));
        let step = pomodoro.apply(Command::Pause, at(10 * MIN));
        assert!(step.changed);
        assert!(step.record.is_empty());
        let json = serde_json::to_value(pomodoro.view(at(30 * MIN))).unwrap();
        assert_eq!(json["phase"], "focus");
        assert_eq!(json["paused"], true);
        assert_eq!(json["remainingSeconds"], 15 * 60);
        assert_eq!(json["title"], "⏸ 15:00");
        assert_eq!(pomodoro.next_change_in(at(30 * MIN)), None);
        let actions: Vec<_> = pomodoro
            .menu(at(30 * MIN))
            .unwrap()
            .into_iter()
            .map(|entry| entry.action)
            .collect();
        assert_eq!(
            actions,
            [MenuAction::Resume, MenuAction::BreakEarly, MenuAction::Stop]
        );
        assert_eq!(MenuAction::Resume.command(), Command::Resume);
        // A second Pause (a double click) changes nothing.
        assert!(!pomodoro.apply(Command::Pause, at(31 * MIN)).changed);
        pomodoro.apply(Command::Resume, at(40 * MIN));
        assert_eq!(pomodoro.title(at(40 * MIN)).as_deref(), Some("🍅 15:00"));
    }

    #[test]
    fn a_break_started_early_stores_the_focus_as_ended_early_and_starts_the_break() {
        let mut pomodoro = start(Some("task-1"));
        let mut step = pomodoro.apply(Command::BreakEarly, at(10 * MIN));
        assert!(step.changed);
        let ended = step.record.remove(0);
        assert_eq!(ended.outcome, SessionOutcome::EndedEarly);
        assert_eq!(ended.stopped_after_seconds, Some(10 * 60));
        assert_eq!(pomodoro.title(at(11 * MIN)).as_deref(), Some("☕ 04:00"));
        assert_eq!(MenuAction::BreakEarly.command(), Command::BreakEarly);
        // Past the planned end it is not the early break: it changes nothing.
        let mut pomodoro = start(None);
        at_the_end(&mut pomodoro);
        assert!(
            pomodoro
                .apply(Command::BreakEarly, at(26 * MIN))
                .record
                .is_empty()
        );
    }

    #[test]
    fn the_window_names_every_command_as_the_page_sends_it() {
        for (json, command) in [
            (r#"{"action":"pause"}"#, Command::Pause),
            (r#"{"action":"resume"}"#, Command::Resume),
            (r#"{"action":"break_early"}"#, Command::BreakEarly),
        ] {
            assert_eq!(serde_json::from_str::<Command>(json).unwrap(), command);
        }
    }

    #[test]
    fn the_view_gives_the_window_seconds_limits_and_settings() {
        let mut pomodoro = start(Some("task-1"));
        let json = serde_json::to_value(pomodoro.view(at(MIN))).unwrap();
        assert_eq!(json["phase"], "focus");
        assert_eq!(json["taskId"], "task-1");
        assert_eq!(json["remainingSeconds"], 24 * 60);
        assert_eq!(json["title"], "🍅 24:00");
        assert_eq!(json["durations"]["focusMinutes"], 25);
        assert_eq!(json["limits"]["focusMinutes"]["max"], 180);
        assert_eq!(json["limits"]["sessionsBeforeLongBreak"]["max"], 12);
        assert_eq!(json["silent"], false);
        assert_eq!(json["saveError"], serde_json::Value::Null);
        at_the_end(&mut pomodoro);
        let json = serde_json::to_value(pomodoro.view(at(27 * MIN + 15 * SEC))).unwrap();
        assert_eq!(json["phase"], "focus_done");
        assert_eq!(json["overtimeSeconds"], 2 * 60 + 15);
        assert_eq!(json["title"], "🍅 +02:15");
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
        for (json, expected) in [
            (r#"{"action":"count_overtime"}"#, Command::CountOvertime),
            (r#"{"action":"start_next_focus"}"#, Command::StartNextFocus),
            (r#"{"action":"retry_save"}"#, Command::RetrySave),
            (r#"{"action":"discard_unsaved"}"#, Command::DiscardUnsaved),
        ] {
            assert_eq!(serde_json::from_str::<Command>(json).unwrap(), expected);
        }
        assert!(serde_json::from_str::<Command>(r#"{"action":"keep_focusing"}"#).is_err());
    }

    /// Runs the real tick loop on its own thread against the real clock.
    struct Ticker {
        shared: Arc<Shared>,
        ticks: Arc<AtomicUsize>,
        started: Instant,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Ticker {
        fn start() -> Self {
            let shared = Arc::new(Shared::new(pomodoro()));
            let ticks = Arc::new(AtomicUsize::new(0));
            let started = Instant::now();
            let thread = {
                let (shared, ticks) = (Arc::clone(&shared), Arc::clone(&ticks));
                std::thread::spawn(move || {
                    shared.run_ticks(
                        move || Moment {
                            wall: T0,
                            awake: i64::try_from(started.elapsed().as_millis()).unwrap(),
                            utc_offset_seconds: 0,
                        },
                        |_| {
                            ticks.fetch_add(1, Ordering::SeqCst);
                        },
                    );
                })
            };
            Self {
                shared,
                ticks,
                started,
                thread: Some(thread),
            }
        }

        fn now(&self) -> Moment {
            Moment {
                wall: T0,
                awake: i64::try_from(self.started.elapsed().as_millis()).unwrap(),
                utc_offset_seconds: 0,
            }
        }

        fn click(&self, command: Command) {
            let now = self.now();
            self.shared.lock().apply(command, now);
            self.shared.wake();
        }

        fn ticks_during(&self, wait: Duration) -> usize {
            let before = self.ticks.load(Ordering::SeqCst);
            std::thread::sleep(wait);
            self.ticks.load(Ordering::SeqCst) - before
        }
    }

    impl Drop for Ticker {
        fn drop(&mut self) {
            self.shared.close();
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    #[test]
    fn the_tick_thread_sleeps_while_nothing_counts_and_ticks_each_second_while_it_does() {
        let ticker = Ticker::start();
        // Idle: no wakeups at all (before this change, one every second).
        assert_eq!(ticker.ticks_during(Duration::from_millis(1300)), 0);
        ticker.click(Command::StartFocus { task_id: None });
        let ticks = ticker.ticks_during(Duration::from_millis(2100));
        assert!((1..=3).contains(&ticks), "{ticks} ticks in 2.1 s");
        ticker.click(Command::Stop);
        assert_eq!(ticker.ticks_during(Duration::from_millis(1300)), 0);
    }
}
