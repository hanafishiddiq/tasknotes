//! The MCP tools: each one reads its arguments, calls one core operation on
//! a store opened as `Interface::Mcp`, and shapes the result with the CLI's
//! contract. Validation, revisions, Removed, Exclude from AI and Audit
//! Entries are all the core's; nothing here decides them.

use std::sync::Arc;

use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use tasknotes_cli::contract::{self, CliError};
use tasknotes_core::note_folders::{CreateNoteFolder, MoveNoteFolder, RenameNoteFolder};
use tasknotes_core::notes::{CreateNote, MoveNote, NoteQuery, NoteScope, UpdateNote};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, MoveTask, PatchTask, PatchTaskDetails, Priority,
    RecordProgress, RenameList, SetListRemoved, SortMode, Task, TaskError, TaskQuery, TaskScope,
    TaskStore, TaskView, WorkStatus,
};

pub use crate::tools_bulk::Previews;

/// What a tool runs against.
pub struct Context<'a> {
    pub store: &'a TaskStore,
    pub previews: &'a Previews,
}

type Handler = Box<dyn Fn(&Context, Value) -> Result<Value, CliError> + Send + Sync>;

pub struct ToolDef {
    pub tool: Tool,
    handler: Handler,
}

impl ToolDef {
    pub fn call(&self, context: &Context, arguments: Value) -> Result<Value, CliError> {
        (self.handler)(context, arguments)
    }
}

#[derive(Clone, Copy)]
enum Kind {
    /// Reads only.
    Read,
    /// Changes data, reversibly (nothing is ever deleted).
    Write,
}

fn tool<A: DeserializeOwned + JsonSchema + 'static>(
    name: &'static str,
    kind: Kind,
    description: &'static str,
    run: fn(&Context, A) -> Result<Value, CliError>,
) -> ToolDef {
    let annotations = match kind {
        Kind::Read => ToolAnnotations::new().read_only(true),
        Kind::Write => ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(false),
    }
    .open_world(false);
    ToolDef {
        tool: Tool::new(name, description, Arc::new(JsonObject::new()))
            .with_input_schema::<A>()
            .with_annotations(annotations),
        handler: Box::new(move |context, arguments| {
            let arguments: A = serde_json::from_value(arguments)
                .map_err(|e| CliError::invalid(format!("Invalid arguments for {name}: {e}")))?;
            run(context, arguments)
        }),
    }
}

