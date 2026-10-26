//! The global Quick Capture shortcut: which key combinations are accepted, how
//! they read in Settings, and turning the shortcut on or off.

use serde::Serialize;

use crate::shell_settings::QuickCaptureSetting;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// A key combination Tasknotes is willing to register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedShortcut {
    /// The canonical spelling that is saved and registered, e.g. `Control+Alt+Space`.
    pub accelerator: String,
    /// How Settings shows it, e.g. `⌃⌥Space` on a Mac.
    pub label: String,
}

/// Why a key combination is not used. Settings words each one for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ShortcutProblem {
    /// Not a key combination at all.
    Invalid,
    /// One modifier (or none) would take over a shortcut every app relies on,
    /// including Tasknotes' own ⌘N, ⌘K and ⌘F.
    NeedsTwoModifiers,
    /// Media keys only work with Accessibility access, which Tasknotes does not ask for.
    UnsupportedKey,
    /// The system already uses it.
    #[serde(rename_all = "camelCase")]
    ReservedBySystem { used_for: String },
}

const MODIFIER_ORDER: [(Modifiers, &str, &str, &str); 4] = [
    (Modifiers::CONTROL, "Control", "⌃", "Ctrl+"),
    (Modifiers::ALT, "Alt", "⌥", "Alt+"),
    (Modifiers::SHIFT, "Shift", "⇧", "Shift+"),
    (Modifiers::SUPER, "Super", "⌘", "Super+"),
];

const MEDIA_KEYS: [Code; 9] = [
    Code::AudioVolumeDown,
    Code::AudioVolumeUp,
    Code::AudioVolumeMute,
    Code::MediaPlay,
    Code::MediaPause,
    Code::MediaPlayPause,
    Code::MediaStop,
    Code::MediaTrackNext,
    Code::MediaTrackPrevious,
];

/// Well-known macOS shortcuts; anything else the system holds is caught when registering.
fn system_use(mods: Modifiers, key: Code) -> Option<&'static str> {
    let control = Modifiers::CONTROL;
    let alt = Modifiers::ALT;
    let shift = Modifiers::SHIFT;
    let command = Modifiers::SUPER;
    let reserved: [(Modifiers, Code, &str); 9] = [
        (control | command, Code::Space, "Emoji & Symbols"),
        (command | alt, Code::Space, "Finder search"),
        (command | shift, Code::Digit3, "Screenshots"),
        (command | shift, Code::Digit4, "Screenshots"),
        (command | shift, Code::Digit5, "Screenshots"),
        (command | alt, Code::Escape, "Force Quit"),
        (control | command, Code::KeyQ, "Lock Screen"),
        (command | shift, Code::KeyQ, "Log Out"),
        (control | command, Code::KeyF, "Full screen"),
    ];
    reserved
        .iter()
        .find(|(m, k, _)| *m == mods && *k == key)
        .map(|(_, _, name)| *name)
}

fn is_function_key(key: Code) -> bool {
    use Code::*;
    matches!(
        key,
        F1 | F2
            | F3
            | F4
            | F5
            | F6
            | F7
            | F8
            | F9
            | F10
            | F11
            | F12
            | F13
            | F14
            | F15
            | F16
            | F17
            | F18
            | F19
            | F20
            | F21
            | F22
            | F23
            | F24
    )
}

fn key_label(key: Code) -> String {
    let name = key.to_string();
    if let Some(letter) = name.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return digit.to_string();
    }
    match key {
        Code::ArrowUp => "↑".into(),
        Code::ArrowDown => "↓".into(),
        Code::ArrowLeft => "←".into(),
        Code::ArrowRight => "→".into(),
        Code::Enter => "Return".into(),
        Code::Escape => "Esc".into(),
        Code::Backspace => "Delete".into(),
        Code::Delete => "Forward Delete".into(),
        Code::Minus => "-".into(),
        Code::Equal => "=".into(),
        Code::Comma => ",".into(),
        Code::Period => ".".into(),
        Code::Slash => "/".into(),
        Code::Backslash => "\\".into(),
        Code::Semicolon => ";".into(),
        Code::Quote => "'".into(),
        Code::Backquote => "`".into(),
        Code::BracketLeft => "[".into(),
        Code::BracketRight => "]".into(),
        _ => name,
    }
}

