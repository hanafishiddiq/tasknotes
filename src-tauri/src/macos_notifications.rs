//! Tasknotes' notifications on macOS, all through `UNUserNotificationCenter`:
//! showing a notification with its buttons, hearing which one was clicked,
//! taking it away once it is answered elsewhere, and reading how
//! notifications are shown (for the one-time Pomodoro tip; read, never
//! changed).
//!
//! Only this center, never the older `NSUserNotificationCenter`: on macOS
//! 27.2 the batch-3 install had every call to the older one refused ("You
//! can't mix modern clients with legacy clients", in `usernoted`'s log) while
//! the style read through this one kept working, so no Pomodoro notification
//! appeared (batch 3, Issue 06).
//!
//! Any thread: the timer, the commands and quitting call in from their own
//! threads, and the native check of Issue 06 added a request from macOS's own
//! completion queue, which was delivered.
//!
//! Outside a bundled app (`cargo run`, the tests) there is no center: the
//! style is unknown, and listening, showing and withdrawing do nothing.

/// One button of a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Button {
    /// Handed back when it is clicked.
    pub id: &'static str,
    pub title: &'static str,
}

/// One kind of notification: its buttons are registered under its identifier
/// at launch ([`listen`]), and a notification of this kind is shown and
/// taken away by the same identifier, so one replaces the one before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    pub identifier: &'static str,
    /// In the order macOS lists them.
    pub buttons: &'static [Button],
}

/// One notification to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    /// Must be one of the kinds given to [`listen`], for its buttons.
    pub kind: Kind,
    /// A number handed back with a click, so a click on an older
    /// notification can be told apart.
    pub tag: i32,
    pub title: &'static str,
    pub body: &'static str,
    /// The system's notification sound.
    pub sound: bool,
}

/// What was clicked on a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clicked {
    /// The notification itself.
    Notification,
    /// One of its buttons, by [`Button::id`].
    Button(String),
}

/// A click on one of Tasknotes' notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Click {
    pub clicked: Clicked,
    /// The [`Post::tag`] of the clicked notification.
    pub tag: Option<i32>,
}

/// Once, at launch: registers each kind's buttons, hears the clicks
/// (`on_click` runs on a thread macOS chooses), and asks macOS whether
/// Tasknotes may show notifications. macOS asks the person only the first
/// time; the older center asked at launch too.
#[cfg(target_os = "macos")]
pub fn listen(kinds: &[Kind], on_click: impl Fn(Click) + Send + Sync + 'static) {
    mac::listen(kinds, Box::new(on_click));
}