pub fn all() -> Vec<ToolDef> {
    let mut tools = vec![
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
        tool(
            "note_list",
            Kind::Read,
            "List Notes, or search their text, optionally in one folder; summaries without the full text.",
            note_list,
        ),
        tool(
            "note_get",
            Kind::Read,
            "Read one Note, with its Markdown.",
            |c, a: Id| {
                item(contract::note_read(
                    c.store,
                    &c.store.get_note(&a.id)?.ok_or(TaskError::NoteNotFound)?,
                )?)
            },
        ),
        tool(
            "note_create",
            Kind::Write,
            "Create a Note from Markdown; its first line is the title.",
            |c, a: NoteCreate| {
                item(contract::note_read(
                    c.store,
                    &c.store.create_note(CreateNote {
                        markdown: a.markdown,
                        folder_id: a.folder_id,
                    })?,
                )?)
            },
        ),
        tool(
            "note_update",
            Kind::Write,
            "Replace a Note's Markdown. Names the revision you last read.",
            |c, a: NoteUpdate| {
                let expected_revision = revision(a.revision)?;
                item(contract::note_read(
                    c.store,
                    &c.store.update_note(UpdateNote {
                        id: a.id,
                        expected_revision,
                        markdown: a.markdown,
                    })?,
                )?)
            },
        ),
        tool(
            "note_move",
            Kind::Write,
            "Move a Note into a folder, or out of every folder.",
            note_move,
        ),
        tool(
            "note_remove",
            Kind::Write,
            "Move a Note to Removed (reversible with note_restore).",
            |c, a: Revisioned| {
                item(contract::note_read(
                    c.store,
                    &c.store.set_note_removed(a.removed(true)?)?,
                )?)
            },
        ),
        tool(
            "note_restore",
            Kind::Write,
            "Restore a Note from Removed.",
            |c, a: Revisioned| {
                item(contract::note_read(
                    c.store,
                    &c.store.set_note_removed(a.removed(false)?)?,
                )?)
            },
        ),
        tool(
            "note_tasks",
            Kind::Read,
            "The Tasks linked to a Note.",
            |c, a: IdPage| {
                let page = c
                    .store
                    .list_note_tasks(&a.id, limit(a.limit), a.cursor.as_deref())?;
                Ok(contract::success_page(
                    page.items.iter().map(contract::linked_task).collect(),
                    page.next_cursor,
                ))
            },
        ),
        tool(
            "folder_list",
            Kind::Read,
            "List the Note Folders at the top level or inside one folder.",
            |c, a: FolderPage| {
                let page = c.store.list_note_folders(
                    a.parent_id.as_deref(),
                    limit(a.limit),
                    a.cursor.as_deref(),
                )?;
                let items = page
                    .items
                    .iter()
                    .map(|f| contract::folder_read(c.store, f))
                    .collect::<Result<_, _>>()?;
                Ok(contract::success_page(items, page.next_cursor))
            },
        ),
        tool(
            "folder_get",
            Kind::Read,
            "Read one Note Folder.",
            |c, a: Id| {
                folder(
                    c,
                    c.store
                        .get_note_folder(&a.id)?
                        .ok_or(TaskError::NoteFolderNotFound)?,
                )
            },
        ),
        tool(
            "folder_create",
            Kind::Write,
            "Create a Note Folder at the top level or inside another.",
            |c, a: FolderCreate| {
                folder(
                    c,
                    c.store.create_note_folder(CreateNoteFolder {
                        name: a.name,
                        parent_id: a.parent_id,
                    })?,
                )
            },
        ),
        tool(
            "folder_rename",
            Kind::Write,
            "Rename a Note Folder.",
            |c, a: Rename| {
                let expected_revision = revision(a.revision)?;
                folder(
                    c,
                    c.store.rename_note_folder(RenameNoteFolder {
                        id: a.id,
                        expected_revision,
                        name: a.name,
                    })?,
                )
            },
        ),
        tool(
            "folder_move",
            Kind::Write,
            "Move a Note Folder into another folder, or to the top level.",
            folder_move,
        ),
        tool(
            "folder_remove",
            Kind::Write,
            "Move a Note Folder, and what is in it, to Removed (reversible).",
            |c, a: Revisioned| folder(c, c.store.set_note_folder_removed(a.removed(true)?)?),
        ),
        tool(
            "folder_restore",
            Kind::Write,
            "Restore a Note Folder from Removed.",
            |c, a: Revisioned| folder(c, c.store.set_note_folder_removed(a.removed(false)?)?),
        ),
        tool(
            "link_create",
            Kind::Write,
            "Link a Note to a Task (returns the existing link if they are linked).",
            |c, a: LinkCreate| item(contract::link(&c.store.link_note(&a.task_id, &a.note_id)?)),
        ),
        tool(
            "link_get",
            Kind::Read,
            "Read one Task–Note link.",
            |c, a: Id| {
                item(contract::link(
                    &c.store.get_link(&a.id)?.ok_or(TaskError::LinkNotFound)?,
                ))
            },
        ),
        tool(
            "link_remove",
            Kind::Write,
            "Unlink a Note from a Task (reversible with link_restore).",
            |c, a: Id| item(contract::link(&c.store.unlink_note(&a.id)?)),
        ),
        tool(
            "link_restore",
            Kind::Write,
            "Restore an unlinked Task–Note link.",
            |c, a: Id| item(contract::link(&c.store.restore_link(&a.id)?)),
        ),
        tool(
            "removed_list",
            Kind::Read,
            "The Removed view: Tasks, Notes, Lists and folders in Removed, with the revision a restore must name.",
            |c, a: TextPage| {
                let page = c.store.list_removed(
                    a.text.as_deref().unwrap_or_default(),
                    limit(a.limit),
                    a.cursor.as_deref(),
                )?;
                Ok(contract::success_page(
                    page.items.iter().map(contract::removed_item).collect(),
                    page.next_cursor,
                ))
            },
        ),
        tool(
            "audit_list",
            Kind::Read,
            "Audit Entries (who changed what, never any text), newest first, optionally for one entity.",
            |c, a: AuditPage| {
                let page = c.store.list_audit_entries(
                    a.entity_id.as_deref(),
                    limit(a.limit),
                    a.cursor.as_deref(),
                )?;
                Ok(contract::success_page(
                    page.items.iter().map(contract::to_value).collect(),
                    page.next_cursor,
                ))
            },
        ),
    ];
    tools.extend(crate::tools_bulk::tools());
    tools
}

pub(crate) fn tool_def<A: DeserializeOwned + JsonSchema + 'static>(
    name: &'static str,
    write: bool,
    description: &'static str,
    run: fn(&Context, A) -> Result<Value, CliError>,
) -> ToolDef {
    tool(
        name,
        if write { Kind::Write } else { Kind::Read },
        description,
        run,
    )
}

pub(crate) fn item(value: Value) -> Result<Value, CliError> {
    Ok(contract::success_item(value))
}

pub(crate) fn task(task: Task) -> Value {
    contract::task(&task)
}

fn folder(
    c: &Context,
    folder: tasknotes_core::note_folders::NoteFolder,
) -> Result<Value, CliError> {
    item(contract::folder_read(c.store, &folder)?)
}

