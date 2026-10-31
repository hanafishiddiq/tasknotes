//! Task and List tools.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tasknotes_cli::contract::{self, CliError};
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, MoveTask, PatchTask, PatchTaskDetails, Priority,
    RecordProgress, RenameList, ReorderList, SetListRemoved, SortMode, TaskError, TaskQuery,
    TaskScope, TaskView, WorkStatus,
};

use crate::tools::*;

pub fn tools() -> Vec<ToolDef> {
    vec![
        tool(
            "task_list",
            Kind::Read,
            "List or search active Tasks (or Removed ones), In Progress → Ready → Completed, in pages with an opaque next_cursor.",
            task_list,
        ),
        tool(
            "task_get",
            Kind::Read,
            "Read one Task by id.",
            |c, a: Id| item(task(c.store.get_task(&a.id)?.ok_or(TaskError::NotFound)?)),
        ),
        tool(
            "task_create",
            Kind::Write,
            "Create a Task (Ready, 0%, Medium priority) in Inbox or an active List.",
            task_create,
        ),
        tool(
            "task_update",
            Kind::Write,
            "Change a Task's title and/or description; fields left out keep their value. Names the revision you last read.",
            task_update,
        ),
        tool(
            "task_change",
            Kind::Write,
            "Change a Task's work status, priority and/or progress percentage under the core's transition rules.",
            task_change,
        ),
        tool(
            "task_progress",
            Kind::Write,
            "Record a dated Progress Entry: status, percentage, Next Step and/or a note.",
            task_progress,
        ),
        tool(
            "task_details",
            Kind::Write,
            "Set or clear a Task's Tags and Deadline; fields left out keep their value.",
            task_details,
        ),
        tool(
            "task_move",
            Kind::Write,
            "Move a Task to an active List, or to Inbox.",
            task_move,
        ),
        tool(
            "task_remove",
            Kind::Write,
            "Move a Task to Removed (reversible with task_restore).",
            |c, a: Revisioned| item(task(c.store.set_task_removed(a.removed(true)?)?)),
        ),
        tool(
            "task_restore",
            Kind::Write,
            "Restore a Task from Removed.",
            |c, a: Revisioned| item(task(c.store.set_task_removed(a.removed(false)?)?)),
        ),
        tool(
            "task_history",
            Kind::Read,
            "A Task's Progress Entries, newest first, in pages of up to 50.",
            |c, a: IdPage| {
                let page =
                    c.store
                        .list_progress_entries(&a.id, limit(a.limit), a.cursor.as_deref())?;
                Ok(contract::success_page(
                    page.items.iter().map(contract::progress_entry).collect(),
                    page.next_cursor,
                ))
            },
        ),
        tool(
            "task_notes",
            Kind::Read,
            "The Notes linked to a Task.",
            |c, a: IdPage| {
                let page = c
                    .store
                    .list_task_notes(&a.id, limit(a.limit), a.cursor.as_deref())?;
                Ok(contract::success_page(
                    page.items.iter().map(contract::linked_note).collect(),
                    page.next_cursor,
                ))
            },
        ),
        tool(
            "list_list",
            Kind::Read,
            "List the Lists, newest first.",
            |c, a: ListPage| {
                let page =
                    c.store
                        .list_lists(limit(a.limit), a.cursor.as_deref(), a.include_removed)?;
                Ok(contract::success_page(
                    page.items.iter().map(contract::list).collect(),
                    page.next_cursor,
                ))
            },
        ),
        tool(
            "list_create",
            Kind::Write,
            "Create a List.",
            |c, a: Name| {
                item(contract::list(
                    &c.store.create_list(CreateList { name: a.name })?,
                ))
            },
        ),
        tool(
            "list_rename",
            Kind::Write,
            "Rename a List.",
            |c, a: Rename| {
                item(contract::list(&c.store.rename_list(RenameList {
                    id: a.id,
                    expected_revision: revision(a.revision)?,
                    name: a.name,
                })?))
            },
        ),
        tool(
            "list_reorder",
            Kind::Write,
            "Put a List directly after another List in the person's manual order, or first.",
            |c, a: Reorder| {
                let after_id = a.after()?;
                item(contract::list(&c.store.reorder_list(ReorderList {
                    expected_revision: revision(a.revision)?,
                    id: a.id,
                    after_id,
                })?))
            },
        ),
        tool(
            "list_remove",
            Kind::Write,
            "Move a List, and so the Tasks in it, to Removed (reversible with list_restore).",
            |c, a: Revisioned| {
                let expected_revision = revision(a.revision)?;
                item(contract::list(&c.store.set_list_removed(
                    SetListRemoved {
                        id: a.id,
                        expected_revision,
                        removed: true,
                    },
                )?))
            },
        ),
        tool(
            "list_restore",
            Kind::Write,
            "Restore a List from Removed.",
            |c, a: Revisioned| {
                let expected_revision = revision(a.revision)?;
                item(contract::list(&c.store.set_list_removed(
                    SetListRemoved {
                        id: a.id,
                        expected_revision,
                        removed: false,
                    },
                )?))
            },
        ),
    ]
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskListArgs {
    /// Search words: every word must match a Task's text or Progress Entries.
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    #[schemars(schema_with = "status_schema")]
    status: Option<WorkStatus>,
    #[serde(default)]
    #[schemars(schema_with = "priority_schema")]
    priority: Option<Priority>,
    #[serde(default)]
    tag: Option<String>,
    /// Only Tasks in this List.
    #[serde(default)]
    list_id: Option<String>,
    /// Only Tasks in Inbox.
    #[serde(default)]
    inbox: bool,
    /// List Tasks in Removed instead of active ones.
    #[serde(default)]
    removed: bool,
    #[serde(default)]
    #[schemars(schema_with = "sort_schema")]
    sort: Option<SortMode>,
    /// Page size, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

fn task_list(c: &Context, a: TaskListArgs) -> Result<Value, CliError> {
    if a.list_id.is_some() && a.inbox {
        return Err(CliError::invalid("Give at most one of list_id or inbox"));
    }
    let query = TaskQuery {
        text: a.text.unwrap_or_default(),
        status: a.status,
        priority: a.priority,
        tag: a.tag,
        view: match (a.list_id, a.inbox) {
            (Some(id), _) => TaskView::List(id),
            (None, true) => TaskView::Inbox,
            (None, false) => TaskView::All,
        },
        scope: if a.removed {
            TaskScope::Removed
        } else {
            TaskScope::Active
        },
        sort: a.sort.unwrap_or_default(),
        task_id: None,
    };
    let page = c
        .store
        .query_tasks(&query, limit(a.limit), a.cursor.as_deref())?;
    Ok(contract::success_page(
        page.items.iter().map(contract::task).collect(),
        page.next_cursor,
    ))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskCreateArgs {
    title: String,
    #[serde(default)]
    description: Option<String>,
    /// An active List; Inbox when left out.
    #[serde(default)]
    list_id: Option<String>,
}

fn task_create(c: &Context, a: TaskCreateArgs) -> Result<Value, CliError> {
    item(task(c.store.create_task_in_list(
        CreateTask {
            title: a.title,
            description: a.description.unwrap_or_default(),
        },
        a.list_id.as_deref(),
    )?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskUpdateArgs {
    id: String,
    revision: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

fn task_update(c: &Context, a: TaskUpdateArgs) -> Result<Value, CliError> {
    item(task(c.store.patch_task(PatchTask {
        expected_revision: revision(a.revision)?,
        id: a.id,
        title: a.title,
        description: a.description,
    })?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskChangeArgs {
    id: String,
    revision: i64,
    #[serde(default)]
    #[schemars(schema_with = "status_schema")]
    status: Option<WorkStatus>,
    #[serde(default)]
    #[schemars(schema_with = "priority_schema")]
    priority: Option<Priority>,
    /// 0–100.
    #[serde(default)]
    progress: Option<u8>,
}

fn task_change(c: &Context, a: TaskChangeArgs) -> Result<Value, CliError> {
    item(task(c.store.change_task(ChangeTask {
        expected_revision: revision(a.revision)?,
        id: a.id,
        status: a.status,
        priority: a.priority,
        progress: a.progress,
    })?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskProgressArgs {
    id: String,
    revision: i64,
    #[serde(default)]
    #[schemars(schema_with = "status_schema")]
    status: Option<WorkStatus>,
    #[serde(default)]
    progress: Option<u8>,
    #[serde(default)]
    next_step: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

fn task_progress(c: &Context, a: TaskProgressArgs) -> Result<Value, CliError> {
    item(task(c.store.record_progress(RecordProgress {
        expected_revision: revision(a.revision)?,
        id: a.id,
        status: a.status,
        progress: a.progress,
        next_step: a.next_step,
        note: a.note,
    })?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskDetailsArgs {
    id: String,
    revision: i64,
    /// The Tags to set; [] clears them; leave out to keep them.
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    #[schemars(schema_with = "nullable_date_schema")]
    deadline: Option<Option<String>>,
}

fn task_details(c: &Context, a: TaskDetailsArgs) -> Result<Value, CliError> {
    item(task(c.store.patch_task_details(PatchTaskDetails {
        expected_revision: revision(a.revision)?,
        id: a.id,
        tags: a.tags,
        due_date: a.deadline,
    })?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskMoveArgs {
    id: String,
    revision: i64,
    /// The List to move to; give this or inbox.
    #[serde(default)]
    list_id: Option<String>,
    /// Move to Inbox; give this or list_id.
    #[serde(default)]
    inbox: bool,
}

fn task_move(c: &Context, a: TaskMoveArgs) -> Result<Value, CliError> {
    let list_id = contract::destination(a.list_id, a.inbox, LIST_OR_INBOX)?;
    item(task(c.store.move_task(MoveTask {
        expected_revision: revision(a.revision)?,
        id: a.id,
        list_id,
    })?))
}
