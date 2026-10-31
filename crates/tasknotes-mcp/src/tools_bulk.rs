//! Bulk Changes over MCP: `bulk_preview` asks for one and `bulk_status`
//! reads its outcome. Both are the CLI's flow (`tasknotes_cli::bulk`) over
//! the core; there is no tool that approves. The person approves or
//! declines in the Tasknotes app (owner decision A).

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tasknotes_cli::bulk::{self, BulkArgs};

use crate::tools::{Context, Id, LIST_OR_INBOX, ToolDef, tool_def};

/// `BulkArgs` with a flat object schema: one `action` enum and the union of
/// every action's fields. Clients (and the MCP specification) want an
/// object at the root, and several reject a root `oneOf`; the per-action
/// rules are still enforced when the arguments are read.
#[derive(Deserialize)]
#[serde(transparent)]
struct PreviewArgs(BulkArgs);

impl JsonSchema for PreviewArgs {
    fn schema_name() -> Cow<'static, str> {
        "BulkPreviewArgs".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        flatten(BulkArgs::json_schema(generator))
    }
}

/// Merge a serde-tagged enum's `oneOf` branches into one object schema.
fn flatten(tagged: Schema) -> Schema {
    let branches = tagged
        .get("oneOf")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut actions = Vec::new();
    let mut properties = Map::new();
    let mut required: Option<Vec<Value>> = None;
    for branch in &branches {
        let Some(fields) = branch.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, schema) in fields {
            if name == "action" {
                actions.extend(schema.get("const").cloned());
                if let Some(names) = schema.get("enum").and_then(Value::as_array) {
                    actions.extend(names.iter().cloned());
                }
                continue;
            }
            properties
                .entry(name.clone())
                .or_insert_with(|| schema.clone());
        }
        let here: Vec<Value> = branch
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        required = Some(match required {
            None => here,
            Some(all) => all.into_iter().filter(|name| here.contains(name)).collect(),
        });
    }
    properties.insert(
        "action".into(),
        json!({"type": "string", "enum": actions, "description": "What to do to every id."}),
    );
    if let Some(ids) = properties.get_mut("ids") {
        ids["description"] = json!(
            "1–100 distinct Task ids, Note ids for the *_notes actions, \
             or exactly one List or Note Folder id for remove_list / remove_folder."
        );
    }
    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "required": required.unwrap_or_default(),
        "additionalProperties": false,
    });
    if let Some(defs) = tagged.get("$defs") {
        schema["$defs"] = defs.clone();
    }
    Schema::try_from(schema).unwrap_or_default()
}

pub fn tools() -> Vec<ToolDef> {
    vec![
        tool_def(
            "bulk_preview",
            true,
            "Ask to change several Tasks or Notes at once (remove, restore, move, set status), \
             or to remove a List or Note Folder that holds anything. Nothing changes yet: \
             returns a pending request (and, except for a List or folder, each item before \
             and after). The person approves or declines it in the Tasknotes app; you cannot. \
             Check the outcome with bulk_status. Single items have their own tools.",
            |c: &Context, a: PreviewArgs| bulk::preview(c.store, a.0, LIST_OR_INBOX),
        ),
        tool_def(
            "bulk_status",
            false,
            "The status of a bulk_preview request: pending (waiting for the person in the \
             Tasknotes app), applied, declined, stale (something changed first; nothing was \
             changed) or expired (nobody decided within a day).",
            |c: &Context, a: Id| bulk::status(c.store, &a.id),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tasknotes_cli::contract::CliError;

    #[test]
    fn the_preview_schema_is_one_flat_object_naming_every_action() {
        let schema = schemars::schema_for!(PreviewArgs);
        let value = serde_json::to_value(&schema).unwrap();
        assert_eq!(value["type"], "object", "{value}");
        assert!(value.get("oneOf").is_none());
        assert_eq!(value["additionalProperties"], false);
        let actions: Vec<&str> = value["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        assert_eq!(
            actions,
            [
                "remove_tasks",
                "restore_tasks",
                "move_tasks",
                "set_task_status",
                "remove_notes",
                "restore_notes",
                "remove_list",
                "remove_folder"
            ]
        );
        // Each name is one the core reads.
        for action in actions {
            let args: Result<BulkArgs, CliError> = serde_json::from_value::<BulkArgs>(
                json!({"action": action, "ids": ["x"], "status": "completed"}),
            )
            .or_else(|_| serde_json::from_value(json!({"action": action, "ids": ["x"]})))
            .map_err(|e| CliError::invalid(e.to_string()));
            assert!(args.is_ok(), "{action}: {args:?}");
        }
        let mut required: Vec<&str> = value["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        required.sort();
        assert_eq!(required, ["action", "ids"]);
        for field in ["list_id", "inbox", "status"] {
            assert!(value["properties"].get(field).is_some(), "{field}");
        }
    }
}
