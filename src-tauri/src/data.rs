//! Backups, restore and export for the desktop window (Ticket 13). Thin
//! commands over the core; the window shows and asks, the core decides.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tasknotes_core::backup::{Backup, RestoreOutcome};
use tasknotes_core::export::{ExportFormat, ExportRequest, ExportSummary, suggested_name};
use tasknotes_core::tasks::TaskStore;
use tauri::{AppHandle, Manager, State};

/// How often a running window checks whether today's backup exists.
const BACKUP_CHECK: Duration = Duration::from_secs(60 * 60);

/// Take the day's backup now and then once an hour while the app runs
/// (spec: backups run while any Tasknotes process is active). Idempotent per
/// day; a failure is logged and tried again an hour later.
pub fn start_daily_backups(store: TaskStore) {
    std::thread::Builder::new()
        .name("tasknotes-backups".into())
        .spawn(move || {
            loop {
                if let Err(error) = store.ensure_daily_backup() {
                    eprintln!("Tasknotes could not take today's backup: {error}");
                }
                std::thread::sleep(BACKUP_CHECK);
            }
        })
        .expect("Unable to start the backup thread");
}

#[tauri::command]
pub fn list_backups(store: State<'_, TaskStore>) -> Result<Vec<Backup>, String> {
    store.list_backups().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_backup(store: State<'_, TaskStore>) -> Result<Backup, String> {
    store.create_backup().map_err(|error| error.to_string())
}

/// The window reloads afterwards, so nothing it read before is written back.
#[tauri::command]
pub fn restore_backup(store: State<'_, TaskStore>, id: String) -> Result<RestoreOutcome, String> {
    store.restore_backup(&id).map_err(|error| error.to_string())
}

/// After a restore made by another window or the `tasknotes` command: accept
/// the restored data. Only called right before the window reloads.
#[tauri::command]
pub fn adopt_restored_data(store: State<'_, TaskStore>) -> Result<(), String> {
    store
        .adopt_restored_data()
        .map_err(|error| error.to_string())
}

/// A human export into Downloads under a new dated name; the person chooses
/// whether Notes marked Exclude from AI are included.
#[tauri::command]
pub fn export_data(
    app: AppHandle,
    store: State<'_, TaskStore>,
    format: ExportFormat,
    include_private_notes: bool,
) -> Result<ExportSummary, String> {
    let folder = app
        .path()
        .download_dir()
        .map_err(|_| "Tasknotes could not find your Downloads folder".to_owned())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default();
    store
        .export_data(ExportRequest {
            format,
            destination: unused_path(folder, &suggested_name(format, now)),
            include_private_notes,
        })
        .map_err(|error| error.to_string())
}

/// `name`, or `name (2)` and so on when an earlier export took it.
fn unused_path(folder: PathBuf, name: &str) -> PathBuf {
    let (stem, extension) = match name.strip_suffix(".json") {
        Some(stem) => (stem, ".json"),
        None => (name, ""),
    };
    let mut candidate = folder.join(name);
    let mut n = 2;
    while candidate.symlink_metadata().is_ok() {
        candidate = folder.join(format!("{stem} ({n}){extension}"));
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::unused_path;

    #[test]
    fn a_second_export_in_the_same_minute_gets_its_own_name() {
        let dir = tempfile::tempdir().unwrap();
        let name = "Tasknotes export 2026-10-26 09.26.json";
        assert_eq!(unused_path(dir.path().into(), name), dir.path().join(name));
        std::fs::write(dir.path().join(name), "synthetic").unwrap();
        assert_eq!(
            unused_path(dir.path().into(), name),
            dir.path()
                .join("Tasknotes export 2026-10-26 09.26 (2).json")
        );
    }
}
