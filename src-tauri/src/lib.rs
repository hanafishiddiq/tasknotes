use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, ListPage, MoveTask, ProgressHistoryPage,
    RecordProgress, RenameList, SetListRemoved, SortMode, Task, TaskPage, TaskStore, TaskView,
    UpdateTask, UpdateTaskDetails,
};
use tauri::{Manager, State};

#[tauri::command]
fn list_tasks(
    store: State<'_, TaskStore>,
    limit: u32,
    cursor: Option<String>,
    sort: Option<SortMode>,
    view: Option<TaskView>,
) -> Result<TaskPage, String> {
    store
        .list_tasks_in_view(
            limit,
            cursor.as_deref(),
            sort.unwrap_or_default(),
            view.unwrap_or(TaskView::All),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_task(
    store: State<'_, TaskStore>,
    input: CreateTask,
    list_id: Option<String>,
) -> Result<Task, String> {
    store
        .create_task_in_list(input, list_id.as_deref())
        .map_err(|error| error.to_string())
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

#[tauri::command]
fn list_lists(
    store: State<'_, TaskStore>,
    limit: u32,
    cursor: Option<String>,
    include_removed: Option<bool>,
) -> Result<ListPage, String> {
    store
        .list_lists(limit, cursor.as_deref(), include_removed.unwrap_or(false))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_list(store: State<'_, TaskStore>, input: CreateList) -> Result<List, String> {
    store.create_list(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_list(store: State<'_, TaskStore>, input: RenameList) -> Result<List, String> {
    store.rename_list(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_list_removed(store: State<'_, TaskStore>, input: SetListRemoved) -> Result<List, String> {
    store
        .set_list_removed(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn move_task(store: State<'_, TaskStore>, input: MoveTask) -> Result<Task, String> {
    store.move_task(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn update_task_details(
    store: State<'_, TaskStore>,
    input: UpdateTaskDetails,
) -> Result<Task, String> {
    store
        .update_task_details(input)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Data is private to this macOS user, never kept in the public source repository.
            let app_data_dir = app.path().app_data_dir()?;
            // A debug-only override keeps synthetic tests separate from personal data.
            // Release always uses the OS-managed private application directory.
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
            list_progress_entries,
            list_lists,
            create_list,
            rename_list,
            set_list_removed,
            move_task,
            update_task_details
        ])
        .run(tauri::generate_context!())
        .expect("Unable to launch Tasknotes");
}