/// Checks a key combination typed in Settings (or read back from the settings file).
pub fn validate_shortcut(text: &str) -> Result<AcceptedShortcut, ShortcutProblem> {
    let shortcut: Shortcut = text.trim().parse().map_err(|_| ShortcutProblem::Invalid)?;
    let (mods, key) = (shortcut.mods, shortcut.key);
    if MEDIA_KEYS.contains(&key) {
        return Err(ShortcutProblem::UnsupportedKey);
    }
    let held = MODIFIER_ORDER
        .iter()
        .filter(|(m, ..)| mods.contains(*m))
        .count();
    if held < 2 && !is_function_key(key) {
        return Err(ShortcutProblem::NeedsTwoModifiers);
    }
    if let Some(used_for) = system_use(mods, key) {
        return Err(ShortcutProblem::ReservedBySystem {
            used_for: used_for.into(),
        });
    }
    let mut accelerator = String::new();
    let mut label = String::new();
    for (modifier, name, mac, other) in MODIFIER_ORDER {
        if mods.contains(modifier) {
            accelerator.push_str(name);
            accelerator.push('+');
            label.push_str(if cfg!(target_os = "macos") {
                mac
            } else {
                other
            });
        }
    }
    accelerator.push_str(&key.to_string());
    label.push_str(&key_label(key));
    Ok(AcceptedShortcut { accelerator, label })
}

/// The operating system's global shortcut table, as Quick Capture needs it.
pub trait Registrar {
    fn register(&mut self, accelerator: &str) -> Result<(), String>;
    fn unregister(&mut self, accelerator: &str);
}

/// What Settings asks for: on or off, and optionally a newly recorded combination.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCaptureRequest {
    pub enabled: bool,
    pub shortcut: Option<String>,
}

/// What Settings shows. `current` is the shortcut still working after a refused change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum QuickCaptureStatus {
    Off,
    Active {
        label: String,
    },
    /// The system would not register it: another app or macOS holds it, or it was refused.
    Unavailable {
        label: String,
        current: Option<String>,
    },
    Rejected {
        problem: ShortcutProblem,
        current: Option<String>,
    },
}

/// The saved setting, what is registered right now, and the latest outcome.
#[derive(Debug)]
pub struct QuickCapture {
    setting: QuickCaptureSetting,
    registered: Option<AcceptedShortcut>,
    status: QuickCaptureStatus,
}

impl QuickCapture {
    pub fn new(setting: QuickCaptureSetting) -> Self {
        Self {
            setting,
            registered: None,
            status: QuickCaptureStatus::Off,
        }
    }

    pub fn setting(&self) -> &QuickCaptureSetting {
        &self.setting
    }

    pub fn status(&self) -> &QuickCaptureStatus {
        &self.status
    }

    fn current_label(&self) -> Option<String> {
        self.registered.as_ref().map(|held| held.label.clone())
    }

    /// At launch: registers the saved shortcut when it is on. A failure is
    /// reported but the setting is kept, so the next launch tries again.
    pub fn start(&mut self, system: &mut impl Registrar) {
        if !self.setting.enabled {
            self.status = QuickCaptureStatus::Off;
            return;
        }
        let saved = self.setting.shortcut.clone().unwrap_or_default();
        self.status = match validate_shortcut(&saved) {
            Err(problem) => QuickCaptureStatus::Rejected {
                problem,
                current: None,
            },
            Ok(accepted) => match system.register(&accepted.accelerator) {
                Ok(()) => {
                    let label = accepted.label.clone();
                    self.registered = Some(accepted);
                    QuickCaptureStatus::Active { label }
                }
                Err(_) => QuickCaptureStatus::Unavailable {
                    label: accepted.label,
                    current: None,
                },
            },
        };
    }

