//! Note, Note Folder, Note Link, Removed and audit tools.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tasknotes_cli::contract::{self, CliError};
use tasknotes_core::note_folders::{CreateNoteFolder, MoveNoteFolder, RenameNoteFolder};
use tasknotes_core::notes::{CreateNote, MoveNote, NoteQuery, NoteScope, UpdateNote};
use tasknotes_core::tasks::TaskError;

use crate::tools::*;

pub fn tools() -> Vec<ToolDef> {
    vec![
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
    ]
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
    let folder_id =
        contract::destination(a.folder_id, a.no_folder, ("folder_id", "no_folder: true"))?;
    let note = c.store.move_note(MoveNote {
        expected_revision: Some(revision(a.revision)?),
        id: a.id,
        folder_id,
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
    let parent_id = contract::destination(a.parent_id, a.top, ("parent_id", "top: true"))?;
    folder(
        c,
        c.store.move_note_folder(MoveNoteFolder {
            expected_revision: revision(a.revision)?,
            id: a.id,
            parent_id,
        })?,
    )
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LinkCreate {
    task_id: String,
    note_id: String,
}
