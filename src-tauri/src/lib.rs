use tasknotes_core::tasks::{
    ChangeTask, CreateTask, ProgressHistoryPage, RecordProgress, SortMode, Task, TaskPage,
    TaskStore, UpdateTask,
};
use tauri::{Manager, State};

#[tauri::command]
fn list_tasks(
    store: State<'_, TaskStore>,
    limit: u32,
    cursor: Option<String>,
    sort: Option<SortMode>,
) -> Result<TaskPage, String> {
    store
        .list_tasks_sorted(limit, cursor.as_deref(), sort.unwrap_or_default())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_task(store: State<'_, TaskStore>, input: CreateTask) -> Result<Task, String> {
    store.create_task(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn update_task(store: State<'_, TaskStore>, input: UpdateTask) -> Result<Task, String> {
    store.update_task(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn change_task(store: State<'_, TaskStore>, input: ChangeTask) -> Result<Task, String> {
    store.change_task(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn record_progress(store: State<'_, TaskStore>, input: RecordProgress) -> Result<Task, String> {
    store
        .record_progress(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_progress_entries(
    store: State<'_, TaskStore>,
    task_id: String,
    limit: u32,
    before: Option<i64>,
) -> Result<ProgressHistoryPage, String> {
    store
        .list_progress_entries(&task_id, limit, before)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Data is private to this macOS user, never kept in the public source repository.
            let app_data_dir = app.path().app_data_dir()?;
            // A debug-only override keeps synthetic smoke tests separate from personal data.
            // The release build always uses the OS-managed private application directory.
            #[cfg(debug_assertions)]
            let app_data_dir = std::env::var_os("TASKNOTES_DEV_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(app_data_dir);
            let store = TaskStore::open(&app_data_dir.join("tasknotes.db"))?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tasks,
            create_task,
            update_task,
            change_task,
            record_progress,
            list_progress_entries
        ])
        .run(tauri::generate_context!())
        .expect("Unable to launch Tasknotes");
}