    /// Applies a change from Settings. Returns whether the saved setting changed.
    /// A refused change leaves the saved setting and any working shortcut alone.
    pub fn change(&mut self, request: QuickCaptureRequest, system: &mut impl Registrar) -> bool {
        let before = self.setting.clone();
        if !request.enabled {
            if let Some(held) = self.registered.take() {
                system.unregister(&held.accelerator);
            }
            self.setting.enabled = false;
            self.status = QuickCaptureStatus::Off;
            return self.setting != before;
        }
        let wanted = request
            .shortcut
            .or_else(|| self.setting.shortcut.clone())
            .unwrap_or_default();
        let accepted = match validate_shortcut(&wanted) {
            Ok(accepted) => accepted,
            Err(problem) => {
                self.status = QuickCaptureStatus::Rejected {
                    problem,
                    current: self.current_label(),
                };
                return false;
            }
        };
        let already_held = self.registered.as_ref() == Some(&accepted);
        if !already_held {
            if system.register(&accepted.accelerator).is_err() {
                self.status = QuickCaptureStatus::Unavailable {
                    label: accepted.label,
                    current: self.current_label(),
                };
                return false;
            }
            if let Some(old) = self.registered.take() {
                system.unregister(&old.accelerator);
            }
        }
        self.setting = QuickCaptureSetting {
            enabled: true,
            shortcut: Some(accepted.accelerator.clone()),
        };
        self.status = QuickCaptureStatus::Active {
            label: accepted.label.clone(),
        };
        self.registered = Some(accepted);
        self.setting != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted(text: &str) -> AcceptedShortcut {
        validate_shortcut(text).unwrap_or_else(|problem| panic!("{text}: {problem:?}"))
    }

    #[test]
    fn two_modifiers_and_a_key_are_accepted_in_a_canonical_spelling() {
        assert_eq!(accepted("ctrl+alt+space").accelerator, "Control+Alt+Space");
        assert_eq!(accepted("Shift+Super+KeyT").accelerator, "Shift+Super+KeyT");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_label_uses_the_mac_modifier_symbols_in_menu_order() {
        assert_eq!(accepted("Super+Shift+Control+Alt+KeyT").label, "⌃⌥⇧⌘T");
        assert_eq!(accepted("Control+Alt+Space").label, "⌃⌥Space");
        assert_eq!(accepted("Alt+Shift+Digit1").label, "⌥⇧1");
    }

    #[test]
    fn a_function_key_needs_no_modifier() {
        assert_eq!(accepted("F13").accelerator, "F13");
    }

    #[test]
    fn a_single_modifier_would_take_over_ordinary_app_shortcuts() {
        // ⌘N is New task inside Tasknotes; ⌘C is Copy everywhere.
        assert_eq!(
            validate_shortcut("Super+KeyN"),
            Err(ShortcutProblem::NeedsTwoModifiers)
        );
        assert_eq!(
            validate_shortcut("Super+KeyC"),
            Err(ShortcutProblem::NeedsTwoModifiers)
        );
        assert_eq!(
            validate_shortcut("KeyT"),
            Err(ShortcutProblem::NeedsTwoModifiers)
        );
    }

    #[test]
    fn well_known_system_shortcuts_are_refused_by_name() {
        assert_eq!(
            validate_shortcut("Super+Shift+Digit4"),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Screenshots".into()
            })
        );
        assert_eq!(
            validate_shortcut("Control+Super+Space"),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Emoji & Symbols".into()
            })
        );
    }

    #[test]
    fn media_keys_are_refused_because_they_would_need_accessibility_access() {
        assert_eq!(
            validate_shortcut("Control+Alt+AudioVolumeUp"),
            Err(ShortcutProblem::UnsupportedKey)
        );
    }

    #[test]
    fn text_that_is_not_a_key_combination_is_invalid() {
        assert_eq!(
            validate_shortcut("Control+Alt"),
            Err(ShortcutProblem::Invalid)
        );
        assert_eq!(
            validate_shortcut("Hyper+KeyX"),
            Err(ShortcutProblem::Invalid)
        );
        assert_eq!(validate_shortcut(""), Err(ShortcutProblem::Invalid));
    }

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
                return Err("RegisterEventHotKey failed".into());
            }
            self.registered.push(accelerator.into());
            Ok(())
        }
        fn unregister(&mut self, accelerator: &str) {
            self.registered.retain(|held| held != accelerator);
        }
    }

    fn turn_on(shortcut: &str) -> QuickCaptureRequest {
        QuickCaptureRequest {
            enabled: true,
            shortcut: Some(shortcut.into()),
        }
    }

    #[test]
    fn it_is_off_by_default_and_registers_nothing_at_launch() {
        let mut system = FakeSystem::default();
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.start(&mut system);
        assert_eq!(capture.status(), &QuickCaptureStatus::Off);
        assert!(system.registered.is_empty());
    }

    #[test]
    fn turning_it_on_registers_the_chosen_shortcut_and_remembers_it() {
        let mut system = FakeSystem::default();
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.change(turn_on("ctrl+alt+space"), &mut system);
        assert_eq!(system.registered, ["Control+Alt+Space"]);
        assert_eq!(
            capture.status(),
            &QuickCaptureStatus::Active {
                label: label("Control+Alt+Space")
            }
        );
        assert_eq!(
            capture.setting(),
            &QuickCaptureSetting {
                enabled: true,
                shortcut: Some("Control+Alt+Space".into())
            }
        );
    }

    #[test]
    fn a_shortcut_another_app_holds_is_not_saved_and_the_previous_one_keeps_working() {
        let mut system = FakeSystem {
            taken_elsewhere: vec!["Alt+Shift+KeyT".into()],
            ..FakeSystem::default()
        };
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.change(turn_on("Control+Alt+Space"), &mut system);
        capture.change(turn_on("Alt+Shift+KeyT"), &mut system);
        assert_eq!(
            capture.status(),
            &QuickCaptureStatus::Unavailable {
                label: label("Alt+Shift+KeyT"),
                current: Some(label("Control+Alt+Space")),
            }
        );
        assert_eq!(
            capture.setting().shortcut.as_deref(),
            Some("Control+Alt+Space")
        );
        assert_eq!(system.registered, ["Control+Alt+Space"]);
    }

    #[test]
    fn a_refused_combination_leaves_everything_as_it_was() {
        let mut system = FakeSystem::default();
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.change(turn_on("Super+KeyN"), &mut system);
        assert_eq!(
            capture.status(),
            &QuickCaptureStatus::Rejected {
                problem: ShortcutProblem::NeedsTwoModifiers,
                current: None,
            }
        );
        assert_eq!(capture.setting(), &QuickCaptureSetting::default());
        assert!(system.registered.is_empty());
    }

    #[test]
    fn changing_to_a_new_shortcut_releases_the_old_one() {
        let mut system = FakeSystem::default();
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.change(turn_on("Control+Alt+Space"), &mut system);
        capture.change(turn_on("Alt+Shift+KeyT"), &mut system);
        assert_eq!(system.registered, ["Alt+Shift+KeyT"]);
    }

    #[test]
    fn turning_it_off_releases_the_shortcut_but_remembers_it_for_next_time() {
        let mut system = FakeSystem::default();
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.change(turn_on("Control+Alt+Space"), &mut system);
        capture.change(
            QuickCaptureRequest {
                enabled: false,
                shortcut: None,
            },
            &mut system,
        );
        assert!(system.registered.is_empty());
        assert_eq!(capture.status(), &QuickCaptureStatus::Off);
        // Turning it back on without recording again uses the remembered one.
        capture.change(
            QuickCaptureRequest {
                enabled: true,
                shortcut: None,
            },
            &mut system,
        );
        assert_eq!(system.registered, ["Control+Alt+Space"]);
    }

    #[test]
    fn a_saved_shortcut_taken_since_the_last_launch_is_reported_and_kept_for_the_next_launch() {
        let saved = QuickCaptureSetting {
            enabled: true,
            shortcut: Some("Control+Alt+Space".into()),
        };
        let mut system = FakeSystem {
            taken_elsewhere: vec!["Control+Alt+Space".into()],
            ..FakeSystem::default()
        };
        let mut capture = QuickCapture::new(saved.clone());
        capture.start(&mut system);
        assert_eq!(
            capture.status(),
            &QuickCaptureStatus::Unavailable {
                label: label("Control+Alt+Space"),
                current: None,
            }
        );
        assert_eq!(capture.setting(), &saved);
    }

    #[test]
    fn turning_it_on_with_nothing_recorded_asks_for_a_shortcut() {
        let mut system = FakeSystem::default();
        let mut capture = QuickCapture::new(QuickCaptureSetting::default());
        capture.change(
            QuickCaptureRequest {
                enabled: true,
                shortcut: None,
            },
            &mut system,
        );
        assert_eq!(
            capture.status(),
            &QuickCaptureStatus::Rejected {
                problem: ShortcutProblem::Invalid,
                current: None,
            }
        );
    }

    fn label(accelerator: &str) -> String {
        accepted(accelerator).label
    }
}
