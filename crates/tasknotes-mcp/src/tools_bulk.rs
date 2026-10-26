//! Bulk changes: `bulk_preview` asks the core for a preview and keeps its
//! plan behind a random, single-use token for this session; `bulk_confirm`
//! hands the plan back to the core, which applies it all or nothing or
//! refuses it as stale. The selection limits, the stale-preview guard and
//! the expiry are the core's (`tasknotes_core::bulk`).

use std::sync::Mutex;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::Deserialize;
use serde_json::{Value, json};
use tasknotes_cli::contract::{self, CliError};
use tasknotes_core::bulk::{BulkAction, BulkItem, BulkPlan};
use tasknotes_core::notes::Note;
use tasknotes_core::tasks::{TaskStore, WorkStatus};

use crate::tools::{Context, ToolDef, item, names_schema, task, tool_def};

/// How many unconfirmed previews one session keeps; the oldest goes first.
const KEPT_PREVIEWS: usize = 16;

/// This session's unconfirmed previews, by token.
#[derive(Default)]
pub struct Previews {
    plans: Mutex<Vec<(String, BulkPlan)>>,
}

impl Previews {
    fn keep(&self, plan: BulkPlan) -> String {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let mut plans = self.plans.lock().unwrap_or_else(|e| e.into_inner());
        if plans.len() >= KEPT_PREVIEWS {
            plans.remove(0);
        }
        plans.push((token.clone(), plan));
        token
    }

    /// The plan for `token`, which is spent whatever happens next.
    fn take(&self, token: &str) -> Option<BulkPlan> {
        let mut plans = self.plans.lock().unwrap_or_else(|e| e.into_inner());
        let index = plans.iter().position(|(t, _)| t == token)?;
        Some(plans.remove(index).1)
    }
}

const ACTIONS: [&str; 6] = [
    "remove_tasks",
    "restore_tasks",
    "move_tasks",
    "set_task_status",
    "remove_notes",
    "restore_notes",
];

fn action_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({"type": "string", "enum": ACTIONS})
}

fn status_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&WorkStatus::ALL)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreviewArgs {
    /// What to do to every item.
    #[schemars(schema_with = "action_schema")]
    action: String,
    /// 1–100 distinct Task ids (Note ids for the *_notes actions).
    ids: Vec<String>,
    /// move_tasks: the active List to move to; give this or inbox.
    #[serde(default)]
    list_id: Option<String>,
    /// move_tasks: move to Inbox instead.
    #[serde(default)]
    inbox: bool,
    /// set_task_status: the status to set.
    #[serde(default)]
    #[schemars(schema_with = "status_schema")]
    status: Option<WorkStatus>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConfirmArgs {
    /// The token from bulk_preview.
    token: String,
}

pub fn tools() -> Vec<ToolDef> {
    vec![
        tool_def(
            "bulk_preview",
            false,
            "Preview a change to several Tasks or Notes at once (remove, restore, move, \
             set status). Changes nothing: returns each item before and after and a token. \
             Show the preview to the person, then call bulk_confirm with the token \
             within 10 minutes. Single items have their own tools.",
            preview,
        ),
        tool_def(
            "bulk_confirm",
            true,
            "Apply a bulk_preview, all or nothing. Refused as stale_preview if anything \
             in it changed since, if it expired, or if the token was already used; \
             then preview again.",
            confirm,
        ),
    ]
}

fn bulk_action(a: &PreviewArgs) -> Result<BulkAction, CliError> {
    let wrong = |what: &str| Err(CliError::invalid(what.to_owned()));
    let targets = a.list_id.is_some() || a.inbox;
    if a.action != "move_tasks" && targets {
        return wrong("list_id and inbox are only for move_tasks");
    }
    if a.action != "set_task_status" && a.status.is_some() {
        return wrong("status is only for set_task_status");
    }
    Ok(match a.action.as_str() {
        "remove_tasks" => BulkAction::RemoveTasks,
        "restore_tasks" => BulkAction::RestoreTasks,
        "move_tasks" if a.list_id.is_some() != a.inbox => BulkAction::MoveTasks {
            list_id: a.list_id.clone(),
        },
        "move_tasks" => return wrong("move_tasks needs exactly one of list_id or inbox: true"),
        "set_task_status" => match a.status {
            Some(status) => BulkAction::SetTaskStatus { status },
            None => return wrong("set_task_status needs a status"),
        },
        "remove_notes" => BulkAction::RemoveNotes,
        "restore_notes" => BulkAction::RestoreNotes,
        other => {
            return Err(CliError::invalid(format!(
                "Unknown bulk action {other:?}; use one of: {}",
                ACTIONS.join(", ")
            )));
        }
    })
}

fn action_name(action: &BulkAction) -> &'static str {
    match action {
        BulkAction::RemoveTasks => "remove_tasks",
        BulkAction::RestoreTasks => "restore_tasks",
        BulkAction::MoveTasks { .. } => "move_tasks",
        BulkAction::SetTaskStatus { .. } => "set_task_status",
        BulkAction::RemoveNotes => "remove_notes",
        BulkAction::RestoreNotes => "restore_notes",
    }
}

fn preview(c: &Context, a: PreviewArgs) -> Result<Value, CliError> {
    let action = bulk_action(&a)?;
    let preview = c.store.preview_bulk(action, a.ids)?;
    let items = changes(c.store, &preview.items)?;
    let plan = preview.plan;
    item(json!({
        "action": action_name(plan.action()),
        "count": plan.len(),
        "expires_at": plan.expires_at(),
        "items": items,
        "token": c.previews.keep(plan),
    }))
}

fn confirm(c: &Context, a: ConfirmArgs) -> Result<Value, CliError> {
    let plan = c.previews.take(&a.token).ok_or_else(|| {
        CliError::new(
            "stale_preview",
            "This preview is unknown, already used or from another session; \
             nothing was changed. Preview again",
        )
    })?;
    let applied = c.store.apply_bulk(&plan)?;
    item(json!({
        "action": action_name(plan.action()),
        "count": applied.len(),
        "items": changes(c.store, &applied)?,
    }))
}

fn changes(store: &TaskStore, items: &[BulkItem]) -> Result<Vec<Value>, CliError> {
    items
        .iter()
        .map(|change| {
            Ok(match change {
                BulkItem::Task { before, after } => {
                    json!({"before": task(before.clone()), "after": task(after.clone())})
                }
                BulkItem::Note { before, after } => {
                    json!({"before": brief(store, before)?, "after": brief(store, after)?})
                }
            })
        })
        .collect()
}

/// A Note in a preview: everything but its text, so a preview of 100 Notes
/// stays small.
fn brief(store: &TaskStore, note: &Note) -> Result<Value, CliError> {
    let mut value = contract::note_read(store, note)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("markdown");
    }
    Ok(value)
}
