//! A Task's checklist over MCP (batch 4): the core's rules and writes, the
//! CLI's `subtask` shapes. Checklist items are Task data, never Note text,
//! so Exclude from AI does not reach them.

use schemars::JsonSchema;
use serde::Deserialize;
use tasknotes_cli::contract;
use tasknotes_core::subtasks::{AddSubtask, MoveSubtask, UpdateSubtask};

use crate::tools::{Id, Kind, Revisioned, ToolDef, item, revision, tool};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskId {
    task_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Add {
    task_id: String,
    /// 1–10,000 characters; line breaks are kept.
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Update {
    id: String,
    /// The revision you last read.
    revision: i64,
    /// New text, 1–10,000 characters.
    #[serde(default)]
    text: Option<String>,
    /// Tick (true) or untick (false) the item.
    #[serde(default)]
    done: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Move {
    id: String,
    /// The revision you last read.
    revision: i64,
    /// The new place among the shown items, 0 first; past the end is last.
    index: u32,
}

pub fn tools() -> Vec<ToolDef> {
    vec![
        tool(
            "subtask_list",
            Kind::Read,
            "A Task's checklist (subtasks) in order, Removed items left out. Tasks also carry subtasks_done and subtasks_total; the Task's progress stays the person's own and does not follow the checklist.",
            |c, a: TaskId| {
                let items = c.store.list_subtasks(&a.task_id)?;
                Ok(contract::success_page(
                    items.iter().map(contract::subtask).collect(),
                    None,
                ))
            },
        ),
        tool(
            "subtask_add",
            Kind::Write,
            "Add an item at the end of a Task's checklist (at most 500 per Task).",
            |c, a: Add| {
                item(contract::subtask(&c.store.add_subtask(AddSubtask {
                    task_id: a.task_id,
                    text: a.text,
                })?))
            },
        ),
        tool(
            "subtask_update",
            Kind::Write,
            "Change a checklist item's text, tick or untick it (done), or both.",
            |c, a: Update| {
                item(contract::subtask(&c.store.update_subtask(
                    UpdateSubtask {
                        id: a.id,
                        expected_revision: revision(a.revision)?,
                        text: a.text,
                        done: a.done,
                    },
                )?))
            },
        ),
        tool(
            "subtask_move",
            Kind::Write,
            "Move a checklist item to another place in its Task's checklist.",
            |c, a: Move| {
                item(contract::subtask(&c.store.move_subtask(MoveSubtask {
                    id: a.id,
                    expected_revision: revision(a.revision)?,
                    index: a.index,
                })?))
            },
        ),
        tool(
            "subtask_remove",
            Kind::Write,
            "Remove an item from a Task's checklist (reversible with subtask_restore; nothing is deleted).",
            |c, a: Revisioned| {
                item(contract::subtask(
                    &c.store.remove_subtask(&a.id, revision(a.revision)?)?,
                ))
            },
        ),
        tool(
            "subtask_restore",
            Kind::Write,
            "Bring a removed checklist item back, in the place it had.",
            |c, a: Id| item(contract::subtask(&c.store.restore_subtask(&a.id)?)),
        ),
    ]
}
