//! The Pomodoro timer in the desktop shell, so it keeps running with the
//! window closed. The rules are the core's (`tasknotes_core::pomodoro`) and
//! `pomodoro_state`'s; this module only connects them to the clock (a tick
//! every second), the menu bar, the notification and the window.
//!
//! The menu bar: while a session or break is under way, Tasknotes shows a
//! second menu-bar item of its own, a title only ("🍅 12:34", "🍅 Done",
//! "🍅 +02:15", "☕ 04:59"), with the choices that fit. It is there whatever
//! the "Keep Tasknotes in the menu bar" setting says and leaves when the timer
//! stops, so that setting keeps meaning what it meant. While it is there,
//! closing the window hides it instead of quitting.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tasknotes_core::pomodoro::{EndedSession, PomodoroDurations};
use tasknotes_core::tasks::TaskStore;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

use crate::pomodoro_state::{Command, Effects, MenuKind, Pomodoro, PomodoroView};

const TRAY_ID: &str = "tasknotes-pomodoro";
const MENU_START_BREAK: &str = "tasknotes-pomodoro-start-break";
const MENU_KEEP_FOCUSING: &str = "tasknotes-pomodoro-keep-focusing";
const MENU_START_FOCUS: &str = "tasknotes-pomodoro-start-focus";
const MENU_STOP: &str = "tasknotes-pomodoro-stop";
const MENU_OPEN: &str = "tasknotes-pomodoro-open";
/// Sent to the page whenever the timer changes phase.
const CHANGED: &str = "tasknotes://pomodoro";
const ACTION_TYPE: &str = "tasknotes-pomodoro-focus-end";
const ACTION_START_BREAK: &str = "start-break";
const ACTION_KEEP_FOCUSING: &str = "keep-focusing";
/// A click on the notification itself.
const ACTION_TAP: &str = "tap";
/// The system sound macOS plays for a notification.
const SYSTEM_SOUND: &str = "NSUserNotificationDefaultSoundName";

pub struct PomodoroShell {
    timer: Mutex<Pomodoro>,
    /// What the menu-bar item shows now; `menu: None` when it is not there.
    tray: Mutex<TrayShown>,
}

#[derive(Default)]
struct TrayShown {
    menu: Option<MenuKind>,
    title: Option<String>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// What the page hears on `tasknotes://pomodoro`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Changed {
    view: PomodoroView,
    /// A Task that gained a Pomodoro Progress Entry.
    task_changed: Option<String>,
}

/// The notification and the store, as the timer's rules reach them.
struct AppEffects<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> Effects for AppEffects<'_, R> {
    fn notify_focus_end(&mut self, silent: bool) {
        let mut notification = self
            .0
            .notification()
            .builder()
            .title("Focus done")
            .body("Start a break, or keep focusing.")
            .action_type_id(ACTION_TYPE);
        if !silent {
            notification = notification.sound(SYSTEM_SOUND);
        }
        if let Err(error) = notification.show() {
            eprintln!("Tasknotes could not show the Pomodoro notification: {error}");
        }
    }

    fn record(&mut self, session: EndedSession) -> Result<Option<String>, String> {
        self.0
            .state::<TaskStore>()
            .record_pomodoro_session(session)
            .map(|recorded| recorded.task.map(|task| task.id))
            .map_err(|error| format!("The Pomodoro Session could not be saved: {error}"))
    }
}

/// At launch, after the shell settings are read: the notification's two
/// actions, and the tick that moves the timer past planned ends.
pub fn start<R: Runtime>(app: &AppHandle<R>, durations: PomodoroDurations) {
    app.manage(PomodoroShell {
        timer: Mutex::new(Pomodoro::new(durations)),
        tray: Mutex::default(),
    });
    let notification = app.notification();
    let registered = notification.register_action_types(vec![
        ActionType::builder(ACTION_TYPE)
            .actions(vec![
                // macOS shows them in an Options menu and reports the first for a
                // click on the menu button itself, so the usual answer comes first.
                Action::builder(ACTION_START_BREAK, "Start break").build(),
                Action::builder(ACTION_KEEP_FOCUSING, "Keep focusing").build(),
            ])
            .build(),
    ]);
    let handle = app.clone();
    let listening = notification.on_action(move |performed| match performed.action_id() {
        ACTION_START_BREAK => report(apply(&handle, Command::StartBreak)),
        ACTION_KEEP_FOCUSING => report(apply(&handle, Command::KeepFocusing)),
        ACTION_TAP => crate::shell::show_main_window(&handle),
        _ => {}
    });
    if let Err(error) = registered.and(listening) {
        eprintln!("Tasknotes could not set up the Pomodoro notification actions: {error}");
    }

    let handle = app.clone();
    let ticking = std::thread::Builder::new()
        .name("pomodoro-tick".into())
        .spawn(move || {
            loop {
                // Wake just after each whole second, so the title changes with the clock.
                let into_second = u64::try_from(now().rem_euclid(1000)).unwrap_or(0);
                std::thread::sleep(Duration::from_millis(1000 - into_second + 5));
                tick(&handle);
            }
        });
    if let Err(error) = ticking {
        eprintln!("Tasknotes could not start the Pomodoro timer: {error}");
    }
}

fn report(result: Result<PomodoroView, String>) {
    if let Err(error) = result {
        eprintln!("{error}");
    }
}

