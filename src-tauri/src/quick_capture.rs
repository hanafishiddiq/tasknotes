//! The global Quick Capture shortcut: which key combinations are accepted, how
//! they read in Settings, and turning the shortcut on or off.
//!
//! The words Settings shows for a refusal are written here, per platform, so
//! the page shows what the shell says instead of restating the rules.

use serde::Serialize;

use crate::platform::Platform;
use crate::shell_settings::QuickCaptureSetting;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// A key combination Tasknotes is willing to register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedShortcut {
    /// The canonical spelling that is saved and registered, e.g. `Control+Alt+Space`.
    pub accelerator: String,
    /// How Settings shows it, e.g. `⌃⌥Space` on a Mac or `Ctrl+Alt+Space` elsewhere.
    pub label: String,
}

/// Why a key combination is not used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutProblem {
    /// Not a key combination at all.
    Invalid,
    /// One modifier (or none) would take over a shortcut every app relies on,
    /// including Tasknotes' own New task shortcut.
    NeedsTwoModifiers,
    /// Media keys need Accessibility access on a Mac, which Tasknotes does not ask for.
    UnsupportedKey,
    /// The system already uses it.
    ReservedBySystem { used_for: &'static str },
}

impl ShortcutProblem {
    /// The sentence Settings shows.
    pub fn reason(&self, platform: Platform) -> String {
        match self {
            ShortcutProblem::Invalid => "Record a key combination to turn Quick Capture on.".into(),
            ShortcutProblem::NeedsTwoModifiers => {
                let (modifiers, copy) = match platform {
                    Platform::Mac => ("⌃ ⌥ ⇧ ⌘", "⌘C"),
                    Platform::Windows => ("Ctrl, Alt, Shift, Win", "Ctrl+C"),
                    Platform::Other => ("Ctrl, Alt, Shift, Super", "Ctrl+C"),
                };
                format!(
                    "Use at least two modifier keys ({modifiers}) with a letter, number or Space, \
                     or a function key on its own. A single modifier would take over shortcuts \
                     other apps use, like {copy}."
                )
            }
            ShortcutProblem::UnsupportedKey => {
                "Media keys cannot be used. Pick another key.".into()
            }
            ShortcutProblem::ReservedBySystem { used_for } => format!(
                "{} uses this for {used_for}. Pick another combination.",
                platform.system_name_starting_a_sentence()
            ),
        }
    }
}

/// Settings' sentence for a combination the system would not register.
pub fn unavailable_reason(label: &str, platform: Platform) -> String {
    format!(
        "{label} is already used by another app or by {}, or the system refused it. \
         Pick another combination.",
        platform.system_name()
    )
}