/// The revision the caller last read: a whole number of at least 1, as in
/// the CLI.
pub(crate) fn revision(value: i64) -> Result<i64, CliError> {
    if value >= 1 {
        Ok(value)
    } else {
        Err(CliError::invalid(
            "revision must be a whole number of at least 1",
        ))
    }
}

/// A JSON Schema string enum of the core's serde names for `all`.
pub(crate) fn names_schema<T: Serialize>(all: &[T]) -> Schema {
    let names: Vec<Value> = all.iter().map(contract::to_value).collect();
    json_schema!({"type": "string", "enum": names})
}

fn status_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&WorkStatus::ALL)
}

fn priority_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&Priority::ALL)
}

fn sort_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&SortMode::ALL)
}

fn nullable_date_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": ["string", "null"],
        "description": "YYYY-MM-DD sets the Deadline; null clears it; leave out to keep it."
    })
}

/// Tells a field that is absent (`None`) from one that is `null` (`Some(None)`).
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Id {
    pub id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Revisioned {
    id: String,
    /// The revision you last read.
    revision: i64,
}

impl Revisioned {
    fn removed(self, removed: bool) -> Result<SetRemoved, CliError> {
        Ok(SetRemoved {
            expected_revision: revision(self.revision)?,
            id: self.id,
            removed,
        })
    }
}

/// The page size: 50 when left out; the core checks the bounds.
pub(crate) fn limit(value: Option<u32>) -> u32 {
    value.unwrap_or(50)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct IdPage {
    id: String,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListPage {
    #[serde(default)]
    include_removed: bool,
    /// Page size, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TextPage {
    /// Words to search for; blank lists everything.
    #[serde(default)]
    text: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AuditPage {
    /// Only entries about this Task, List, Note, folder or link.
    #[serde(default)]
    entity_id: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FolderPage {
    /// List inside this folder; the top level when left out.
    #[serde(default)]
    parent_id: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Name {
    name: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Rename {
    id: String,
    revision: i64,
    name: String,
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
    if a.list_id.is_some() == a.inbox {
        return Err(CliError::invalid(
            "Give exactly one of list_id or inbox: true",
        ));
    }
    item(task(c.store.move_task(MoveTask {
        expected_revision: revision(a.revision)?,
        id: a.id,
        list_id: a.list_id,
    })?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoteListArgs {
    /// Search words: every word must match the Note's text.
    #[serde(default)]
    text: Option<String>,
    /// Only Notes directly in this folder.
    #[serde(default)]
    folder_id: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

fn note_list(c: &Context, a: NoteListArgs) -> Result<Value, CliError> {
    let query = NoteQuery {
        text: a.text.unwrap_or_default(),
        folder: a.folder_id.map_or(NoteScope::All, NoteScope::Folder),
    };
    let page = c
        .store
        .query_notes(&query, limit(a.limit), a.cursor.as_deref())?;
    let items = page
        .items
        .iter()
        .map(|n| contract::note_summary_read(c.store, n))
        .collect::<Result<_, _>>()?;
    Ok(contract::success_page(items, page.next_cursor))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoteCreate {
    markdown: String,
    #[serde(default)]
    folder_id: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoteUpdate {
    id: String,
    revision: i64,
    markdown: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoteMoveArgs {
    id: String,
    revision: i64,
    /// The folder to move into; give this or no_folder.
    #[serde(default)]
    folder_id: Option<String>,
    /// Take the Note out of every folder; give this or folder_id.
    #[serde(default)]
    no_folder: bool,
}

fn note_move(c: &Context, a: NoteMoveArgs) -> Result<Value, CliError> {
    if a.folder_id.is_some() == a.no_folder {
        return Err(CliError::invalid(
            "Give exactly one of folder_id or no_folder: true",
        ));
    }
    let note = c.store.move_note(MoveNote {
        expected_revision: Some(revision(a.revision)?),
        id: a.id,
        folder_id: a.folder_id,
    })?;
    item(contract::note_read(c.store, &note)?)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FolderCreate {
    name: String,
    #[serde(default)]
    parent_id: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FolderMoveArgs {
    id: String,
    revision: i64,
    /// The folder to move into; give this or top.
    #[serde(default)]
    parent_id: Option<String>,
    /// Move to the top level; give this or parent_id.
    #[serde(default)]
    top: bool,
}

fn folder_move(c: &Context, a: FolderMoveArgs) -> Result<Value, CliError> {
    if a.parent_id.is_some() == a.top {
        return Err(CliError::invalid(
            "Give exactly one of parent_id or top: true",
        ));
    }
    folder(
        c,
        c.store.move_note_folder(MoveNoteFolder {
            expected_revision: revision(a.revision)?,
            id: a.id,
            parent_id: a.parent_id,
        })?,
    )
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LinkCreate {
    task_id: String,
    note_id: String,
}