fn tick<R: Runtime>(app: &AppHandle<R>) {
    let shell = app.state::<PomodoroShell>();
    let now = now();
    let (changed, active, view) = {
        let mut timer = lock(&shell.timer);
        let changed = timer.tick(now, &mut AppEffects(app));
        (changed, timer.menu().is_some(), timer.view(now))
    };
    // Idle, with no menu-bar item to update, a tick costs only the lock.
    if active || changed {
        refresh_tray(app);
    }
    if changed {
        let _ = app.emit(
            CHANGED,
            Changed {
                view,
                task_changed: None,
            },
        );
    }
}

/// One click, from the window, the menu bar or the notification. The page
/// hears every change, whoever made it.
pub fn apply<R: Runtime>(app: &AppHandle<R>, command: Command) -> Result<PomodoroView, String> {
    let shell = app.state::<PomodoroShell>();
    let now = now();
    let (result, view) = {
        let mut timer = lock(&shell.timer);
        let result = timer.apply(command, now, &mut AppEffects(app));
        (result, timer.view(now))
    };
    refresh_tray(app);
    // The timer moved on even when the session could not be stored.
    let task_changed = result
        .as_ref()
        .ok()
        .and_then(|applied| applied.task_changed.clone());
    let changed = result.as_ref().map_or(true, |applied| applied.changed);
    if changed {
        let _ = app.emit(
            CHANGED,
            Changed {
                view: view.clone(),
                task_changed,
            },
        );
    }
    result.map(|_| view)
}

pub fn set_durations<R: Runtime>(app: &AppHandle<R>, durations: PomodoroDurations) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    let mut timer = lock(&shell.timer);
    timer.set_durations(durations);
    timer.view(now())
}

/// Whether the session's menu-bar item is there, so closing the window hides it.
pub fn in_menu_bar<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<PomodoroShell>()
        .is_some_and(|shell| lock(&shell.tray).menu.is_some())
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
    let (title, menu) = {
        let timer = lock(&shell.timer);
        (timer.title(now()), timer.menu())
    };
    let mut shown = lock(&shell.tray);
    let (Some(kind), Some(title)) = (menu, title) else {
        if shown.menu.is_some() {
            app.remove_tray_by_id(TRAY_ID);
            *shown = TrayShown::default();
        }
        return;
    };
    let result = match app.tray_by_id(TRAY_ID) {
        None => build_tray(app, kind, &title),
        Some(tray) => {
            let titled = if shown.title.as_deref() != Some(title.as_str()) {
                tray.set_title(Some(&title))
            } else {
                Ok(())
            };
            let menued = if shown.menu != Some(kind) {
                build_menu(app, kind).and_then(|menu| tray.set_menu(Some(menu)))
            } else {
                Ok(())
            };
            titled.and(menued)
        }
    };
    match result {
        Ok(()) => {
            shown.menu = Some(kind);
            shown.title = Some(title);
        }
        Err(error) => eprintln!("Tasknotes could not show the Pomodoro in the menu bar: {error}"),
    }
}

fn build_menu<R: Runtime>(app: &AppHandle<R>, kind: MenuKind) -> tauri::Result<Menu<R>> {
    let item = |id: &str, text: &str| MenuItem::with_id(app, id, text, true, None::<&str>);
    let menu = Menu::new(app)?;
    match kind {
        MenuKind::Focus => {}
        MenuKind::FocusDone => {
            menu.append(&item(MENU_START_BREAK, "Start break")?)?;
            menu.append(&item(MENU_KEEP_FOCUSING, "Keep focusing")?)?;
        }
        MenuKind::Overtime => menu.append(&item(MENU_START_BREAK, "Start break")?)?,
        MenuKind::Break => menu.append(&item(MENU_START_FOCUS, "Start focus")?)?,
    }
    menu.append(&item(MENU_STOP, "Stop Pomodoro")?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item(MENU_OPEN, "Open Tasknotes")?)?;
    Ok(menu)
}

fn build_tray<R: Runtime>(app: &AppHandle<R>, kind: MenuKind, title: &str) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .title(title)
        .tooltip("Tasknotes Pomodoro")
        .menu(&build_menu(app, kind)?)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu)
        .build(app)?;
    Ok(())
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let command = match event.id().as_ref() {
        MENU_START_BREAK => Command::StartBreak,
        MENU_KEEP_FOCUSING => Command::KeepFocusing,
        MENU_START_FOCUS => Command::StartNextFocus,
        MENU_STOP => Command::Stop,
        MENU_OPEN => return crate::shell::show_main_window(app),
        _ => return,
    };
    report(apply(app, command));
}

#[tauri::command]
pub fn pomodoro_state(app: AppHandle) -> PomodoroView {
    let shell = app.state::<PomodoroShell>();
    lock(&shell.timer).view(now())
}

#[tauri::command]
pub fn pomodoro_command(app: AppHandle, command: Command) -> Result<PomodoroView, String> {
    apply(&app, command)
}

/// Silent mode (the window's interaction sounds muted): the notification
/// makes no sound. Ambient rain is not involved.
#[tauri::command]
pub fn set_pomodoro_silent(app: AppHandle, silent: bool) {
    lock(&app.state::<PomodoroShell>().timer).set_silent(silent);
}

/// Opens Tasknotes' page in System Settings › Notifications.
#[tauri::command]
pub fn open_notification_settings(app: AppHandle) -> Result<(), String> {
    crate::notification_style::open_settings(&app.config().identifier)
}
