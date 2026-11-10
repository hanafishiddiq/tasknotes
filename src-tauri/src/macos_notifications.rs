//! What Tasknotes asks macOS about its notifications beyond what the
//! notification plugin offers: how they are shown (read, never changed),
//! for the one-time Pomodoro tip, and taking a delivered Pomodoro
//! notification away once it is answered elsewhere.
//!
//! The two paths differ, and that is known: the plugin (2.8.0, through
//! `notify-rust` without its `preview-macos-un` feature) posts with the
//! deprecated `NSUserNotificationCenter`, so withdrawing goes through that
//! center too; the style is read from `UNUserNotificationCenter`, which
//! answers for the same per-app entry in System Settings › Notifications.

/// Whether notifications stay on screen until the person acts on them
/// ("Persistent", the Alerts style). `None` when it cannot be read: outside a
/// bundled app (a development build) or when macOS does not answer in time.
#[cfg(target_os = "macos")]
pub fn persistent() -> Option<bool> {
    use std::ptr::NonNull;
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2_foundation::NSBundle;
    use objc2_user_notifications::{
        UNAlertStyle, UNNotificationSettings, UNUserNotificationCenter,
    };

    // `currentNotificationCenter` ends the process when it is not in an app
    // bundle with an identifier, as in `cargo run` or tests.
    let bundle = NSBundle::mainBundle();
    bundle.bundleIdentifier()?;
    if !bundle.bundlePath().to_string().ends_with(".app") {
        return None;
    }
    let (sender, receiver) = mpsc::channel();
    let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        // SAFETY: macOS hands the handler a valid settings object for the call.
        let style = unsafe { settings.as_ref() }.alertStyle();
        let _ = sender.send(style);
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .getNotificationSettingsWithCompletionHandler(&handler);
    // The handler runs on a queue of macOS's own, so waiting here is safe
    // off the main thread (the command that calls this is async).
    let style = receiver.recv_timeout(Duration::from_secs(2)).ok()?;
    Some(style == UNAlertStyle::Alert)
}

#[cfg(not(target_os = "macos"))]
pub fn persistent() -> Option<bool> {
    None
}

/// Takes away the delivered notifications with one of these titles, so
/// their buttons do not linger once answered from the menu bar or the
/// window. Call on the main thread.
///
/// The plugin cannot remove a notification on macOS, but the notification
/// it posts waits on a thread of its own until it is answered or leaves
/// Notification Center (`mac-notification-sys` polls for that every half
/// second); taking it away here also ends that thread.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub fn withdraw_delivered(titles: &[&str]) {
    use objc2_foundation::{NSBundle, NSUserNotificationCenter};

    // Without a bundle identifier there is no center, and nothing of ours
    // was delivered (a bare development run before its first notification).
    if NSBundle::mainBundle().bundleIdentifier().is_none() {
        return;
    }
    let center = NSUserNotificationCenter::defaultUserNotificationCenter();
    for delivered in center.deliveredNotifications().to_vec() {
        let ours = delivered
            .title()
            .is_some_and(|title| titles.contains(&title.to_string().as_str()));
        if ours {
            center.removeDeliveredNotification(&delivered);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn withdraw_delivered(_titles: &[&str]) {}

/// Opens Tasknotes' page in the system's notification settings.
#[cfg(target_os = "macos")]
pub fn open_settings(identifier: &str) -> Result<(), String> {
    let url = settings_url(identifier);
    std::process::Command::new("/usr/bin/open")
        .arg(&url)
        .status()
        .map_err(|error| format!("Could not open System Settings: {error}"))
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| "Could not open System Settings".to_owned())
        })
}

#[cfg(not(target_os = "macos"))]
pub fn open_settings(_identifier: &str) -> Result<(), String> {
    Err("Notification settings can only be opened on macOS".into())
}

/// The System Settings page for one app's notifications.
pub fn settings_url(identifier: &str) -> String {
    format!("x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={identifier}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_link_names_tasknotes() {
        assert_eq!(
            settings_url("store.adamaja.tasknotes"),
            "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=store.adamaja.tasknotes"
        );
    }

    #[test]
    fn outside_an_app_bundle_the_style_is_unknown_and_nothing_crashes() {
        // The test binary is not in an app bundle.
        assert_eq!(persistent(), None);
    }
}
