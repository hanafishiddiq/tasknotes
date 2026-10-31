//! The versioned JSON contract AI clients read (`CONTRACT_VERSION` 1, sent as
//! `schema_version`). The MCP adapter (`tasknotes-mcp`) returns these same
//! envelopes and shapes, so both AI interfaces agree.
//!
//! Success: `{"schema_version":1,"ok":true,"item":{..}}`, or for a page
//! `{"schema_version":1,"ok":true,"items":[..],"next_cursor":null|".."}`.
//! Failure: `{"schema_version":1,"ok":false,"error":{"code":"..","message":".."}}`.
//! A refusal the core meant to audit but could not also carries
//! `"audit_recorded": false` in `error`; the refusal itself is unchanged.
//! Either envelope may carry `"warnings":[{"code":"..","message":".."}]`
//! for something that went wrong beside the command, which still ran:
//! `backup_failed` (the day's automatic backup) or `backup_retention_failed`
//! (a backup was taken but an old one could not be removed). The key is
//! absent when there is nothing to report.
//! Keys are snake_case and times are Unix milliseconds.

use serde::Serialize;
use serde_json::{Value, json};
use tasknotes_core::links::{LinkedNote, LinkedTask, NoteLink};
use tasknotes_core::note_folders::NoteFolder;
use tasknotes_core::notes::{Note, NoteSummary};
use tasknotes_core::removed::RemovedItem;
use tasknotes_core::tasks::{List, ProgressEntry, Task, TaskError, TaskStore};

/// The version of this JSON contract, sent as `schema_version` in every
/// envelope. Not the database schema (`tasknotes_core::tasks::SCHEMA_VERSION`).
pub const CONTRACT_VERSION: u32 = 1;

/// A refusal with a stable code; the message is for people and may change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub code: &'static str,
    pub message: String,
    /// The core refused the change but could not write its Audit Entry.
    pub audit_unrecorded: bool,
}

impl CliError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            audit_unrecorded: false,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }

    pub fn not_trusted() -> Self {
        Self::new(
            "not_trusted",
            "Tasknotes CLI access has not been set up for this data directory. \
             The person using Tasknotes can run `tasknotes access grant` in a terminal.",
        )
    }

    /// The process exit status for this error's code.
    pub fn exit_code(&self) -> i32 {
        match self.code {
            "invalid_input" | "invalid_cursor" => 2,
            "not_found" => 3,
            "revision_conflict" | "stale_preview" | "database_restored" => 4,
            "removed" | "name_conflict" | "invalid_transition" | "unchanged" | "desktop_only"
            | "needs_approval" | "invalid_backup" => 5,
            "not_trusted" => 6,
            "storage_unavailable" | "unsupported_schema" => 7,
            _ => 1,
        }
    }
}

impl From<TaskError> for CliError {
    fn from(error: TaskError) -> Self {
        match error {
            // Tell the person how to grant access from this interface.
            TaskError::NotAuthorized => Self::not_trusted(),
            error => Self {
                audit_unrecorded: error.audit_unrecorded(),
                ..Self::new(error.code(), error.to_string())
            },
        }
    }
}

/// The one move-target rule every adapter uses (single moves and bulk
/// moves): exactly one of a destination id or the switch naming the other
/// place (`--list ID` or `--inbox`; `list_id` or `inbox: true`). The id, or
/// `None` for the switch's place.
pub fn destination(
    id: Option<String>,
    switch: bool,
    (id_name, switch_name): (&str, &str),
) -> Result<Option<String>, CliError> {
    if id.is_some() == switch {
        return Err(CliError::invalid(format!(
            "Give exactly one of {id_name} or {switch_name}"
        )));
    }
    Ok(id)
}

pub fn success_item(item: Value) -> Value {
    json!({"schema_version": CONTRACT_VERSION, "ok": true, "item": item})
}

pub fn success_page(items: Vec<Value>, next_cursor: Option<String>) -> Value {
    json!({
        "schema_version": CONTRACT_VERSION,
        "ok": true,
        "items": items,
        "next_cursor": next_cursor,
    })
}

pub fn warning(code: &'static str, message: String) -> Value {
    json!({"code": code, "message": message})
}

/// Attach warnings to an envelope; nothing when there are none.
pub fn add_warnings(envelope: &mut Value, warnings: Vec<Value>) {
    if !warnings.is_empty() {
        envelope["warnings"] = Value::Array(warnings);
    }
}

pub fn failure(error: &CliError) -> Value {
    let mut detail = json!({"code": error.code, "message": error.message});
    if error.audit_unrecorded {
        detail["audit_recorded"] = Value::Bool(false);
    }
    json!({"schema_version": CONTRACT_VERSION, "ok": false, "error": detail})
}

