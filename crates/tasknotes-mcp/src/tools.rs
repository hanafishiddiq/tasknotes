//! The MCP tools: each one reads its arguments, calls one core operation on
//! a store opened as `Interface::Mcp`, and shapes the result with the CLI's
//! contract. Validation, revisions, Removed, Exclude from AI and Audit
//! Entries are all the core's; nothing here decides them.

use std::sync::Arc;

use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tasknotes_cli::contract::{self, CliError};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::style::{self, SetStyle};
use tasknotes_core::tasks::{
    Difficulty, DifficultyFilter, Priority, SortMode, Task, TaskStore, WorkStatus,
};

/// What a tool runs against.
pub struct Context<'a> {
    pub store: &'a TaskStore,
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
pub(crate) enum Kind {
    /// Reads only.
    Read,
    /// Changes data, reversibly (nothing is ever deleted).
    Write,
}

pub(crate) fn tool<A: DeserializeOwned + JsonSchema + 'static>(
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
    let mut tools = crate::tools_task::tools();
    tools.extend(crate::tools_note::tools());
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

/// How the MCP tools name a Task's move target.
pub(crate) const LIST_OR_INBOX: (&str, &str) = ("list_id", "inbox: true");

pub(crate) fn item(value: Value) -> Result<Value, CliError> {
    Ok(contract::success_item(value))
}

pub(crate) fn task(task: Task) -> Value {
    contract::task(&task)
}

pub(crate) fn folder(
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

pub(crate) fn status_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&WorkStatus::ALL)
}

pub(crate) fn priority_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&Priority::ALL)
}

/// For a filter: a level's name, or `none` for only the Tasks with No
/// Difficulty; leaving it out is Any Difficulty.
pub(crate) fn difficulty_filter_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&DifficultyFilter::ALL)
}

/// For a change: a name sets the Difficulty, `null` clears it to No
/// Difficulty, and leaving it out keeps it.
pub(crate) fn nullable_difficulty_schema(_: &mut SchemaGenerator) -> Schema {
    let mut names: Vec<Value> = Difficulty::ALL.iter().map(contract::to_value).collect();
    names.push(Value::Null);
    json_schema!({
        "type": ["string", "null"],
        "enum": names,
        "description": "A name sets the Difficulty; null clears it to No Difficulty; leave out to keep it."
    })
}

pub(crate) fn sort_schema(_: &mut SchemaGenerator) -> Schema {
    names_schema(&SortMode::ALL)
}

pub(crate) fn nullable_date_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": ["string", "null"],
        "description": "YYYY-MM-DD sets the Deadline; null clears it; leave out to keep it."
    })
}

/// Tells a field that is absent (`None`) from one that is `null` (`Some(None)`).
pub(crate) use tasknotes_core::tasks::present;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Id {
    pub id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Revisioned {
    pub id: String,
    /// The revision you last read.
    pub revision: i64,
}

impl Revisioned {
    pub(crate) fn removed(self, removed: bool) -> Result<SetRemoved, CliError> {
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
pub(crate) struct IdPage {
    pub id: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListPage {
    #[serde(default)]
    pub include_removed: bool,
    /// Page size, 50 when left out.
    #[serde(default)]
    pub limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextPage {
    /// Words to search for; blank lists everything.
    #[serde(default)]
    pub text: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    pub limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuditPage {
    /// Only entries about this Task, List, Note, folder or link.
    #[serde(default)]
    pub entity_id: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    pub limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FolderPage {
    /// List inside this folder; the top level when left out.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Page size, 50 when left out.
    #[serde(default)]
    pub limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Name {
    pub name: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reorder {
    pub id: String,
    /// The revision you last read.
    pub revision: i64,
    /// Put it directly after this sibling; give this or first.
    #[serde(default)]
    pub after_id: Option<String>,
    /// Put it first among its siblings; give this or after_id.
    #[serde(default)]
    pub first: bool,
}

impl Reorder {
    pub(crate) fn after(&self) -> Result<Option<String>, CliError> {
        contract::destination(
            self.after_id.clone(),
            self.first,
            ("after_id", "first: true"),
        )
    }
}

/// A List's or Note Folder's icon and colour; the core checks the names.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Style {
    pub id: String,
    /// The revision you last read.
    pub revision: i64,
    #[serde(default, deserialize_with = "present")]
    #[schemars(schema_with = "nullable_icon_schema")]
    pub icon: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    #[schemars(schema_with = "nullable_color_schema")]
    pub color: Option<Option<String>>,
}

impl Style {
    pub(crate) fn change(self) -> Result<SetStyle, CliError> {
        Ok(SetStyle {
            expected_revision: revision(self.revision)?,
            id: self.id,
            icon: self.icon,
            color: self.color,
        })
    }
}

/// The icon names are too many for an enum; `style_options` lists them.
pub(crate) fn nullable_icon_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": ["string", "null"],
        "description": "A Lucide icon name from style_options sets the icon; null clears it to the default; leave out to keep it."
    })
}

pub(crate) fn nullable_color_schema(_: &mut SchemaGenerator) -> Schema {
    let mut names: Vec<Value> = style::options()
        .colors
        .iter()
        .map(|c| Value::from(c.as_str()))
        .collect();
    names.push(Value::Null);
    json_schema!({
        "type": ["string", "null"],
        "enum": names,
        "description": "A colour name sets the colour; null clears it to the default; leave out to keep it."
    })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoArguments {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rename {
    pub id: String,
    pub revision: i64,
    pub name: String,
}
