//! Bulk Change requests, shared by both AI interfaces: `tasknotes bulk
//! preview|status` and the MCP tools `bulk_preview` / `bulk_status` take the
//! same arguments (`BulkArgs`) and answer with the same shapes. The rules
//! (selection, stale guard, expiry, privacy, who may approve) are the
//! core's (`tasknotes_core::bulk`); an AI can never approve its own request.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tasknotes_core::bulk::{BulkAction, BulkItem, BulkRequest, BulkRequestStatus};
use tasknotes_core::notes::Note;
use tasknotes_core::tasks::{TaskStore, WorkStatus};

use crate::args::{Spec, parse};
use crate::contract::{self, CliError};
use crate::{ONE_ID, USAGE};

/// What to ask for, by action. The action names are the core's.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum BulkArgs {
    RemoveTasks {
        /// 1–100 distinct ids.
        ids: Vec<String>,
    },
    RestoreTasks {
        ids: Vec<String>,
    },
    MoveTasks {
        ids: Vec<String>,
        /// move_tasks: the active List to move to; give this or inbox.
        #[serde(default)]
        list_id: Option<String>,
        /// move_tasks: move to Inbox instead.
        #[serde(default)]
        inbox: bool,
    },
    SetTaskStatus {
        ids: Vec<String>,
        /// set_task_status: the status to set.
        #[schemars(schema_with = "status_schema")]
        status: WorkStatus,
    },
    RemoveNotes {
        ids: Vec<String>,
    },
    RestoreNotes {
        ids: Vec<String>,
    },
    /// Exactly one List id.
    RemoveList {
        ids: Vec<String>,
    },
    /// Exactly one Note Folder id.
    RemoveFolder {
        ids: Vec<String>,
    },
}

fn status_schema(_: &mut SchemaGenerator) -> Schema {
    let names: Vec<Value> = WorkStatus::ALL.iter().map(contract::to_value).collect();
    schemars::json_schema!({"type": "string", "enum": names})
}

impl BulkArgs {
    /// The core's action and ids; `targets` names the move-target switches
    /// as this interface spells them.
    pub fn into_core(self, targets: (&str, &str)) -> Result<(BulkAction, Vec<String>), CliError> {
        Ok(match self {
            Self::RemoveTasks { ids } => (BulkAction::RemoveTasks, ids),
            Self::RestoreTasks { ids } => (BulkAction::RestoreTasks, ids),
            Self::MoveTasks {
                ids,
                list_id,
                inbox,
            } => (
                BulkAction::MoveTasks {
                    list_id: contract::destination(list_id, inbox, targets)?,
                },
                ids,
            ),
            Self::SetTaskStatus { ids, status } => (BulkAction::SetTaskStatus { status }, ids),
            Self::RemoveNotes { ids } => (BulkAction::RemoveNotes, ids),
            Self::RestoreNotes { ids } => (BulkAction::RestoreNotes, ids),
            Self::RemoveList { ids } => (BulkAction::RemoveList, ids),
            Self::RemoveFolder { ids } => (BulkAction::RemoveFolder, ids),
        })
    }
}

/// Ask for a Bulk Change; the answer is the pending request and what it
/// would do (nothing about what a List or folder holds).
pub fn preview(
    store: &TaskStore,
    args: BulkArgs,
    targets: (&str, &str),
) -> Result<Value, CliError> {
    let (action, ids) = args.into_core(targets)?;
    let requested = store.request_bulk(action, ids)?;
    let mut answer = request(&requested.request);
    if requested.request.count.is_some() {
        answer["items"] = Value::Array(items(store, &requested.items)?);
    }
    Ok(contract::success_item(answer))
}

/// A request's status.
pub fn status(store: &TaskStore, id: &str) -> Result<Value, CliError> {
    Ok(contract::success_item(request(&store.bulk_request(id)?)))
}

/// A request as AI interfaces read it: its action (with its List or status),
/// status, times and, except for a List or folder removal, how many items.
pub fn request(request: &BulkRequest) -> Value {
    let mut fields = match contract::to_value(&request.action) {
        Value::Object(fields) => fields,
        _ => Map::new(),
    };
    fields.insert("request_id".into(), request.id.clone().into());
    fields.insert("status".into(), contract::to_value(&request.status));
    fields.insert("message".into(), status_message(request.status).into());
    if let Some(count) = request.count {
        fields.insert("count".into(), count.into());
    }
    fields.insert(
        "requested_by".into(),
        contract::to_value(&request.requested_by),
    );
    fields.insert("created_at".into(), request.created_at.into());
    fields.insert("expires_at".into(), request.expires_at.into());
    fields.insert("decided_at".into(), json!(request.decided_at));
    Value::Object(fields)
}

fn status_message(status: BulkRequestStatus) -> &'static str {
    match status {
        BulkRequestStatus::Pending => {
            "Waiting for the person to approve or decline it in the Tasknotes app; \
             ask for its status again later"
        }
        BulkRequestStatus::Applied => "Approved in Tasknotes and applied",
        BulkRequestStatus::Declined => "Declined in Tasknotes; nothing was changed",
        BulkRequestStatus::Stale => {
            "Something in it changed before it was approved; nothing was changed. Ask again"
        }
        BulkRequestStatus::Expired => "Nobody decided within a day; nothing was changed",
    }
}

fn items(store: &TaskStore, items: &[BulkItem]) -> Result<Vec<Value>, CliError> {
    items
        .iter()
        .map(|change| {
            Ok(match change {
                BulkItem::Task { before, after } => {
                    json!({"before": contract::task(before), "after": contract::task(after)})
                }
                BulkItem::Note { before, after } => {
                    json!({"before": brief(store, before)?, "after": brief(store, after)?})
                }
            })
        })
        .collect()
}

/// A Note in a preview: everything but its text, so 100 Notes stay small.
fn brief(store: &TaskStore, note: &Note) -> Result<Value, CliError> {
    let mut value = contract::note_read(store, note)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("markdown");
    }
    Ok(value)
}

/// `tasknotes bulk preview --action A --id ID [--id ID ...] [--list ID |
/// --inbox] [--status S]` and `tasknotes bulk status REQUEST_ID`.
pub(crate) fn bulk_command(
    store: &TaskStore,
    verb: &str,
    words: &[String],
) -> Result<Value, CliError> {
    match verb {
        "status" => {
            let p = parse(words, &ONE_ID)?;
            status(store, p.id())
        }
        "preview" => {
            let p = parse(
                words,
                &Spec {
                    values: &["action", "list", "status"],
                    repeated: &["id"],
                    switches: &["inbox"],
                    positionals: 0,
                },
            )?;
            // The same arguments the MCP tool takes, so both interfaces
            // refuse the same mistakes.
            let mut fields = Map::new();
            fields.insert("action".into(), p.required("action")?.into());
            fields.insert("ids".into(), p.all("id").into());
            if let Some(list) = p.value("list") {
                fields.insert("list_id".into(), list.into());
            }
            if p.has("inbox") {
                fields.insert("inbox".into(), true.into());
            }
            if let Some(status) = p.value("status") {
                fields.insert("status".into(), status.into());
            }
            let args: BulkArgs = serde_json::from_value(Value::Object(fields))
                .map_err(|e| CliError::invalid(format!("Invalid bulk preview: {e}")))?;
            preview(store, args, ("--list ID", "--inbox"))
        }
        _ => Err(CliError::invalid(USAGE)),
    }
}
