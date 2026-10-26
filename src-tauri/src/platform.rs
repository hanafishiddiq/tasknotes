//! The desktop system Tasknotes runs on, for words and key names the person
//! sees. Every variant is compiled on every host, so the wording for other
//! systems is built and unit-tested on a Mac too.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    /// Linux and other desktops.
    Other,
}

impl Platform {
    pub const CURRENT: Platform = if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Other
    };

    /// As in "already used by another app or by macOS".
    pub fn system_name(self) -> &'static str {
        match self {
            Platform::Mac => "macOS",
            Platform::Windows => "Windows",
            Platform::Other => "the desktop",
        }
    }

    pub fn system_name_starting_a_sentence(self) -> &'static str {
        match self {
            Platform::Other => "The desktop",
            Platform::Mac | Platform::Windows => self.system_name(),
        }
    }

    /// Where the tray item lives, as in "Keep Tasknotes in the menu bar".
    pub fn tray_place(self) -> &'static str {
        match self {
            Platform::Mac => "menu bar",
            Platform::Windows => "notification area",
            Platform::Other => "system tray",
        }
    }

    /// The tray item itself.
    pub fn tray_item(self) -> &'static str {
        match self {
            Platform::Mac => "menu-bar item",
            Platform::Windows => "notification-area icon",
            Platform::Other => "tray icon",
        }
    }

    /// The Quit shortcut (`CmdOrCtrl+Q` in the app menu).
    pub fn quit_keys(self) -> &'static str {
        match self {
            Platform::Mac => "⌘Q",
            Platform::Windows | Platform::Other => "Ctrl+Q",
        }
    }
}
