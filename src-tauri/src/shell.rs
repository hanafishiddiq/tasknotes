//! The desktop shell around the main window: the optional menu-bar item, what
//! closing the window does, and the global Quick Capture shortcut.
//!
//! The rules and the state live in `shell_state`, `shell_settings` and
//! `quick_capture`; this module only connects them to the window, the tray
//! and the operating system's shortcut table. A shortcut press is queued in
//! the state, opens the main window and tells the page, which collects it and
//! focuses its ordinary New task field, so a Task made this way goes through
//! the same `create_task` command as any other.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Window, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::platform::Platform;
use crate::quick_capture::{QuickCaptureRequest, Registrar};
use crate::shell_settings::{CloseAction, SETTINGS_FILE};
use crate::shell_state::{ShellState, ShellView};

const MAIN_WINDOW: &str = "main";
const TRAY_ID: &str = "tasknotes-menu-bar";
const TRAY_OPEN: &str = "tasknotes-menu-bar-open";
const TRAY_CAPTURE: &str = "tasknotes-menu-bar-capture";
const TRAY_QUIT: &str = "tasknotes-menu-bar-quit";
/// Sent to the page when Quick Capture opens: it collects the request with
/// `take_quick_capture` and focuses the New task field.
const QUICK_CAPTURE_REQUESTED: &str = "tasknotes://quick-capture";

pub struct Shell(Mutex<ShellState>);

impl Shell {
    fn lock(&self) -> MutexGuard<'_, ShellState> {
        self.0.lock().expect("shell state")
    }
}

/// The plugin's shortcut table seen through the `Registrar` the rules expect.
struct SystemShortcuts<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> Registrar for SystemShortcuts<'_, R> {
    fn register(&mut self, accelerator: &str) -> Result<(), String> {
        self.0
            .global_shortcut()
            .register(accelerator)
            .map_err(|error| error.to_string())
    }

    fn unregister(&mut self, accelerator: &str) {
        // Nothing to do when the system already let go of it.
        let _ = self.0.global_shortcut().unregister(accelerator);
    }
}

/// The global-shortcut plugin; every shortcut it holds is Quick Capture.
pub fn shortcut_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                open_quick_capture(app);
            }
        })
        .build()
}

/// At launch: reads the saved settings, shows the tray item and registers
/// the shortcut when they are on.
pub fn start<R: Runtime>(app: &AppHandle<R>, data_dir: PathBuf) {
    let state = ShellState::launch(
        data_dir.join(SETTINGS_FILE),
        Platform::CURRENT,
        &mut SystemShortcuts(app),
    );
    let keep = state.keep_in_menu_bar();
    app.manage(Shell(Mutex::new(state)));
    sync_tray(app, &mut app.state::<Shell>().lock(), keep);
}

/// Hides the window instead of quitting when menu-bar mode is on. Returns true
/// when it handled the event.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) -> bool {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return false;
    };
    let app = window.app_handle();
    let Some(shell) = app.try_state::<Shell>() else {
        return false;
    };
    let action = shell.lock().close_action(crate::quit::exiting(app));
    match action {
        CloseAction::HideToMenuBar => {
            api.prevent_close();
            let _ = window.hide();
            true
        }
        CloseAction::Quit | CloseAction::Close => false,
    }
}

/// Brings the main window back, e.g. from the Dock or the menu-bar item.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn open_quick_capture<R: Runtime>(app: &AppHandle<R>) {
    if let Some(shell) = app.try_state::<Shell>() {
        shell.lock().request_capture();
    }
    show_main_window(app);
    let _ = app.emit(QUICK_CAPTURE_REQUESTED, ());
}

/// Builds or removes the tray item and records whether it is really there.
fn sync_tray<R: Runtime>(app: &AppHandle<R>, state: &mut ShellState, wanted: bool) {
    if wanted && !state.tray_shown() {
        let built = build_tray(app);
        if let Err(error) = &built {
            eprintln!("Tasknotes could not show its tray item: {error}");
        }
        state.record_tray(built.is_ok());
    } else if !wanted && state.tray_shown() {
        app.remove_tray_by_id(TRAY_ID);
        state.record_tray(false);
    }
}

/// Fails without an icon: an item with no icon would be invisible.
fn build_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("the app has no icon")?;
    let item = |id: &str, text: &str| MenuItem::with_id(app, id, text, true, None::<&str>);
    let build = || -> tauri::Result<()> {
        let menu = Menu::with_items(
            app,
            &[
                &item(TRAY_OPEN, "Open Tasknotes")?,
                &item(TRAY_CAPTURE, "New Task…")?,
                &PredefinedMenuItem::separator(app)?,
                &item(TRAY_QUIT, "Quit Tasknotes")?,
            ],
        )?;
        TrayIconBuilder::with_id(TRAY_ID)
            .icon(icon)
            .tooltip("Tasknotes")
            .menu(&menu)
            .show_menu_on_left_click(true)
            .on_menu_event(on_tray_menu)
            .build(app)?;
        Ok(())
    };
    build().map_err(|error| error.to_string())
}

fn on_tray_menu<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        TRAY_OPEN => show_main_window(app),
        TRAY_CAPTURE => open_quick_capture(app),
        // The same path as the Quit shortcut: the page saves Notes first.
        TRAY_QUIT => crate::quit::request(app),
        _ => {}
    }
}

#[tauri::command]
pub fn get_shell_settings(shell: State<'_, Shell>) -> ShellView {
    shell.lock().view()
}

/// When saving fails nothing changes, the tray included, and the error is returned.
#[tauri::command]
pub fn set_keep_in_menu_bar(
    app: AppHandle,
    shell: State<'_, Shell>,
    enabled: bool,
) -> Result<ShellView, String> {
    let mut state = shell.lock();
    state.set_keep_in_menu_bar(enabled)?;
    sync_tray(&app, &mut state, enabled);
    Ok(state.view())
}

/// A refused or unavailable shortcut is not an error: the view says why. When
/// saving fails the change is undone, registration included.
#[tauri::command]
pub fn set_quick_capture(
    app: AppHandle,
    shell: State<'_, Shell>,
    input: QuickCaptureRequest,
) -> Result<ShellView, String> {
    let mut state = shell.lock();
    state.set_quick_capture(input, &mut SystemShortcuts(&app))?;
    Ok(state.view())
}

/// Whether Quick Capture was asked for since the page last looked, e.g. by a
/// shortcut pressed while the page was still loading.
#[tauri::command]
pub fn take_quick_capture(shell: State<'_, Shell>) -> bool {
    shell.lock().take_pending_capture()
}

/// The launch-time shortcut problem for the main window, handed out once.
#[tauri::command]
pub fn take_launch_notice(shell: State<'_, Shell>) -> Option<String> {
    shell.lock().take_launch_notice()
}
