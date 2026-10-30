//! Ticket 17's measuring build (`--features qa-probe`), the only release build
//! that accepts another data directory. It refuses to start without one, so it
//! can never open the person's real data.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

const DATA_DIR: &str = "TASKNOTES_DEV_DATA_DIR";
const REPORT_FILE: &str = "TASKNOTES_QA_REPORT";

/// The data directory to use. Keeps the window on top without taking focus,
/// because macOS stops drawing a covered window and frames would say nothing.
pub fn prepare(app: &AppHandle, _default: PathBuf) -> Result<PathBuf, String> {
    let dir = std::env::var_os(DATA_DIR)
        .map(PathBuf::from)
        .ok_or_else(|| format!("The qa-probe build needs {DATA_DIR}"))?;
    if !dir.join("synthetic-dataset.json").exists() {
        return Err(format!("{} holds no synthetic dataset", dir.display()));
    }
    if let Some(window) = app.get_webview_window("main") {
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

/// Save what the probe measured and end the run.
#[tauri::command]
pub fn qa_report(app: AppHandle, report: serde_json::Value) -> Result<(), String> {
    let path = std::env::var_os(REPORT_FILE)
        .map(PathBuf::from)
        .ok_or_else(|| format!("The qa-probe build needs {REPORT_FILE}"))?;
    std::fs::write(path, report.to_string()).map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
