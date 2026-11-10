//! The Pomodoro timer in the desktop shell, so it keeps running with the
//! window closed. The rules are the core's (`tasknotes_core::pomodoro`) and
//! `pomodoro_state`'s; this module only connects them to the clocks, the menu
//! bar, the notifications, the store and the window.
//!
//! The clocks: lengths run on `Instant`, which on macOS is
//! `clock_gettime(CLOCK_UPTIME_RAW)` and does not advance while the Mac
//! sleeps (measured 2026-11-10 with Rust 1.98.1: `Instant` matched
//! `CLOCK_UPTIME_RAW` and trailed `CLOCK_MONOTONIC_RAW` by the 9774 s the Mac
//! had slept since boot). So sleep counts as no focus, break or Overtime; a
//! few seconds of Power Nap dark wake may. Stored times are wall times.
//!
//! The tick thread sleeps until the shown time next changes, and while
//! nothing counts (Idle, "☕ Done") until a click wakes it.
//!
//! The menu bar: while a session or break is under way, or a session waits
//! to be stored, Tasknotes shows a second menu-bar item of its own, a title
//! only ("🍅 12:34", "🍅 +07:12", "☕ 04:59", "☕ Done"), with the choices
//! that fit. It is there whatever "Keep Tasknotes in the menu bar" says and
//! leaves when the timer stops, so that setting keeps meaning what it meant.
//! While it is there, closing the window hides it instead of quitting.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tasknotes_core::pomodoro::{EndedSession, Moment, PomodoroDurations};
use tasknotes_core::tasks::TaskStore;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

use crate::pomodoro_state::{
    Command, MenuAction, MenuEntry, Notice, NoticeKind, Pomodoro, PomodoroView, Shared, Step,
};

const TRAY_ID: &str = "tasknotes-pomodoro";
const MENU_OPEN: &str = "tasknotes-pomodoro-open";
/// Sent to the page whenever what the timer shows changes.
const CHANGED: &str = "tasknotes://pomodoro";
const FOCUS_END_TYPE: &str = "tasknotes-pomodoro-focus-end";
const BREAK_END_TYPE: &str = "tasknotes-pomodoro-break-end";
const ACTION_START_BREAK: &str = "start-break";
const ACTION_COUNT_OVERTIME: &str = "count-overtime";
const ACTION_START_FOCUS: &str = "start-focus";
/// A click on the notification itself.
const ACTION_TAP: &str = "tap";
const FOCUS_END_TITLE: &str = "Focus done";
const BREAK_END_TITLE: &str = "Break over";
/// The system sound macOS plays for a notification.
const SYSTEM_SOUND: &str = "NSUserNotificationDefaultSoundName";

pub struct PomodoroShell {
    shared: Shared,
    /// Where the awake clock starts.
    started: Instant,
}

impl PomodoroShell {
    fn now(&self) -> Moment {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        let awake = i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX);
        Moment { wall, awake }
    }

    fn lock(&self) -> MutexGuard<'_, Pomodoro> {
        self.shared.lock()
    }
}

/// What the menu-bar item shows now; `on_screen: false` when it is not there.
struct TrayShown<R: Runtime> {
    on_screen: bool,
    title: Option<String>,
    actions: Vec<MenuAction>,
    labels: Vec<String>,
    /// The items of `actions`, so a changing label (Count +07:12) is updated
    /// in place, even while the menu is open.
    items: Vec<MenuItem<R>>,
}

impl<R: Runtime> Default for TrayShown<R> {
    fn default() -> Self {
        Self {
            on_screen: false,
            title: None,
            actions: Vec::new(),
            labels: Vec::new(),
            items: Vec::new(),
        }
    }
}

struct Tray<R: Runtime>(Mutex<TrayShown<R>>);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the page hears on `tasknotes://pomodoro`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Changed {
    view: PomodoroView,
    /// A Task that gained a Pomodoro Progress Entry.
    task_changed: Option<String>,
    /// A session was stored just now, so the history has a new entry.
    session_stored: bool,
}

