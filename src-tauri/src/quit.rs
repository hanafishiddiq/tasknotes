//! Quitting waits for Notes to save. Cmd+Q and closing the window ask the page
//! first; the page answers through `quit_ready` once its unsaved text has
//! reached the core, or keeps the app open when it could not save it. A page
//! that never answers cannot hold the app open.
//!
//! Quit from the Dock or a system shutdown still ends the app at once: macOS
//! sends those straight to `applicationWillTerminate`, which the window toolkit
//! gives no chance to delay.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tasknotes_core::tasks::BUSY_TIMEOUT;
use tauri::menu::{Menu, MenuEvent, MenuItem, MenuItemKind};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Window, WindowEvent};

const QUIT_MENU_ID: &str = "tasknotes-quit";
const QUIT_REQUESTED: &str = "tasknotes://quit-requested";
/// Room for the last save to wait out another writer's lock and still answer,
/// short enough that a hung page does not trap the person.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(15);
const _: () = assert!(ANSWER_TIMEOUT.as_secs() > 2 * BUSY_TIMEOUT.as_secs());

/// Numbered quit requests and the newest one the page has answered.
#[derive(Default)]
pub struct QuitState {
    asked: AtomicU64,
    answered: AtomicU64,
    exiting: AtomicBool,
}

impl QuitState {
    fn ask(&self) -> u64 {
        self.asked.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Answering a request also settles the older ones; they asked the same thing.
    fn answer(&self, ticket: u64) {
        self.answered.fetch_max(ticket, Ordering::SeqCst);
    }

    fn answered(&self, ticket: u64) -> bool {
        self.answered.load(Ordering::SeqCst) >= ticket
    }
}

/// The default menu, with its Quit item replaced by one that asks the page first.
pub fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let Some(app_menu) = menu
        .items()?
        .into_iter()
        .find_map(|item| item.as_submenu().cloned())
    else {
        return Ok(menu);
    };
    for (position, item) in app_menu.items()?.into_iter().enumerate() {
        let MenuItemKind::Predefined(predefined) = &item else {
            continue;
        };
        let text = predefined.text()?;
        if text.starts_with("Quit") {
            app_menu.remove_at(position)?;
            let quit = MenuItem::with_id(app, QUIT_MENU_ID, text, true, Some("CmdOrCtrl+Q"))?;
            app_menu.insert(&quit, position)?;
            break;
        }
    }
    Ok(menu)
}

pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if event.id() == QUIT_MENU_ID {
        request(app);
    }
}

pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && !window.state::<QuitState>().exiting.load(Ordering::SeqCst)
    {
        api.prevent_close();
        request(window.app_handle());
    }
}

fn request<R: Runtime>(app: &AppHandle<R>) {
    let ticket = app.state::<QuitState>().ask();
    if app.emit(QUIT_REQUESTED, ticket).is_err() {
        return exit(app);
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(ANSWER_TIMEOUT);
        if !app.state::<QuitState>().answered(ticket) {
            exit(&app);
        }
    });
}

fn exit<R: Runtime>(app: &AppHandle<R>) {
    app.state::<QuitState>()
        .exiting
        .store(true, Ordering::SeqCst);
    app.exit(0);
}

/// The page's answer to a quit request: quit now, or stay open to show why not.
#[tauri::command]
pub fn quit_ready(app: AppHandle, state: State<'_, QuitState>, ticket: u64, proceed: bool) {
    state.answer(ticket);
    if proceed {
        exit(&app);
    }
}

#[cfg(test)]
mod tests {
    use super::QuitState;

    #[test]
    fn an_answer_settles_only_the_requests_it_answers() {
        let state = QuitState::default();
        let first = state.ask();
        let second = state.ask();
        state.answer(first);
        assert!(state.answered(first));
        assert!(!state.answered(second));
        state.answer(second);
        // A late answer to an older request does not reopen a newer one.
        state.answer(first);
        assert!(state.answered(second));
    }
}
