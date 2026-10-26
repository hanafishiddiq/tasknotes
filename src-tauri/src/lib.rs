use tasknotes_core::links::{LinkPage, LinkedNote, LinkedTask, NoteLink};
use tasknotes_core::note_folders::{
    CreateNoteFolder, MoveNoteFolder, NoteFolder, NoteFolderPage, RenameNoteFolder,
};
use tasknotes_core::notes::{
    CreateNote, MoveNote, Note, NotePage, NoteQuery, NoteScope, SetNotePinned, UpdateNote,
};
use tasknotes_core::removed::{RemovedPage, SetRemoved};
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, ListPage, MoveTask, Priority, ProgressSequencePage,
    RecordProgress, RenameList, SetListRemoved, SortMode, Task, TaskPage, TaskQuery, TaskStore,
    TaskView, UpdateTask, UpdateTaskDetails, WorkStatus,
};
use tauri::{Manager, State};

#[cfg(desktop)]
mod quick_capture;
mod quit;
#[cfg(desktop)]
mod shell;
#[cfg(desktop)]
mod shell_settings;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn list_tasks(
    store: State<'_, TaskStore>,
    limit: u32,
    cursor: Option<String>,
    sort: Option<SortMode>,
    view: Option<TaskView>,
    text: Option<String>,
    status: Option<WorkStatus>,
    priority: Option<Priority>,
    tag: Option<String>,
    task_id: Option<String>,
) -> Result<TaskPage, String> {
    let query = TaskQuery {
        text: text.unwrap_or_default(),
        status,
        priority,
        tag,
        view: view.unwrap_or_default(),
        sort: sort.unwrap_or_default(),
        task_id,
        ..TaskQuery::default()
    };
    store
        .query_tasks(&query, limit, cursor.as_deref())
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
) -> Result<ProgressSequencePage, String> {
    store
        .list_progress_entries_before(&task_id, limit, before)
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

#[tauri::command]
fn list_notes(
    store: State<'_, TaskStore>,
    limit: u32,
    cursor: Option<String>,
    folder: Option<NoteScope>,
    text: Option<String>,
) -> Result<NotePage, String> {
    let query = NoteQuery {
        text: text.unwrap_or_default(),
        folder: folder.unwrap_or_default(),
    };
    store
        .query_notes(&query, limit, cursor.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_note(store: State<'_, TaskStore>, id: String) -> Result<Option<Note>, String> {
    store.get_note(&id).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_note(store: State<'_, TaskStore>, input: CreateNote) -> Result<Note, String> {
    store.create_note(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn move_note(store: State<'_, TaskStore>, input: MoveNote) -> Result<Note, String> {
    store.move_note(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_note_pinned(store: State<'_, TaskStore>, input: SetNotePinned) -> Result<Note, String> {
    store
        .set_note_pinned(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_note_folders(
    store: State<'_, TaskStore>,
    parent_id: Option<String>,
    limit: u32,
    cursor: Option<String>,
) -> Result<NoteFolderPage, String> {
    store
        .list_note_folders(parent_id.as_deref(), limit, cursor.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_note_folder(
    store: State<'_, TaskStore>,
    input: CreateNoteFolder,
) -> Result<NoteFolder, String> {
    store
        .create_note_folder(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_note_folder(
    store: State<'_, TaskStore>,
    input: RenameNoteFolder,
) -> Result<NoteFolder, String> {
    store
        .rename_note_folder(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn move_note_folder(
    store: State<'_, TaskStore>,
    input: MoveNoteFolder,
) -> Result<NoteFolder, String> {
    store
        .move_note_folder(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn update_note(store: State<'_, TaskStore>, input: UpdateNote) -> Result<Note, String> {
    store.update_note(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn link_note(
    store: State<'_, TaskStore>,
    task_id: String,
    note_id: String,
) -> Result<NoteLink, String> {
    store
        .link_note(&task_id, &note_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn unlink_note(store: State<'_, TaskStore>, link_id: String) -> Result<NoteLink, String> {
    store
        .unlink_note(&link_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_link(store: State<'_, TaskStore>, link_id: String) -> Result<NoteLink, String> {
    store
        .restore_link(&link_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_task_notes(
    store: State<'_, TaskStore>,
    task_id: String,
    limit: u32,
    cursor: Option<String>,
) -> Result<LinkPage<LinkedNote>, String> {
    store
        .list_task_notes(&task_id, limit, cursor.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_note_tasks(
    store: State<'_, TaskStore>,
    note_id: String,
    limit: u32,
    cursor: Option<String>,
) -> Result<LinkPage<LinkedTask>, String> {
    store
        .list_note_tasks(&note_id, limit, cursor.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_task_removed(store: State<'_, TaskStore>, input: SetRemoved) -> Result<Task, String> {
    store
        .set_task_removed(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_note_removed(store: State<'_, TaskStore>, input: SetRemoved) -> Result<Note, String> {
    store
        .set_note_removed(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_note_folder_removed(
    store: State<'_, TaskStore>,
    input: SetRemoved,
) -> Result<NoteFolder, String> {
    store
        .set_note_folder_removed(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_removed(
    store: State<'_, TaskStore>,
    text: String,
    limit: u32,
    cursor: Option<String>,
) -> Result<RemovedPage, String> {
    store
        .list_removed(&text, limit, cursor.as_deref())
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(shell::shortcut_plugin());
    builder
        .manage(quit::QuitState::default())
        .menu(quit::menu)
        .on_menu_event(quit::on_menu_event)
        .on_window_event(|window, event| {
            // With menu-bar mode on, closing hides the window instead of quitting.
            #[cfg(desktop)]
            if shell::on_window_event(window, event) {
                return;
            }
            quit::on_window_event(window, event);
        })
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
            #[cfg(desktop)]
            shell::start(app.handle(), app_data_dir);
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
            update_task_details,
            list_notes,
            get_note,
            create_note,
            update_note,
            move_note,
            set_note_pinned,
            list_note_folders,
            create_note_folder,
            rename_note_folder,
            move_note_folder,
            link_note,
            unlink_note,
            restore_link,
            list_task_notes,
            list_note_tasks,
            set_task_removed,
            set_note_removed,
            set_note_folder_removed,
            list_removed,
            quit::quit_ready,
            #[cfg(desktop)]
            shell::get_shell_settings,
            #[cfg(desktop)]
            shell::set_keep_in_menu_bar,
            #[cfg(desktop)]
            shell::set_quick_capture
        ])
        .build(tauri::generate_context!())
        .expect("Unable to launch Tasknotes")
        .run(|_app, _event| {
            // Clicking the Dock icon brings back a window hidden to the menu bar.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = _event
            {
                shell::show_main_window(_app);
            }
        });
}
