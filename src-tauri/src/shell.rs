//! The desktop shell around the main window: the optional menu-bar item, what
//! closing the window does, and the global Quick Capture shortcut.
//!
//! The rules live in `shell_settings` and `quick_capture`; this module only
//! connects them to the window, the tray and the operating system's shortcut
//! table. A shortcut press opens the main window and asks the page to focus its
//! ordinary New task field, so a Task made this way goes through the same
//! `create_task` command as any other.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Window, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::quick_capture::{QuickCapture, QuickCaptureRequest, QuickCaptureStatus, Registrar};
use crate::shell_settings::{
    CloseAction, Loaded, SETTINGS_FILE, ShellSettings, close_action, load_settings, save_settings,
};

const MAIN_WINDOW: &str = "main";
const TRAY_ID: &str = "tasknotes-menu-bar";
const TRAY_OPEN: &str = "tasknotes-menu-bar-open";
const TRAY_CAPTURE: &str = "tasknotes-menu-bar-capture";
const TRAY_QUIT: &str = "tasknotes-menu-bar-quit";
/// Sent to the page when Quick Capture opens: it focuses the New task field.
const QUICK_CAPTURE_REQUESTED: &str = "tasknotes://quick-capture";

pub struct Shell {
    path: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    keep_in_menu_bar: bool,
    capture: QuickCapture,
    unreadable: bool,
}

impl Inner {
    fn settings(&self) -> ShellSettings {
        ShellSettings {
            keep_in_menu_bar: self.keep_in_menu_bar,
            quick_capture: self.capture.setting().clone(),
        }
    }
}

/// What Settings shows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellView {
    keep_in_menu_bar: bool,
    /// The menu-bar item could not be created on this system.
    menu_bar_unavailable: bool,
    quick_capture: QuickCaptureStatus,
    /// The recorded shortcut, also while Quick Capture is off.
    remembered_label: Option<String>,
    /// The settings file could not be read at launch; the defaults are in use.
    settings_unreadable: bool,
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

/// At launch: reads the saved settings, shows the menu-bar item and registers
/// the shortcut when they are on.
pub fn start<R: Runtime>(app: &AppHandle<R>, data_dir: PathBuf) {
    let path = data_dir.join(SETTINGS_FILE);
    let Loaded {
        settings,
        unreadable,
    } = load_settings(&path);
    let mut capture = QuickCapture::new(settings.quick_capture);
    capture.start(&mut SystemShortcuts(app));
    app.manage(Shell {
        path,
        inner: Mutex::new(Inner {
            keep_in_menu_bar: settings.keep_in_menu_bar,
            capture,
            unreadable,
        }),
    });
    sync_tray(app, settings.keep_in_menu_bar);
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
    let settings = shell.inner.lock().expect("shell settings").settings();
    let shown = app.tray_by_id(TRAY_ID).is_some();
    match close_action(&settings, shown, crate::quit::exiting(app)) {
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
    show_main_window(app);
    let _ = app.emit(QUICK_CAPTURE_REQUESTED, ());
}

fn sync_tray<R: Runtime>(app: &AppHandle<R>, wanted: bool) {
    let shown = app.tray_by_id(TRAY_ID).is_some();
    if wanted && !shown {
        if let Err(error) = build_tray(app) {
            eprintln!("Tasknotes could not show its menu-bar item: {error}");
        }
    } else if !wanted && shown {
        app.remove_tray_by_id(TRAY_ID);
    }
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, TRAY_OPEN, "Open Tasknotes", true, None::<&str>)?,
            &MenuItem::with_id(app, TRAY_CAPTURE, "New Task…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, TRAY_QUIT, "Quit Tasknotes", true, None::<&str>)?,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Tasknotes")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_tray_menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn on_tray_menu<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        TRAY_OPEN => show_main_window(app),
        TRAY_CAPTURE => open_quick_capture(app),
        // The same path as ⌘Q: the page saves Notes first.
        TRAY_QUIT => crate::quit::request(app),
        _ => {}
    }
}

fn view<R: Runtime>(app: &AppHandle<R>, inner: &Inner) -> ShellView {
    let remembered_label = inner
        .capture
        .setting()
        .shortcut
        .as_deref()
        .and_then(|saved| crate::quick_capture::validate_shortcut(saved).ok())
        .map(|accepted| accepted.label);
    ShellView {
        keep_in_menu_bar: inner.keep_in_menu_bar,
        menu_bar_unavailable: inner.keep_in_menu_bar && app.tray_by_id(TRAY_ID).is_none(),
        quick_capture: inner.capture.status().clone(),
        remembered_label,
        settings_unreadable: inner.unreadable,
    }
}

fn save(shell: &Shell, inner: &mut Inner) -> Result<(), String> {
    save_settings(&shell.path, &inner.settings())
        .map_err(|error| format!("Could not save the setting: {error}"))?;
    inner.unreadable = false;
    Ok(())
}

#[tauri::command]
pub fn get_shell_settings(app: AppHandle, shell: State<'_, Shell>) -> ShellView {
    let inner = shell.inner.lock().expect("shell settings");
    view(&app, &inner)
}

#[tauri::command]
pub fn set_keep_in_menu_bar(
    app: AppHandle,
    shell: State<'_, Shell>,
    enabled: bool,
) -> Result<ShellView, String> {
    let mut inner = shell.inner.lock().expect("shell settings");
    let before = inner.keep_in_menu_bar;
    inner.keep_in_menu_bar = enabled;
    if let Err(error) = save(&shell, &mut inner) {
        inner.keep_in_menu_bar = before;
        return Err(error);
    }
    sync_tray(&app, enabled);
    Ok(view(&app, &inner))
}

/// A refused or unavailable shortcut is not an error: the view says why, and
/// the previous shortcut (if any) keeps working.
#[tauri::command]
pub fn set_quick_capture(
    app: AppHandle,
    shell: State<'_, Shell>,
    input: QuickCaptureRequest,
) -> Result<ShellView, String> {
    let mut inner = shell.inner.lock().expect("shell settings");
    if inner.capture.change(input, &mut SystemShortcuts(&app)) {
        // Registered already; it works this session even if saving fails.
        save(&shell, &mut inner)?;
    }
    Ok(view(&app, &inner))
}