/// What storing a step's ended sessions did, for the page.
#[derive(Default)]
struct Stored {
    /// A Task that gained a Pomodoro Progress Entry.
    task_changed: Option<String>,
    /// At least one session was stored.
    any: bool,
}

/// At launch, after the shell settings are read: the notifications' buttons,
/// and the tick that moves the timer past planned ends.
pub fn start<R: Runtime>(app: &AppHandle<R>, durations: PomodoroDurations, silent: bool) {
    app.manage(PomodoroShell {
        shared: Shared::new(Pomodoro::new(durations, silent)),
        started: Instant::now(),
    });
    app.manage(Tray::<R>(Mutex::default()));
    let notification = app.notification();
    let registered = notification.register_action_types(vec![
        ActionType::builder(FOCUS_END_TYPE)
            .actions(vec![
                // macOS shows them in an Options menu and reports the first for a
                // click on the menu button itself, so the choice that counts
                // nothing comes first.
                Action::builder(ACTION_START_BREAK, "Start break").build(),
                Action::builder(ACTION_COUNT_OVERTIME, "Count overtime").build(),
            ])
            .build(),
        ActionType::builder(BREAK_END_TYPE)
            .actions(vec![
                Action::builder(ACTION_START_FOCUS, "Start focus").build(),
            ])
            .build(),
    ]);
    let handle = app.clone();
    // Runs on the plugin's own thread, so the store write is off the main thread.
    let listening = notification.on_action(move |performed| {
        let command = match performed.action_id() {
            ACTION_START_BREAK => Command::StartBreak,
            ACTION_COUNT_OVERTIME => Command::CountOvertime,
            ACTION_START_FOCUS => Command::StartNextFocus,
            ACTION_TAP => return crate::shell::show_main_window(&handle),
            _ => return,
        };
        let notice = performed.notification().map(|shown| shown.id());
        run(&handle, |pomodoro, now| {
            pomodoro.answer(notice, command, now)
        });
    });
    if let Err(error) = registered.and(listening) {
        eprintln!("Tasknotes could not set up the Pomodoro notification actions: {error}");
    }
    // A notification left from before a restart has nothing to answer.
    let _ = app.run_on_main_thread(|| {
        crate::macos_notifications::withdraw_delivered(&[FOCUS_END_TITLE, BREAK_END_TITLE]);
    });

    let handle = app.clone();
    let ticking = std::thread::Builder::new()
        .name("pomodoro-tick".into())
        .spawn(move || {
            let shell = handle.state::<PomodoroShell>();
            shell.shared.run_ticks(
                || shell.now(),
                |step| {
                    let stored = perform(&handle, step);
                    publish(&handle, stored);
                },
            );
        });
    if let Err(error) = ticking {
        eprintln!("Tasknotes could not start the Pomodoro timer: {error}");
    }
}

/// One step of the rules under the lock, then what it asks for without it:
/// the notification, the store, the menu bar and the window.
fn run<R: Runtime>(
    app: &AppHandle<R>,
    step: impl FnOnce(&mut Pomodoro, Moment) -> Step,
) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    let step = step(&mut shell.lock(), shell.now());
    shell.shared.wake();
    let changed = step.changed;
    let stored = perform(app, step);
    if changed || stored.any {
        publish(app, stored)
    } else {
        shell.lock().view(shell.now())
    }
}

/// Withdraws and posts notifications (in that order, on the main thread)
/// and stores the ended sessions.
fn perform<R: Runtime>(app: &AppHandle<R>, step: Step) -> Stored {
    if step.withdraw || step.notify.is_some() {
        let handle = app.clone();
        let (withdraw, notify) = (step.withdraw, step.notify);
        let _ = app.run_on_main_thread(move || {
            if withdraw {
                crate::macos_notifications::withdraw_delivered(&[FOCUS_END_TITLE, BREAK_END_TITLE]);
            }
            if let Some(notice) = notify {
                show(&handle, notice);
            }
        });
    }
    let shell = app.state::<PomodoroShell>();
    let mut stored = Stored::default();
    for session in step.record {
        let result = record(app, session.clone());
        stored.any |= result.is_ok();
        stored.task_changed = shell.lock().saved(session, result).or(stored.task_changed);
    }
    stored
}

