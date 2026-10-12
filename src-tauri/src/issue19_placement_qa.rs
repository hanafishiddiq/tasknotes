//! Issue 19 experimental window-placement comparison ONLY.
//!
//! This module is compiled only with the explicit QA feature and refuses
//! to start outside a distinct synthetic app identifier. Normal Tasknotes
//! windows, persisted Tasks and release configurations are untouched.

use std::error::Error;
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::webview::{WebviewBuilder, WebviewWindowBuilder};
use tauri::window::WindowBuilder;
use tauri::{LogicalPosition, LogicalSize, WebviewUrl};

const QA_IDENTIFIER: &str = "store.adamaja.tasknotes.issue19placement";
const QA_TITLE: &str = "Tasknotes Issue19 Placement QA";
const WIDTH: f64 = 1120.0;
const HEIGHT: f64 = 740.0;

/// Fail closed *before the application opens a TaskStore* if a QA-only
/// compile flag was accidentally used with a normal user app bundle.
pub(crate) fn assert_qa_identity(app: &tauri::App) {
    assert_eq!(
        app.config().identifier.as_str(),
        QA_IDENTIFIER,
        "QA placement feature refuses the normal user Tasknotes app identifier"
    );
}

pub(crate) fn create_qa_window(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    assert_qa_identity(app);
    let mode = std::env::var("TASKNOTES_QA_PLACEMENT")
        .expect("Issue19 QA placement mode is required: top, child, or child-no-throttle");
    match mode.as_str() {
        "top" => {
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title(QA_TITLE)
                    .inner_size(WIDTH, HEIGHT)
                    .center()
                    .build()?;
            window.show()?;
            window.set_focus()?;
            window.as_ref().set_focus()?;
        }
        "child" | "child-no-throttle" => {
            let window = WindowBuilder::new(app, "main")
                .title(QA_TITLE)
                .inner_size(WIDTH, HEIGHT)
                .center()
                .build()?;
            let scale = window.scale_factor()?;
            let inner: LogicalSize<f64> = window.inner_size()?.to_logical(scale);
            let webview = WebviewBuilder::new("qa-content", WebviewUrl::App("index.html".into()))
                .auto_resize();
            // Keep the initial child placement variable separate from
            // ChatGrid's extra Disabled policy, so we can falsify them
            // independently instead of conflating two changes.
            let webview = if mode == "child-no-throttle" {
                webview.background_throttling(BackgroundThrottlingPolicy::Disabled)
            } else {
                webview
            };
            let webview = window.add_child(
                webview,
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(inner.width, inner.height),
            )?;
            webview.show()?;
            window.show()?;
            window.set_focus()?;
            // The parent window being key does not guarantee WKWebView is its
            // first responder. Give both variants a focused *webview*.
            webview.set_focus()?;
        }
        _ => panic!("Unsupported Issue19 placement mode; synthetic QA only"),
    }
    println!("[ISSUE19-PLACEMENT-QA] mode={mode}; bundle={QA_IDENTIFIER}");
    Ok(())
}