#[cfg(not(target_os = "macos"))]
pub fn listen(_kinds: &[Kind], _on_click: impl Fn(Click) + Send + Sync + 'static) {}

/// Shows a notification, unless Tasknotes may not show notifications. While
/// Tasknotes is in front it goes to Notification Center without a banner or
/// sound, as with the older center.
#[cfg(target_os = "macos")]
pub fn show(post: Post) {
    mac::show(post);
}

#[cfg(not(target_os = "macos"))]
pub fn show(_post: Post) {}

/// Takes away the notifications of these kinds, shown or about to be, so
/// their buttons do not linger once answered from the menu bar or the window.
#[cfg(target_os = "macos")]
pub fn withdraw(kinds: &[Kind]) {
    mac::withdraw(kinds);
}

#[cfg(not(target_os = "macos"))]
pub fn withdraw(_kinds: &[Kind]) {}

/// Whether notifications stay on screen until the person acts on them
/// ("Persistent", the Alerts style). `None` when it cannot be read: outside a
/// bundled app (a development build) or when macOS does not answer in time.
#[cfg(target_os = "macos")]
pub fn persistent() -> Option<bool> {
    mac::persistent()
}

#[cfg(not(target_os = "macos"))]
pub fn persistent() -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ptr::NonNull;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool, ProtocolObject};
    use objc2::{AnyThread, define_class, msg_send};
    use objc2_foundation::{
        NSArray, NSBundle, NSDictionary, NSError, NSNumber, NSObject, NSObjectProtocol, NSSet,
        NSString,
    };
    use objc2_user_notifications::{
        UNAlertStyle, UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
        UNNotificationAction, UNNotificationActionOptions, UNNotificationCategory,
        UNNotificationCategoryOptions, UNNotificationDefaultActionIdentifier,
        UNNotificationDismissActionIdentifier, UNNotificationPresentationOptions,
        UNNotificationRequest, UNNotificationResponse, UNNotificationSettings, UNNotificationSound,
        UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    use super::{Click, Clicked, Kind, Post};

    /// The key of [`Post::tag`] in a notification's user info.
    const TAG_KEY: &str = "tasknotes.tag";

    type OnClick = Box<dyn Fn(Click) + Send + Sync>;
    static ON_CLICK: OnceLock<OnClick> = OnceLock::new();
    /// macOS said Tasknotes may show notifications.
    static ALLOWED: AtomicBool = AtomicBool::new(false);

    /// The center, only in a bundled app: `currentNotificationCenter` ends
    /// the process without a bundle identifier, as in `cargo run` or tests.
    fn center() -> Option<Retained<UNUserNotificationCenter>> {
        let bundle = NSBundle::mainBundle();
        bundle.bundleIdentifier()?;
        if !bundle.bundlePath().to_string().ends_with(".app") {
            return None;
        }
        Some(UNUserNotificationCenter::currentNotificationCenter())
    }

    define_class!(
        /// Hears the clicks, and decides how a notification shows while
        /// Tasknotes is in front.
        #[unsafe(super(NSObject))]
        #[name = "TasknotesNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                // The window already says so; as with the older center, no
                // banner or sound while Tasknotes is in front.
                completion_handler.call((UNNotificationPresentationOptions::List,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion_handler: &block2::DynBlock<dyn Fn()>,
            ) {
                if let (Some(on_click), Some(click)) = (ON_CLICK.get(), click(response)) {
                    on_click(click);
                }
                completion_handler.call(());
            }
        }
    );

    impl Delegate {
        fn new() -> Retained<Self> {
            let this = Self::alloc().set_ivars(());
            // SAFETY: `init` is NSObject's designated initializer.
            unsafe { msg_send![super(this), init] }
        }
    }

    /// What a response says was clicked; nothing for a dismissal.
    fn click(response: &UNNotificationResponse) -> Option<Click> {
        let action = response.actionIdentifier();
        // SAFETY: Apple's constant strings, valid for the life of the process.
        let (tap, dismiss) = unsafe {
            (
                UNNotificationDefaultActionIdentifier,
                UNNotificationDismissActionIdentifier,
            )
        };
        let clicked = if &*action == tap {
            Clicked::Notification
        } else if &*action == dismiss {
            return None;
        } else {
            Clicked::Button(action.to_string())
        };
        let tag = response
            .notification()
            .request()
            .content()
            .userInfo()
            .objectForKey(&NSString::from_str(TAG_KEY))
            .and_then(|value| value.downcast::<NSNumber>().ok())
            .map(|number| number.intValue());
        Some(Click { clicked, tag })
    }

    pub fn listen(kinds: &[Kind], on_click: OnClick) {
        let Some(center) = center() else {
            return;
        };
        if ON_CLICK.set(on_click).is_err() {
            return;
        }
        let categories: Vec<Retained<UNNotificationCategory>> = kinds
            .iter()
            .map(|kind| {
                let actions: Vec<Retained<UNNotificationAction>> = kind
                    .buttons
                    .iter()
                    .map(|button| {
                        UNNotificationAction::actionWithIdentifier_title_options(
                            &NSString::from_str(button.id),
                            &NSString::from_str(button.title),
                            UNNotificationActionOptions::empty(),
                        )
                    })
                    .collect();
                UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                    &NSString::from_str(kind.identifier),
                    &NSArray::from_retained_slice(&actions),
                    &NSArray::new(),
                    UNNotificationCategoryOptions::empty(),
                )
            })
            .collect();
        center.setNotificationCategories(&NSSet::from_retained_slice(&categories));
        let delegate = Delegate::new();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The center holds its delegate weakly; this one lives as long as the app.
        std::mem::forget(delegate);
        ask(&center, None);
    }

    pub fn show(post: Post) {
        let Some(center) = center() else {
            return;
        };
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(post.title));
        content.setBody(&NSString::from_str(post.body));
        content.setCategoryIdentifier(&NSString::from_str(post.kind.identifier));
        if post.sound {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }
        let info = NSDictionary::from_retained_objects(
            &[&*NSString::from_str(TAG_KEY)],
            &[NSNumber::new_i32(post.tag)],
        );
        // SAFETY: reading this immutable NSDictionary<NSString, NSNumber> as
        // NSDictionary<AnyObject, AnyObject> only widens its generics, and
        // NSString keys with NSNumber values are the property-list types a
        // notification's user info requires.
        unsafe { content.setUserInfo(info.cast_unchecked::<AnyObject, AnyObject>()) };
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(post.kind.identifier),
            &content,
            None,
        );
        if ALLOWED.load(Ordering::Relaxed) {
            add(&center, &request);
        } else {
            // macOS has not said yes since launch (it had not answered yet,
            // or the person said no): ask again and show it if the answer is yes.
            ask(&center, Some(request));
        }
    }

    /// Asks macOS whether Tasknotes may show notifications, then shows `then`
    /// if it may.
    fn ask(center: &UNUserNotificationCenter, then: Option<Retained<UNNotificationRequest>>) {
        let asked = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let granted = granted.as_bool();
            ALLOWED.store(granted, Ordering::Relaxed);
            if let Some(error) = NonNull::new(error) {
                // SAFETY: macOS hands the handler a valid error for the call.
                let error = unsafe { error.as_ref() };
                eprintln!(
                    "Tasknotes could not ask macOS about notifications: {}",
                    error.localizedDescription()
                );
            }
            match (&then, granted) {
                (Some(request), true) => add(
                    &UNUserNotificationCenter::currentNotificationCenter(),
                    request,
                ),
                (Some(_), false) => eprintln!(
                    "Tasknotes may not show notifications (System Settings › Notifications)"
                ),
                (None, _) => {}
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &asked,
        );
    }

    fn add(center: &UNUserNotificationCenter, request: &UNNotificationRequest) {
        let added = RcBlock::new(|error: *mut NSError| {
            if let Some(error) = NonNull::new(error) {
                // SAFETY: macOS hands the handler a valid error for the call.
                let error = unsafe { error.as_ref() };
                eprintln!(
                    "Tasknotes could not show the notification: {}",
                    error.localizedDescription()
                );
            }
        });
        center.addNotificationRequest_withCompletionHandler(request, Some(&added));
    }

    pub fn withdraw(kinds: &[Kind]) {
        let Some(center) = center() else {
            return;
        };
        let identifiers: Vec<Retained<NSString>> = kinds
            .iter()
            .map(|kind| NSString::from_str(kind.identifier))
            .collect();
        let identifiers = NSArray::from_retained_slice(&identifiers);
        center.removeDeliveredNotificationsWithIdentifiers(&identifiers);
        center.removePendingNotificationRequestsWithIdentifiers(&identifiers);
    }

    pub fn persistent() -> Option<bool> {
        let center = center()?;
        let (sender, receiver) = mpsc::channel();
        let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            // SAFETY: macOS hands the handler a valid settings object for the call.
            let style = unsafe { settings.as_ref() }.alertStyle();
            let _ = sender.send(style);
        });
        center.getNotificationSettingsWithCompletionHandler(&handler);
        // The handler runs on a queue of macOS's own, so waiting here is safe
        // off the main thread (the command that calls this is async).
        let style = receiver.recv_timeout(Duration::from_secs(2)).ok()?;
        Some(style == UNAlertStyle::Alert)
    }
}

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
        // The test binary is not in an app bundle: no center is asked.
        let kind = Kind {
            identifier: "tasknotes-test",
            buttons: &[],
        };
        listen(&[kind], |_| {});
        assert_eq!(persistent(), None);
        show(Post {
            kind,
            tag: 1,
            title: "Synthetic",
            body: "Synthetic",
            sound: false,
        });
        withdraw(&[kind]);
    }
}