/// Stores one ended session in the core.
fn record<R: Runtime>(app: &AppHandle<R>, session: EndedSession) -> Result<Option<String>, String> {
    app.state::<TaskStore>()
        .record_pomodoro_session(session)
        .map(|recorded| recorded.task.map(|task| task.id))
        .map_err(|error| format!("The Pomodoro Session could not be saved: {error}"))
}

/// Brings the menu bar up to date and tells the page.
fn publish<R: Runtime>(app: &AppHandle<R>, stored: Stored) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    let view = shell.lock().view(shell.now());
    refresh_tray(app);
    let _ = app.emit(
        CHANGED,
        Changed {
            view: view.clone(),
            task_changed: stored.task_changed,
            session_stored: stored.any,
        },
    );
    view
}

fn show<R: Runtime>(app: &AppHandle<R>, notice: Notice) {
    let (kind, title, body) = match notice.kind {
        NoticeKind::FocusEnded => (
            FOCUS_END_TYPE,
            FOCUS_END_TITLE,
            "Start a break, or count the overtime if you kept focusing.",
        ),
        NoticeKind::BreakEnded => (
            BREAK_END_TYPE,
            BREAK_END_TITLE,
            "Start the next focus when you are ready.",
        ),
    };
    let mut notification = app
        .notification()
        .builder()
        .id(notice.id)
        .title(title)
        .body(body)
        .action_type_id(kind);
    if !notice.silent {
        notification = notification.sound(SYSTEM_SOUND);
    }
    if let Err(error) = notification.show() {
        eprintln!("Tasknotes could not show the Pomodoro notification: {error}");
    }
}

/// One click from the window, the menu bar or the notification. The page
/// hears every change, whoever made it.
pub fn apply<R: Runtime>(app: &AppHandle<R>, command: Command) -> PomodoroView {
    run(app, |pomodoro, now| pomodoro.apply(command, now))
}

pub fn set_durations<R: Runtime>(app: &AppHandle<R>, durations: PomodoroDurations) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    let mut pomodoro = shell.lock();
    pomodoro.set_durations(durations);
    pomodoro.view(shell.now())
}

pub fn set_silent<R: Runtime>(app: &AppHandle<R>, silent: bool) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    let mut pomodoro = shell.lock();
    pomodoro.set_silent(silent);
    pomodoro.view(shell.now())
}

/// Quitting (⌘Q, the menu-bar Quit, the Dock, a shutdown): a running focus is
/// stored as Stop would store it, with any session still waiting. Runs on the
/// main thread as the app ends; a failure can only be logged now.
pub fn save_on_quit<R: Runtime>(app: &AppHandle<R>) {
    let Some(shell) = app.try_state::<PomodoroShell>() else {
        return;
    };
    let step = shell.lock().quit(shell.now());
    shell.shared.close();
    if step.withdraw {
        crate::macos_notifications::withdraw_delivered(&[FOCUS_END_TITLE, BREAK_END_TITLE]);
    }
    for session in step.record {
        if let Err(error) = record(app, session) {
            eprintln!("{error} (while quitting)");
        }
    }
}

/// Whether the session's menu-bar item is there, so closing the window hides it.
pub fn in_menu_bar<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<Tray<R>>()
        .is_some_and(|tray| lock(&tray.0).on_screen)
}

/// Brings the menu-bar item up to date on the main thread, where every tray
/// change is made: a tray call from another thread waits for the main thread,
/// which could itself be waiting for the tray lock. It reads the timer when it
/// runs, so a late update never shows an older title.
fn refresh_tray<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || sync_tray(&handle));
}

