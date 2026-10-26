//! What the desktop shell knows between events: the saved settings, the Quick
//! Capture registration, whether the tray item is really there, and the
//! one-off things the page collects (a capture asked for before it was
//! listening, the launch notice). `shell` connects this to Tauri; everything
//! here is tested without a window.

use std::path::PathBuf;

use serde::Serialize;

use crate::platform::Platform;
use crate::quick_capture::{QuickCapture, QuickCaptureRequest, QuickCaptureView, Registrar};
use crate::shell_settings::{
    CloseAction, Loaded, ShellSettings, close_action, load_settings, save_settings,
};

/// What Settings shows.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellView {
    keep_in_menu_bar: bool,
    /// Where the tray item lives on this system, e.g. "menu bar".
    tray_place: &'static str,
    /// What closing the window does right now, in words.
    close_hint: String,
    #[serde(flatten)]
    quick_capture: QuickCaptureView,
    /// The settings file could not be read at launch; the defaults are in use.
    settings_unreadable: bool,
}

pub struct ShellState {
    path: PathBuf,
    platform: Platform,
    settings: ShellSettings,
    capture: QuickCapture,
    unreadable: bool,
    /// The tray item was built and is on screen.
    tray_shown: bool,
    /// Quick Capture was asked for and the page has not collected it yet.
    pending_capture: bool,
    /// Why the saved shortcut could not be used at launch, until the page shows it.
    launch_notice: Option<String>,
}

impl ShellState {
    /// Reads the saved settings and registers the shortcut when it is on.
    pub fn launch(path: PathBuf, platform: Platform, system: &mut impl Registrar) -> Self {
        let Loaded {
            settings,
            unreadable,
        } = load_settings(&path);
        let mut capture = QuickCapture::new(platform);
        capture.start(&settings.quick_capture, system);
        let launch_notice = capture.launch_notice();
        Self {
            path,
            platform,
            settings,
            capture,
            unreadable,
            tray_shown: false,
            pending_capture: false,
            launch_notice,
        }
    }

    pub fn keep_in_menu_bar(&self) -> bool {
        self.settings.keep_in_menu_bar
    }

    pub fn tray_shown(&self) -> bool {
        self.tray_shown
    }

    /// Records whether the tray item could be built. Without it, closing the
    /// window quits: hiding it would leave no way back.
    pub fn record_tray(&mut self, shown: bool) {
        self.tray_shown = shown;
    }

    pub fn close_action(&self, exiting: bool) -> CloseAction {
        close_action(&self.settings, self.tray_shown, exiting)
    }

    fn save(&mut self) -> Result<(), String> {
        save_settings(&self.path, &self.settings)
            .map_err(|error| format!("Could not save the setting: {error}"))?;
        self.unreadable = false;
        Ok(())
    }

    /// Saves menu-bar mode. When saving fails nothing changes and the error is returned.
    pub fn set_keep_in_menu_bar(&mut self, enabled: bool) -> Result<(), String> {
        let before = self.settings.keep_in_menu_bar;
        self.settings.keep_in_menu_bar = enabled;
        self.save()
            .inspect_err(|_| self.settings.keep_in_menu_bar = before)
    }

    /// A refused or unavailable shortcut is not an error: the view says why and
    /// the previous shortcut keeps working. When saving fails the change is
    /// undone, registration included, and the error is returned.
    pub fn set_quick_capture(
        &mut self,
        request: QuickCaptureRequest,
        system: &mut impl Registrar,
    ) -> Result<(), String> {
        let before_capture = self.capture.clone();
        let before_setting = self.settings.quick_capture.clone();
        if !self
            .capture
            .change(&mut self.settings.quick_capture, request, system)
        {
            return Ok(());
        }
        self.save().inspect_err(|_| {
            self.capture.restore(before_capture, system);
            self.settings.quick_capture = before_setting;
        })
    }

    /// The shortcut or the tray asked for Quick Capture. Kept until the page
    /// collects it, so a press before the page is listening is not lost.
    pub fn request_capture(&mut self) {
        self.pending_capture = true;
    }

