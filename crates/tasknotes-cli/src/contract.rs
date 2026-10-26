//! The versioned JSON contract AI clients read (`CONTRACT_VERSION` 1, sent as
//! `schema_version`). Ticket 12's
//! MCP adapter is meant to reuse these shapes so both interfaces agree.
//!
//! Success: `{"schema_version":1,"ok":true,"item":{..}}`, or for a page
//! `{"schema_version":1,"ok":true,"items":[..],"next_cursor":null|".."}`.
//! Failure: `{"schema_version":1,"ok":false,"error":{"code":"..","message":".."}}`.
//! A refusal the core meant to audit but could not also carries
//! `"audit_recorded": false` in `error`; the refusal itself is unchanged.
//! Keys are snake_case and times are Unix milliseconds.

use serde::Serialize;
use serde_json::{Value, json};
use tasknotes_core::tasks::{List, ProgressEntry, Task, TaskError};

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
            "revision_conflict" => 4,
            "removed" | "name_conflict" | "invalid_transition" | "unchanged" => 5,
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