fn sync_tray<R: Runtime>(app: &AppHandle<R>) {
    let shell = app.state::<PomodoroShell>();
    let now = shell.now();
    let (title, entries) = {
        let pomodoro = shell.lock();
        (pomodoro.title(now), pomodoro.menu(now))
    };
    let tray_state = app.state::<Tray<R>>();
    let mut shown = lock(&tray_state.0);
    let (Some(title), Some(entries)) = (title, entries) else {
        if shown.on_screen {
            app.remove_tray_by_id(TRAY_ID);
            *shown = TrayShown::default();
        }
        return;
    };
    let actions: Vec<MenuAction> = entries.iter().map(|entry| entry.action).collect();
    let labels: Vec<String> = entries.iter().map(|entry| entry.label.clone()).collect();
    let result = (|| -> tauri::Result<()> {
        match app.tray_by_id(TRAY_ID) {
            None => {
                let (menu, items) = build_menu(app, &entries)?;
                TrayIconBuilder::with_id(TRAY_ID)
                    .title(&title)
                    .tooltip("Tasknotes Pomodoro")
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(on_menu)
                    .build(app)?;
                shown.items = items;
            }
            Some(tray) => {
                if shown.title.as_deref() != Some(title.as_str()) {
                    tray.set_title(Some(&title))?;
                }
                if shown.actions != actions {
                    let (menu, items) = build_menu(app, &entries)?;
                    tray.set_menu(Some(menu))?;
                    shown.items = items;
                } else {
                    for ((item, label), before) in
                        shown.items.iter().zip(&labels).zip(&shown.labels)
                    {
                        if label != before {
                            item.set_text(label)?;
                        }
                    }
                }
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            shown.on_screen = true;
            shown.title = Some(title);
            shown.actions = actions;
            shown.labels = labels;
        }
        Err(error) => eprintln!("Tasknotes could not show the Pomodoro in the menu bar: {error}"),
    }
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    entries: &[MenuEntry],
) -> tauri::Result<(Menu<R>, Vec<MenuItem<R>>)> {
    let menu = Menu::new(app)?;
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        let item = MenuItem::with_id(app, entry.action.id(), &entry.label, true, None::<&str>)?;
        menu.append(&item)?;
        items.push(item);
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        MENU_OPEN,
        "Open Tasknotes",
        true,
        None::<&str>,
    )?)?;
    Ok((menu, items))
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id().as_ref();
    if id == MENU_OPEN {
        return crate::shell::show_main_window(app);
    }
    let Some(action) = MenuAction::from_id(id) else {
        return;
    };
    // Menu events arrive on the main thread; the store write happens elsewhere.
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        apply(&handle, action.command());
    });
}

#[tauri::command]
pub fn pomodoro_state(app: AppHandle) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    shell.lock().view(shell.now())
}

/// `async`: runs off the main thread, since an ended session is stored here.
/// A session that could not be stored is reported in the view (`saveError`)
/// and in the menu bar, and kept to try again.
#[tauri::command(async)]
pub fn pomodoro_command(app: AppHandle, command: Command) -> PomodoroView {
    apply(&app, command)
}

/// Opens Tasknotes' page in System Settings › Notifications.
#[tauri::command]
pub fn open_notification_settings(app: AppHandle) -> Result<(), String> {
    crate::macos_notifications::open_settings(&app.config().identifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page reads the history again on `sessionStored` and on nothing
    /// else the timer sends each second (`src/lib/pomodoro-api.ts`).
    #[test]
    fn the_page_hears_when_a_session_was_stored() {
        let view = Pomodoro::new(PomodoroDurations::default(), false).view(Moment {
            wall: 1_794_243_600_000,
            awake: 0,
        });
        let sent = |stored: Stored| {
            serde_json::to_value(Changed {
                view: view.clone(),
                task_changed: stored.task_changed,
                session_stored: stored.any,
            })
            .unwrap()
        };
        let stored = sent(Stored {
            task_changed: Some("task-1".into()),
            any: true,
        });
        assert_eq!(stored["sessionStored"], true);
        assert_eq!(stored["taskChanged"], "task-1");
        assert_eq!(sent(Stored::default())["sessionStored"], false);
    }
}