    pub fn take_pending_capture(&mut self) -> bool {
        std::mem::take(&mut self.pending_capture)
    }

    /// The launch-time shortcut problem, handed out once.
    pub fn take_launch_notice(&mut self) -> Option<String> {
        self.launch_notice.take()
    }

    pub fn view(&self) -> ShellView {
        let platform = self.platform;
        let close_hint = match self.close_action(false) {
            CloseAction::HideToMenuBar => format!(
                "Closing the window leaves Tasknotes in the {}. Choose Quit Tasknotes there, \
                 or press {}, to quit.",
                platform.tray_place(),
                platform.quit_keys()
            ),
            _ if self.settings.keep_in_menu_bar => format!(
                "Tasknotes could not show its {} here, so closing the window still quits.",
                platform.tray_item()
            ),
            _ => "Closing the window quits Tasknotes.".into(),
        };
        ShellView {
            keep_in_menu_bar: self.settings.keep_in_menu_bar,
            tray_place: platform.tray_place(),
            close_hint,
            quick_capture: self.capture.view(&self.settings.quick_capture),
            settings_unreadable: self.unreadable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quick_capture::StatusView;
    use crate::shell_settings::SETTINGS_FILE;
    use std::path::Path;

    /// Stands in for the operating system's shortcut table.
    #[derive(Default)]
    struct FakeSystem {
        taken_elsewhere: Vec<String>,
        registered: Vec<String>,
    }

    impl Registrar for FakeSystem {
        fn register(&mut self, accelerator: &str) -> Result<(), String> {
            if self
                .taken_elsewhere
                .iter()
                .any(|taken| taken == accelerator)
            {
                return Err("taken".into());
            }
            self.registered.push(accelerator.into());
            Ok(())
        }
        fn unregister(&mut self, accelerator: &str) {
            self.registered.retain(|held| held != accelerator);
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tasknotes-shell-state-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Makes later saves fail: the data folder is replaced by a plain file.
    fn break_the_data_folder(dir: &Path) {
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::write(dir, "not a folder").unwrap();
    }

    fn on(shortcut: &str) -> QuickCaptureRequest {
        QuickCaptureRequest {
            enabled: true,
            shortcut: Some(shortcut.into()),
        }
    }

    fn launch(dir: &Path, system: &mut FakeSystem) -> ShellState {
        ShellState::launch(dir.join(SETTINGS_FILE), Platform::Mac, system)
    }

    #[test]
    fn a_failed_save_of_menu_bar_mode_changes_nothing_and_reports_the_error() {
        let dir = scratch_dir("menu-bar-save-fails");
        let mut state = launch(&dir, &mut FakeSystem::default());
        break_the_data_folder(&dir);

        let result = state.set_keep_in_menu_bar(true);
        assert!(
            result
                .unwrap_err()
                .starts_with("Could not save the setting")
        );
        assert!(!state.keep_in_menu_bar());
        assert!(!state.view().keep_in_menu_bar);
    }

    #[test]
    fn a_failed_save_of_the_shortcut_releases_it_and_keeps_the_previous_one() {
        let dir = scratch_dir("shortcut-save-fails");
        let mut system = FakeSystem::default();
        let mut state = launch(&dir, &mut system);
        state
            .set_quick_capture(on("Control+Alt+Space"), &mut system)
            .unwrap();
        break_the_data_folder(&dir);

        let result = state.set_quick_capture(on("Alt+Shift+KeyT"), &mut system);
        assert!(
            result
                .unwrap_err()
                .starts_with("Could not save the setting")
        );
        assert_eq!(system.registered, ["Control+Alt+Space"]);
        let view = state.view().quick_capture;
        assert_eq!(view.remembered_label.as_deref(), Some("⌃⌥Space"));
        assert_eq!(
            view.quick_capture,
            StatusView::Active {
                label: "⌃⌥Space".into()
            }
        );
    }

    #[test]
    fn a_failed_save_when_first_turning_the_shortcut_on_leaves_it_off() {
        let dir = scratch_dir("first-shortcut-save-fails");
        let mut system = FakeSystem::default();
        let mut state = launch(&dir, &mut system);
        break_the_data_folder(&dir);

        assert!(
            state
                .set_quick_capture(on("Control+Alt+Space"), &mut system)
                .is_err()
        );
        assert!(system.registered.is_empty());
        assert!(!state.view().quick_capture.quick_capture_enabled);
    }

    #[test]
    fn settings_survive_a_restart() {
        let dir = scratch_dir("restart");
        let mut system = FakeSystem::default();
        let mut state = launch(&dir, &mut system);
        state.set_keep_in_menu_bar(true).unwrap();
        state
            .set_quick_capture(on("Control+Alt+Space"), &mut system)
            .unwrap();

        let mut after = FakeSystem::default();
        let state = launch(&dir, &mut after);
        assert!(state.keep_in_menu_bar());
        assert_eq!(after.registered, ["Control+Alt+Space"]);
    }

    #[test]
    fn an_unreadable_file_is_reported_until_a_setting_is_saved_again() {
        let dir = scratch_dir("unreadable");
        std::fs::write(dir.join(SETTINGS_FILE), "{ broken").unwrap();
        let mut state = launch(&dir, &mut FakeSystem::default());
        assert!(state.view().settings_unreadable);
        assert!(!state.keep_in_menu_bar());

        state.set_keep_in_menu_bar(false).unwrap();
        assert!(!state.view().settings_unreadable);
    }

    #[test]
    fn closing_quits_when_the_tray_item_could_not_be_built() {
        let dir = scratch_dir("tray-failed");
        let mut state = launch(&dir, &mut FakeSystem::default());
        state.set_keep_in_menu_bar(true).unwrap();

        state.record_tray(false);
        assert_eq!(state.close_action(false), CloseAction::Quit);
        assert_eq!(
            state.view().close_hint,
            "Tasknotes could not show its menu-bar item here, so closing the window still quits."
        );

        state.record_tray(true);
        assert_eq!(state.close_action(false), CloseAction::HideToMenuBar);
        assert_eq!(
            state.view().close_hint,
            "Closing the window leaves Tasknotes in the menu bar. Choose Quit Tasknotes there, \
             or press ⌘Q, to quit."
        );
    }

    #[test]
    fn the_close_hint_uses_this_systems_words() {
        let dir = scratch_dir("windows-words");
        let mut state = ShellState::launch(
            dir.join(SETTINGS_FILE),
            Platform::Windows,
            &mut FakeSystem::default(),
        );
        assert_eq!(
            state.view().close_hint,
            "Closing the window quits Tasknotes."
        );
        state.set_keep_in_menu_bar(true).unwrap();
        state.record_tray(true);
        let view = state.view();
        assert_eq!(view.tray_place, "notification area");
        assert_eq!(
            view.close_hint,
            "Closing the window leaves Tasknotes in the notification area. Choose Quit Tasknotes \
             there, or press Ctrl+Q, to quit."
        );
    }

    #[test]
    fn a_capture_asked_for_before_the_page_listens_waits_until_it_is_collected_once() {
        let dir = scratch_dir("cold-start");
        let mut state = launch(&dir, &mut FakeSystem::default());
        assert!(!state.take_pending_capture());

        state.request_capture();
        state.request_capture();
        assert!(state.take_pending_capture());
        assert!(!state.take_pending_capture());
    }

    #[test]
    fn a_shortcut_taken_since_the_last_launch_gives_one_launch_notice() {
        let dir = scratch_dir("launch-notice");
        let mut system = FakeSystem::default();
        let mut state = launch(&dir, &mut system);
        state
            .set_quick_capture(on("Control+Alt+Space"), &mut system)
            .unwrap();
        assert_eq!(state.take_launch_notice(), None);

        let mut relaunch = FakeSystem {
            taken_elsewhere: vec!["Control+Alt+Space".into()],
            ..FakeSystem::default()
        };
        let mut state = launch(&dir, &mut relaunch);
        let notice = state.take_launch_notice().expect("a launch notice");
        assert!(notice.contains("⌃⌥Space"), "{notice}");
        assert_eq!(state.take_launch_notice(), None);
    }
}