pub fn task(task: &Task) -> Value {
    json!({
        "id": task.id,
        "title": task.title,
        "description": task.description,
        "status": to_value(&task.status),
        "priority": to_value(&task.priority),
        "progress": task.progress,
        "next_step": task.next_step,
        "list_id": task.list_id,
        "deadline": task.due_date,
        "tags": task.tags,
        "revision": task.revision,
        "created_at": task.created_at,
        "updated_at": task.updated_at,
        "removed": task.removed,
    })
}

pub fn list(list: &List) -> Value {
    json!({
        "id": list.id,
        "name": list.name,
        "revision": list.revision,
        "created_at": list.created_at,
        "updated_at": list.updated_at,
        "removed": list.removed,
    })
}

pub fn progress_entry(entry: &ProgressEntry) -> Value {
    json!({
        "sequence": entry.sequence,
        "id": entry.id,
        "task_id": entry.task_id,
        "previous_status": to_value(&entry.previous_status),
        "status": to_value(&entry.status),
        "previous_progress": entry.previous_progress,
        "progress": entry.progress,
        "note": entry.note,
        "next_step": entry.next_step,
        "created_at": entry.created_at,
    })
}

pub fn to_value(value: &impl Serialize) -> Value {
    serde_json::to_value(value).expect("contract values always serialize")
}

/// A Note as AI interfaces read it. Exclude from AI is never shown: an AI
/// interface only ever sees Notes that are not excluded.
pub fn note(note: &Note, folder_path: Option<String>) -> Value {
    json!({
        "id": note.id,
        "title": note.title,
        "markdown": note.markdown,
        "revision": note.revision,
        "created_at": note.created_at,
        "updated_at": note.updated_at,
        "folder_id": note.folder_id,
        "folder_path": folder_path,
        "pinned": note.pinned,
        "removed": note.removed,
    })
}

/// A listed or found Note, without its text; a search hit carries a short
/// plain-text snippet around the match.
pub fn note_summary(note: &NoteSummary, folder_path: Option<String>) -> Value {
    json!({
        "id": note.id,
        "title": note.title,
        "revision": note.revision,
        "updated_at": note.updated_at,
        "folder_id": note.folder_id,
        "folder_path": folder_path,
        "pinned": note.pinned,
        "snippet": note.snippet.as_deref().map(|s| s.replace(['\u{2}', '\u{3}'], "")),
    })
}

/// The `folder_path` an AI interface shows for a Note in `folder_id`.
pub fn folder_path_of(
    store: &TaskStore,
    folder_id: Option<&str>,
) -> Result<Option<String>, CliError> {
    Ok(folder_id.map(|id| store.note_folder_path(id)).transpose()?)
}

/// A Note with its folder path, read through `store`.
pub fn note_read(store: &TaskStore, item: &Note) -> Result<Value, CliError> {
    Ok(note(
        item,
        folder_path_of(store, item.folder_id.as_deref())?,
    ))
}

/// A listed or found Note with its folder path, read through `store`.
pub fn note_summary_read(store: &TaskStore, item: &NoteSummary) -> Result<Value, CliError> {
    Ok(note_summary(
        item,
        folder_path_of(store, item.folder_id.as_deref())?,
    ))
}

/// A folder with its path and the Notes this interface may see in it.
pub fn folder_read(store: &TaskStore, item: &NoteFolder) -> Result<Value, CliError> {
    Ok(folder(
        item,
        store.note_folder_path(&item.id)?,
        store.count_folder_notes(Some(&item.id))?,
    ))
}

/// `path` is the names from the top level down joined with `/`;
/// `note_count` counts only the Notes directly inside that this interface
/// may see.
pub fn folder(folder: &NoteFolder, path: String, note_count: i64) -> Value {
    json!({
        "id": folder.id,
        "parent_id": folder.parent_id,
        "name": folder.name,
        "path": path,
        "revision": folder.revision,
        "created_at": folder.created_at,
        "updated_at": folder.updated_at,
        "has_children": folder.has_children,
        "note_count": note_count,
        "removed": folder.removed,
    })
}

pub fn link(link: &NoteLink) -> Value {
    json!({
        "id": link.id,
        "task_id": link.task_id,
        "note_id": link.note_id,
        "linked_at": link.linked_at,
        "unlinked_at": link.unlinked_at,
    })
}

pub fn linked_note(note: &LinkedNote) -> Value {
    json!({
        "link_id": note.link_id,
        "note_id": note.note_id,
        "title": note.title,
        "updated_at": note.updated_at,
    })
}

pub fn linked_task(task: &LinkedTask) -> Value {
    json!({
        "link_id": task.link_id,
        "task_id": task.task_id,
        "title": task.title,
        "status": to_value(&task.status),
    })
}

pub fn removed_item(item: &RemovedItem) -> Value {
    json!({
        "kind": to_value(&item.kind),
        "id": item.id,
        "title": item.title,
        "revision": item.revision,
        "removed_at": item.removed_at,
        "in_removed_container": item.in_removed_container,
    })
}