/// Modifier: saved name, Mac symbol, Windows label, other desktops' label.
const MODIFIER_ORDER: [(Modifiers, &str, &str, &str, &str); 4] = [
    (Modifiers::CONTROL, "Control", "⌃", "Ctrl+", "Ctrl+"),
    (Modifiers::ALT, "Alt", "⌥", "Alt+", "Alt+"),
    (Modifiers::SHIFT, "Shift", "⇧", "Shift+", "Shift+"),
    (Modifiers::SUPER, "Super", "⌘", "Win+", "Super+"),
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

type Reserved = (Modifiers, Code, &'static str);

const CONTROL_SUPER: Modifiers = Modifiers::CONTROL.union(Modifiers::SUPER);
const ALT_SUPER: Modifiers = Modifiers::ALT.union(Modifiers::SUPER);
const SHIFT_SUPER: Modifiers = Modifiers::SHIFT.union(Modifiers::SUPER);
const CONTROL_ALT: Modifiers = Modifiers::CONTROL.union(Modifiers::ALT);
const CONTROL_SHIFT: Modifiers = Modifiers::CONTROL.union(Modifiers::SHIFT);

const MAC_RESERVED: &[Reserved] = &[
    (CONTROL_SUPER, Code::Space, "Emoji & Symbols"),
    (ALT_SUPER, Code::Space, "Finder search"),
    (SHIFT_SUPER, Code::Digit3, "Screenshots"),
    (SHIFT_SUPER, Code::Digit4, "Screenshots"),
    (SHIFT_SUPER, Code::Digit5, "Screenshots"),
    (ALT_SUPER, Code::Escape, "Force Quit"),
    (CONTROL_SUPER, Code::KeyQ, "Lock Screen"),
    (SHIFT_SUPER, Code::KeyQ, "Log Out"),
    (CONTROL_SUPER, Code::KeyF, "Full screen"),
];

const WINDOWS_RESERVED: &[Reserved] = &[
    (CONTROL_ALT, Code::Delete, "Security options"),
    (CONTROL_SHIFT, Code::Escape, "Task Manager"),
    (SHIFT_SUPER, Code::KeyS, "Screenshots"),
    (CONTROL_SUPER, Code::KeyD, "New desktop"),
    (CONTROL_SUPER, Code::F4, "Closing a desktop"),
    (CONTROL_SUPER, Code::ArrowLeft, "Switching desktops"),
    (CONTROL_SUPER, Code::ArrowRight, "Switching desktops"),
];

/// Common GNOME and KDE defaults.
const OTHER_RESERVED: &[Reserved] = &[
    (CONTROL_ALT, Code::Delete, "Log Out"),
    (CONTROL_ALT, Code::KeyT, "Terminal"),
    (CONTROL_ALT, Code::ArrowLeft, "Switching workspaces"),
    (CONTROL_ALT, Code::ArrowRight, "Switching workspaces"),
    (CONTROL_ALT, Code::ArrowUp, "Switching workspaces"),
    (CONTROL_ALT, Code::ArrowDown, "Switching workspaces"),
];

/// Well-known system shortcuts on this platform; anything else the system
/// holds is caught when registering.
fn system_use(platform: Platform, mods: Modifiers, key: Code) -> Option<&'static str> {
    let reserved = match platform {
        Platform::Mac => MAC_RESERVED,
        Platform::Windows => WINDOWS_RESERVED,
        Platform::Other => OTHER_RESERVED,
    };
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

fn key_label(key: Code, platform: Platform) -> String {
    let name = key.to_string();
    if let Some(letter) = name.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return digit.to_string();
    }
    let mac = platform == Platform::Mac;
    match key {
        Code::ArrowUp => "↑".into(),
        Code::ArrowDown => "↓".into(),
        Code::ArrowLeft => "←".into(),
        Code::ArrowRight => "→".into(),
        Code::Enter if mac => "Return".into(),
        Code::Enter => "Enter".into(),
        Code::Escape => "Esc".into(),
        Code::Backspace if mac => "Delete".into(),
        Code::Backspace => "Backspace".into(),
        Code::Delete if mac => "Forward Delete".into(),
        Code::Delete => "Delete".into(),
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

/// Checks a key combination typed in Settings (or read back from the settings
/// file) against the rules and the known system shortcuts of `platform`.
pub fn validate_shortcut(
    text: &str,
    platform: Platform,
) -> Result<AcceptedShortcut, ShortcutProblem> {
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
    if let Some(used_for) = system_use(platform, mods, key) {
        return Err(ShortcutProblem::ReservedBySystem { used_for });
    }
    let mut accelerator = String::new();
    let mut label = String::new();
    for (modifier, name, mac, windows, other) in MODIFIER_ORDER {
        if mods.contains(modifier) {
            accelerator.push_str(name);
            accelerator.push('+');
            label.push_str(match platform {
                Platform::Mac => mac,
                Platform::Windows => windows,
                Platform::Other => other,
            });
        }
    }
    accelerator.push_str(&key.to_string());
    label.push_str(&key_label(key, platform));
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

/// The latest outcome. `current` is the shortcut still working after a refused change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickCaptureStatus {
    Off,
    Active {
        label: String,
    },
    /// The system would not register it: another app or the system holds it, or it was refused.
    Unavailable {
        label: String,
        current: Option<String>,
    },
    Rejected {
        problem: ShortcutProblem,
        current: Option<String>,
    },
}

/// The status as Settings shows it, with the reason already worded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum StatusView {
    Off,
    Active {
        label: String,
    },
    Unavailable {
        label: String,
        reason: String,
        current: Option<String>,
    },
    Rejected {
        reason: String,
        current: Option<String>,
    },
}

/// Quick Capture's part of what Settings shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCaptureView {
    /// The saved choice; `quick_capture` says whether it is working.
    pub quick_capture_enabled: bool,
    pub quick_capture: StatusView,
    /// The recorded shortcut, also while Quick Capture is off.
    pub remembered_label: Option<String>,
}

/// What is registered right now and the latest outcome. The saved setting is
/// kept by the caller and passed in, so there is one copy of it.
#[derive(Debug, Clone)]
pub struct QuickCapture {
    platform: Platform,
    registered: Option<AcceptedShortcut>,
    status: QuickCaptureStatus,
}

impl QuickCapture {
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            registered: None,
            status: QuickCaptureStatus::Off,
        }
    }

    #[cfg(test)]
    fn status(&self) -> &QuickCaptureStatus {
        &self.status
    }

    fn current_label(&self) -> Option<String> {
        self.registered.as_ref().map(|held| held.label.clone())
    }

    /// At launch: registers the saved shortcut when it is on. A failure is
    /// reported but the setting is kept, so the next launch tries again.
    pub fn start(&mut self, setting: &QuickCaptureSetting, system: &mut impl Registrar) {
        if !setting.enabled {
            self.status = QuickCaptureStatus::Off;
            return;
        }
        let saved = setting.shortcut.clone().unwrap_or_default();
        self.status = match validate_shortcut(&saved, self.platform) {
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

    /// The main window's one-time notice when the saved shortcut could not be
    /// used at launch; `None` when it works or is off.
    pub fn launch_notice(&self) -> Option<String> {
        match &self.status {
            QuickCaptureStatus::Unavailable { label, .. } => Some(format!(
                "Quick Capture could not use {label}: it is already used by another app or by {}, \
                 or the system refused it. Choose another shortcut in Settings.",
                self.platform.system_name()
            )),
            QuickCaptureStatus::Rejected { .. } => Some(
                "Quick Capture could not use the saved shortcut. Choose another shortcut in Settings."
                    .into(),
            ),
            QuickCaptureStatus::Off | QuickCaptureStatus::Active { .. } => None,
        }
    }

    /// Applies a change from Settings to `setting`. Returns whether the saved
    /// setting changed. A refused change leaves the setting and any working
    /// shortcut alone.
    pub fn change(
        &mut self,
        setting: &mut QuickCaptureSetting,
        request: QuickCaptureRequest,
        system: &mut impl Registrar,
    ) -> bool {
        let before = setting.clone();
        if !request.enabled {
            if let Some(held) = self.registered.take() {
                system.unregister(&held.accelerator);
            }
            setting.enabled = false;
            self.status = QuickCaptureStatus::Off;
            return *setting != before;
        }
        let wanted = request
            .shortcut
            .or_else(|| setting.shortcut.clone())
            .unwrap_or_default();
        let accepted = match validate_shortcut(&wanted, self.platform) {
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
        *setting = QuickCaptureSetting {
            enabled: true,
            shortcut: Some(accepted.accelerator.clone()),
        };
        self.status = QuickCaptureStatus::Active {
            label: accepted.label.clone(),
        };
        self.registered = Some(accepted);
        *setting != before
    }

    /// Undoes a change whose save failed: releases what the change registered
    /// and takes back what was registered before.
    pub fn restore(&mut self, before: QuickCapture, system: &mut impl Registrar) {
        if self.registered == before.registered {
            *self = before;
            return;
        }
        if let Some(now) = self.registered.take() {
            system.unregister(&now.accelerator);
        }
        *self = before;
        if let Some(old) = self.registered.clone()
            && system.register(&old.accelerator).is_err()
        {
            self.registered = None;
            self.status = QuickCaptureStatus::Unavailable {
                label: old.label,
                current: None,
            };
        }
    }

    pub fn view(&self, setting: &QuickCaptureSetting) -> QuickCaptureView {
        let platform = self.platform;
        let quick_capture = match &self.status {
            QuickCaptureStatus::Off => StatusView::Off,
            QuickCaptureStatus::Active { label } => StatusView::Active {
                label: label.clone(),
            },
            QuickCaptureStatus::Unavailable { label, current } => StatusView::Unavailable {
                reason: unavailable_reason(label, platform),
                label: label.clone(),
                current: current.clone(),
            },
            QuickCaptureStatus::Rejected { problem, current } => StatusView::Rejected {
                reason: problem.reason(platform),
                current: current.clone(),
            },
        };
        QuickCaptureView {
            quick_capture_enabled: setting.enabled,
            quick_capture,
            remembered_label: setting
                .shortcut
                .as_deref()
                .and_then(|saved| validate_shortcut(saved, platform).ok())
                .map(|accepted| accepted.label),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: Platform = Platform::Mac;

    fn accepted_on(text: &str, platform: Platform) -> AcceptedShortcut {
        validate_shortcut(text, platform).unwrap_or_else(|problem| panic!("{text}: {problem:?}"))
    }

    fn accepted(text: &str) -> AcceptedShortcut {
        accepted_on(text, MAC)
    }

    #[test]
    fn two_modifiers_and_a_key_are_accepted_in_a_canonical_spelling() {
        assert_eq!(accepted("ctrl+alt+space").accelerator, "Control+Alt+Space");
        assert_eq!(accepted("Shift+Super+KeyT").accelerator, "Shift+Super+KeyT");
    }

    #[test]
    fn the_label_uses_the_mac_modifier_symbols_in_menu_order_on_a_mac() {
        assert_eq!(accepted("Super+Shift+Control+Alt+KeyT").label, "⌃⌥⇧⌘T");
        assert_eq!(accepted("Control+Alt+Space").label, "⌃⌥Space");
        assert_eq!(accepted("Alt+Shift+Digit1").label, "⌥⇧1");
        assert_eq!(accepted("Control+Alt+Enter").label, "⌃⌥Return");
    }

    #[test]
    fn the_label_spells_out_the_keys_on_windows_and_other_desktops() {
        let windows = Platform::Windows;
        let other = Platform::Other;
        assert_eq!(
            accepted_on("Super+Shift+KeyT", windows).label,
            "Shift+Win+T"
        );
        assert_eq!(
            accepted_on("Control+Alt+Space", windows).label,
            "Ctrl+Alt+Space"
        );
        assert_eq!(
            accepted_on("Control+Alt+Enter", windows).label,
            "Ctrl+Alt+Enter"
        );
        assert_eq!(
            accepted_on("Super+Shift+KeyT", other).label,
            "Shift+Super+T"
        );
        assert_eq!(
            accepted_on("Control+Alt+Backspace", other).label,
            "Ctrl+Alt+Backspace"
        );
    }

    #[test]
    fn a_function_key_needs_no_modifier() {
        assert_eq!(accepted("F13").accelerator, "F13");
    }

    #[test]
    fn a_single_modifier_would_take_over_ordinary_app_shortcuts() {
        // ⌘N is New task inside Tasknotes; ⌘C is Copy everywhere.
        for text in ["Super+KeyN", "Super+KeyC", "KeyT", "Control+KeyN"] {
            for platform in [Platform::Mac, Platform::Windows, Platform::Other] {
                assert_eq!(
                    validate_shortcut(text, platform),
                    Err(ShortcutProblem::NeedsTwoModifiers),
                    "{text} on {platform:?}"
                );
            }
        }
    }

    #[test]
    fn well_known_mac_shortcuts_are_refused_by_name_on_a_mac_only() {
        assert_eq!(
            validate_shortcut("Super+Shift+Digit4", MAC),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Screenshots"
            })
        );
        assert_eq!(
            validate_shortcut("Control+Super+Space", MAC),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Emoji & Symbols"
            })
        );
        assert!(validate_shortcut("Super+Shift+Digit4", Platform::Windows).is_ok());
        assert!(validate_shortcut("Control+Super+Space", Platform::Other).is_ok());
    }

    #[test]
    fn well_known_windows_and_desktop_shortcuts_are_refused_on_their_own_system() {
        assert_eq!(
            validate_shortcut("Control+Shift+Escape", Platform::Windows),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Task Manager"
            })
        );
        assert_eq!(
            validate_shortcut("Super+Shift+KeyS", Platform::Windows),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Screenshots"
            })
        );
        assert_eq!(
            validate_shortcut("Control+Alt+KeyT", Platform::Other),
            Err(ShortcutProblem::ReservedBySystem {
                used_for: "Terminal"
            })
        );
        assert!(validate_shortcut("Control+Alt+KeyT", MAC).is_ok());
    }

    #[test]
    fn a_reserved_shortcut_is_explained_with_the_name_of_this_system() {
        let screenshots = ShortcutProblem::ReservedBySystem {
            used_for: "Screenshots",
        };
        assert_eq!(
            screenshots.reason(Platform::Mac),
            "macOS uses this for Screenshots. Pick another combination."
        );
        assert_eq!(
            screenshots.reason(Platform::Windows),
            "Windows uses this for Screenshots. Pick another combination."
        );
        assert_eq!(
            screenshots.reason(Platform::Other),
            "The desktop uses this for Screenshots. Pick another combination."
        );
    }

    #[test]
    fn the_two_modifier_rule_is_explained_with_this_systems_keys() {
        let mac = ShortcutProblem::NeedsTwoModifiers.reason(Platform::Mac);
        assert!(mac.contains("(⌃ ⌥ ⇧ ⌘)"), "{mac}");
        assert!(mac.ends_with("like ⌘C."), "{mac}");
        let windows = ShortcutProblem::NeedsTwoModifiers.reason(Platform::Windows);
        assert!(windows.contains("(Ctrl, Alt, Shift, Win)"), "{windows}");
        assert!(windows.ends_with("like Ctrl+C."), "{windows}");
        assert!(!windows.contains('⌘'), "{windows}");
    }

    #[test]
    fn a_taken_combination_is_explained_with_the_name_of_this_system() {
        assert_eq!(
            unavailable_reason("⌃⌥Space", Platform::Mac),
            "⌃⌥Space is already used by another app or by macOS, or the system refused it. \
             Pick another combination."
        );
        assert_eq!(
            unavailable_reason("Ctrl+Alt+Space", Platform::Windows),
            "Ctrl+Alt+Space is already used by another app or by Windows, or the system refused it. \
             Pick another combination."
        );
        assert!(!unavailable_reason("Ctrl+Alt+Space", Platform::Other).contains("macOS"));
    }

    #[test]
    fn media_keys_are_refused_because_they_would_need_accessibility_access() {
        assert_eq!(
            validate_shortcut("Control+Alt+AudioVolumeUp", MAC),
            Err(ShortcutProblem::UnsupportedKey)
        );
    }

    #[test]
    fn text_that_is_not_a_key_combination_is_invalid() {
        for text in ["Control+Alt", "Hyper+KeyX", ""] {
            assert_eq!(
                validate_shortcut(text, MAC),
                Err(ShortcutProblem::Invalid),
                "{text}"
            );
        }
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

    fn turn_off() -> QuickCaptureRequest {
        QuickCaptureRequest {
            enabled: false,
            shortcut: None,
        }
    }

    fn turn_back_on() -> QuickCaptureRequest {
        QuickCaptureRequest {
            enabled: true,
            shortcut: None,
        }
    }

    /// Quick Capture with its saved setting, as the shell holds them.
    struct Subject {
        capture: QuickCapture,
        setting: QuickCaptureSetting,
    }

    impl Subject {
        fn new() -> Self {
            Self::saved(QuickCaptureSetting::default())
        }
        fn saved(setting: QuickCaptureSetting) -> Self {
            Self {
                capture: QuickCapture::new(MAC),
                setting,
            }
        }
        fn change(&mut self, request: QuickCaptureRequest, system: &mut FakeSystem) -> bool {
            self.capture.change(&mut self.setting, request, system)
        }
    }

    #[test]
    fn it_is_off_by_default_and_registers_nothing_at_launch() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        subject.capture.start(&subject.setting, &mut system);
        assert_eq!(subject.capture.status(), &QuickCaptureStatus::Off);
        assert_eq!(subject.capture.launch_notice(), None);
        assert!(system.registered.is_empty());
    }

    #[test]
    fn turning_it_on_registers_the_chosen_shortcut_and_remembers_it() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        assert!(subject.change(turn_on("ctrl+alt+space"), &mut system));
        assert_eq!(system.registered, ["Control+Alt+Space"]);
        assert_eq!(
            subject.capture.view(&subject.setting),
            QuickCaptureView {
                quick_capture_enabled: true,
                quick_capture: StatusView::Active {
                    label: "⌃⌥Space".into()
                },
                remembered_label: Some("⌃⌥Space".into()),
            }
        );
        assert_eq!(
            subject.setting,
            QuickCaptureSetting {
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
        let mut subject = Subject::new();
        subject.change(turn_on("Control+Alt+Space"), &mut system);
        assert!(!subject.change(turn_on("Alt+Shift+KeyT"), &mut system));
        assert_eq!(
            subject.capture.view(&subject.setting).quick_capture,
            StatusView::Unavailable {
                label: "⌥⇧T".into(),
                reason: unavailable_reason("⌥⇧T", MAC),
                current: Some("⌃⌥Space".into()),
            }
        );
        assert_eq!(
            subject.setting.shortcut.as_deref(),
            Some("Control+Alt+Space")
        );
        assert_eq!(system.registered, ["Control+Alt+Space"]);
    }

    #[test]
    fn a_refused_combination_leaves_everything_as_it_was_and_says_why() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        assert!(!subject.change(turn_on("Super+KeyN"), &mut system));
        assert_eq!(
            subject.capture.view(&subject.setting).quick_capture,
            StatusView::Rejected {
                reason: ShortcutProblem::NeedsTwoModifiers.reason(MAC),
                current: None,
            }
        );
        assert_eq!(subject.setting, QuickCaptureSetting::default());
        assert!(system.registered.is_empty());
    }

    #[test]
    fn changing_to_a_new_shortcut_releases_the_old_one() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        subject.change(turn_on("Control+Alt+Space"), &mut system);
        subject.change(turn_on("Alt+Shift+KeyT"), &mut system);
        assert_eq!(system.registered, ["Alt+Shift+KeyT"]);
    }

    #[test]
    fn turning_it_off_releases_the_shortcut_but_remembers_it_for_next_time() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        subject.change(turn_on("Control+Alt+Space"), &mut system);
        subject.change(turn_off(), &mut system);
        assert!(system.registered.is_empty());
        let view = subject.capture.view(&subject.setting);
        assert_eq!(view.quick_capture, StatusView::Off);
        assert_eq!(view.remembered_label.as_deref(), Some("⌃⌥Space"));
        // Turning it back on without recording again uses the remembered one.
        subject.change(turn_back_on(), &mut system);
        assert_eq!(system.registered, ["Control+Alt+Space"]);
    }

    #[test]
    fn restoring_after_a_failed_save_puts_the_previous_shortcut_back() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        subject.change(turn_on("Control+Alt+Space"), &mut system);

        let before = subject.capture.clone();
        subject.change(turn_on("Alt+Shift+KeyT"), &mut system);
        subject.capture.restore(before, &mut system);
        assert_eq!(system.registered, ["Control+Alt+Space"]);
        assert_eq!(
            subject.capture.status(),
            &QuickCaptureStatus::Active {
                label: "⌃⌥Space".into()
            }
        );

        let before = subject.capture.clone();
        subject.change(turn_off(), &mut system);
        subject.capture.restore(before, &mut system);
        assert_eq!(system.registered, ["Control+Alt+Space"]);
    }

    #[test]
    fn restoring_after_turning_it_on_for_the_first_time_registers_nothing() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        let before = subject.capture.clone();
        subject.change(turn_on("Control+Alt+Space"), &mut system);
        subject.capture.restore(before, &mut system);
        assert!(system.registered.is_empty());
        assert_eq!(subject.capture.status(), &QuickCaptureStatus::Off);
    }

    #[test]
    fn a_saved_shortcut_taken_since_the_last_launch_is_reported_once_and_kept_for_the_next_launch()
    {
        let saved = QuickCaptureSetting {
            enabled: true,
            shortcut: Some("Control+Alt+Space".into()),
        };
        let mut system = FakeSystem {
            taken_elsewhere: vec!["Control+Alt+Space".into()],
            ..FakeSystem::default()
        };
        let mut subject = Subject::saved(saved.clone());
        subject.capture.start(&subject.setting, &mut system);
        assert_eq!(
            subject.capture.status(),
            &QuickCaptureStatus::Unavailable {
                label: "⌃⌥Space".into(),
                current: None,
            }
        );
        assert_eq!(
            subject.capture.launch_notice().as_deref(),
            Some(
                "Quick Capture could not use ⌃⌥Space: it is already used by another app or by \
                 macOS, or the system refused it. Choose another shortcut in Settings."
            )
        );
        assert_eq!(subject.setting, saved);
    }

    #[test]
    fn a_saved_shortcut_that_breaks_the_rules_is_reported_at_launch() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::saved(QuickCaptureSetting {
            enabled: true,
            shortcut: Some("Super+KeyN".into()),
        });
        subject.capture.start(&subject.setting, &mut system);
        assert!(system.registered.is_empty());
        assert!(subject.capture.launch_notice().is_some());
    }

    #[test]
    fn turning_it_on_with_nothing_recorded_asks_for_a_shortcut() {
        let mut system = FakeSystem::default();
        let mut subject = Subject::new();
        subject.change(turn_back_on(), &mut system);
        assert_eq!(
            subject.capture.status(),
            &QuickCaptureStatus::Rejected {
                problem: ShortcutProblem::Invalid,
                current: None,
            }
        );
    }
}
