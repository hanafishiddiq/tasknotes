//! Desktop shell settings: whether closing the window keeps Tasknotes in the
//! menu bar, and the global Quick Capture shortcut.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Kept in the app data folder beside the database, not in the core: they
/// describe this device's window behaviour, not Tasks or Notes, and the shell
/// needs them before the page has loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShellSettings {
    /// Off by default: closing the window quits.
    pub keep_in_menu_bar: bool,
    pub quick_capture: QuickCaptureSetting,
}

/// Off by default (owner decision 2026-10-25). The shortcut is remembered while
/// off so turning it back on needs no new recording.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QuickCaptureSetting {
    pub enabled: bool,
    /// An accelerator such as `Control+Alt+Space`.
    pub shortcut: Option<String>,
}

/// What closing the main window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Ask the page to save, then quit (the default).
    Quit,
    /// Hide the window; the menu-bar item stays.
    HideToMenuBar,
    /// A quit is already under way: let the window go.
    Close,
}

/// `menu_bar_item_shown` guards against hiding the only way back: where the
/// system has no menu bar or tray, closing quits as usual.
pub fn close_action(
    settings: &ShellSettings,
    menu_bar_item_shown: bool,
    exiting: bool,
) -> CloseAction {
    if exiting {
        CloseAction::Close
    } else if settings.keep_in_menu_bar && menu_bar_item_shown {
        CloseAction::HideToMenuBar
    } else {
        CloseAction::Quit
    }
}

/// The file name inside the app data folder.
pub const SETTINGS_FILE: &str = "shell-settings.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub settings: ShellSettings,
    /// The file exists but could not be read: the defaults apply and the file
    /// is kept as it is until the person changes a setting.
    pub unreadable: bool,
}

pub fn load_settings(path: &Path) -> Loaded {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(settings) => Loaded {
                settings,
                unreadable: false,
            },
            Err(_) => Loaded {
                settings: ShellSettings::default(),
                unreadable: true,
            },
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Loaded {
            settings: ShellSettings::default(),
            unreadable: false,
        },
        Err(_) => Loaded {
            settings: ShellSettings::default(),
            unreadable: true,
        },
    }
}

/// Writes beside the file and renames over it, so a crash never leaves half a file.
pub fn save_settings(path: &Path, settings: &ShellSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let partial = path.with_extension("json.partial");
    let text = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(&partial, text)?;
    std::fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_window_quits_by_default() {
        let settings = ShellSettings::default();
        assert_eq!(close_action(&settings, true, false), CloseAction::Quit);
    }

    #[test]
    fn closing_the_window_hides_it_when_menu_bar_mode_is_on() {
        let settings = ShellSettings {
            keep_in_menu_bar: true,
            ..ShellSettings::default()
        };
        assert_eq!(
            close_action(&settings, true, false),
            CloseAction::HideToMenuBar
        );
    }

    #[test]
    fn closing_quits_when_the_menu_bar_item_could_not_be_shown() {
        // Hiding the window then would leave no way back to Tasknotes.
        let settings = ShellSettings {
            keep_in_menu_bar: true,
            ..ShellSettings::default()
        };
        assert_eq!(close_action(&settings, false, false), CloseAction::Quit);
    }

    #[test]
    fn a_quit_already_under_way_is_never_turned_into_hiding() {
        let settings = ShellSettings {
            keep_in_menu_bar: true,
            ..ShellSettings::default()
        };
        assert_eq!(close_action(&settings, true, true), CloseAction::Close);
    }

    fn scratch_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tasknotes-shell-settings-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(SETTINGS_FILE)
    }

    #[test]
    fn a_first_launch_has_no_file_and_uses_the_defaults() {
        let path = scratch_file("first-launch");
        let loaded = load_settings(&path);
        assert_eq!(
            loaded,
            Loaded {
                settings: ShellSettings::default(),
                unreadable: false
            }
        );
    }

    #[test]
    fn saved_settings_come_back_after_a_restart() {
        let path = scratch_file("round-trip");
        let settings = ShellSettings {
            keep_in_menu_bar: true,
            quick_capture: QuickCaptureSetting {
                enabled: true,
                shortcut: Some("Control+Alt+Space".into()),
            },
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(load_settings(&path).settings, settings);
    }

    #[test]
    fn an_unreadable_file_falls_back_to_the_defaults_and_is_left_alone() {
        let path = scratch_file("corrupt");
        std::fs::write(&path, "{ not json").unwrap();
        let loaded = load_settings(&path);
        assert_eq!(
            loaded,
            Loaded {
                settings: ShellSettings::default(),
                unreadable: true
            }
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let path = scratch_file("partial");
        std::fs::write(&path, r#"{"keepInMenuBar":true}"#).unwrap();
        let loaded = load_settings(&path).settings;
        assert!(loaded.keep_in_menu_bar);
        assert_eq!(loaded.quick_capture, QuickCaptureSetting::default());
    }
}
