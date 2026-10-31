//! Note, Note Folder, Note Link and Removed commands. Exclude from AI is
//! enforced by the core store this adapter opens for `Interface::Cli`: an
//! excluded Note is simply never returned, and every command naming one
//! fails as `not_found`, like an id that was never used.

use serde_json::Value;
use tasknotes_core::note_folders::{
    CreateNoteFolder, MoveNoteFolder, PlaceNoteFolder, RenameNoteFolder,
};
use tasknotes_core::notes::{CreateNote, MoveNote, Note, NoteQuery, NoteScope, UpdateNote};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{TaskError, TaskStore};

use crate::args::{Spec, parse};
use crate::contract::{self, CliError};
use crate::{ONE_ID, USAGE, flags};

fn page_flags(positionals: usize) -> Spec<'static> {
    flags(&["limit", "cursor"], positionals)
}

fn note_item(store: &TaskStore, note: &Note) -> Result<Value, CliError> {
    Ok(contract::success_item(contract::note_read(store, note)?))
}

pub fn note_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let note = match verb {
        "list" => return note_page(store, words),
        "tasks" => {
            let p = parse(words, &page_flags(1))?;
            let page = store.list_note_tasks(p.id(), p.limit()?, p.value("cursor"))?;
            return Ok(contract::success_page(
                page.items.iter().map(contract::linked_task).collect(),
                page.next_cursor,
            ));
        }
        "get" => {
            let p = parse(words, &ONE_ID)?;
            store.get_note(p.id())?.ok_or(TaskError::NoteNotFound)?
        }
        "create" => {
            let p = parse(words, &flags(&["markdown", "folder"], 0))?;
            store.create_note(CreateNote {
                markdown: p.required("markdown")?.to_owned(),
                folder_id: p.value("folder").map(str::to_owned),
            })?
        }
        "update" => {
            let p = parse(words, &flags(&["revision", "markdown"], 1))?;
            store.update_note(UpdateNote {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                markdown: p.required("markdown")?.to_owned(),
            })?
        }
        "move" => {
            let p = parse(
                words,
                &Spec {
                    values: &["revision", "folder"],
                    repeated: &[],
                    switches: &["no-folder"],
                    positionals: 1,
                },
            )?;
            let folder_id = contract::destination(
                p.value("folder").map(str::to_owned),
                p.has("no-folder"),
                ("--folder ID", "--no-folder"),
            )?;
            store.move_note(MoveNote {
                id: p.id().to_owned(),
                folder_id,
                expected_revision: Some(p.revision()?),
            })?
        }
        "remove" | "restore" => {
            let p = parse(words, &flags(&["revision"], 1))?;
            store.set_note_removed(SetRemoved {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                removed: verb == "remove",
            })?
        }
        _ => return Err(CliError::invalid(USAGE)),
    };
    note_item(store, &note)
}

fn note_page(store: &TaskStore, words: &[String]) -> Result<Value, CliError> {
    let p = parse(words, &flags(&["limit", "cursor", "text", "folder"], 0))?;
    let query = NoteQuery {
        text: p.value("text").unwrap_or_default().to_owned(),
        folder: p
            .value("folder")
            .map_or(NoteScope::All, |id| NoteScope::Folder(id.to_owned())),
    };
    let page = store.query_notes(&query, p.limit()?, p.value("cursor"))?;
    let items = page
        .items
        .iter()
        .map(|note| contract::note_summary_read(store, note))
        .collect::<Result<Vec<_>, CliError>>()?;
    Ok(contract::success_page(items, page.next_cursor))
}

/// Notes linked to a Task (`task notes ID`).
pub fn task_notes(store: &TaskStore, words: &[String]) -> Result<Value, CliError> {
    let p = parse(words, &page_flags(1))?;
    let page = store.list_task_notes(p.id(), p.limit()?, p.value("cursor"))?;
    Ok(contract::success_page(
        page.items.iter().map(contract::linked_note).collect(),
        page.next_cursor,
    ))
}

pub fn folder_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let folder = match verb {
        "list" => {
            let p = parse(words, &flags(&["limit", "cursor", "parent"], 0))?;
            let page = store.list_note_folders(p.value("parent"), p.limit()?, p.value("cursor"))?;
            let items = page
                .items
                .iter()
                .map(|folder| contract::folder_read(store, folder))
                .collect::<Result<Vec<_>, CliError>>()?;
            return Ok(contract::success_page(items, page.next_cursor));
        }
        "get" => {
            let p = parse(words, &ONE_ID)?;
            store
                .get_note_folder(p.id())?
                .ok_or(TaskError::NoteFolderNotFound)?
        }
        "create" => {
            let p = parse(words, &flags(&["name", "parent"], 0))?;
            store.create_note_folder(CreateNoteFolder {
                name: p.required("name")?.to_owned(),
                parent_id: p.value("parent").map(str::to_owned),
            })?
        }
        "rename" => {
            let p = parse(words, &flags(&["revision", "name"], 1))?;
            store.rename_note_folder(RenameNoteFolder {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                name: p.required("name")?.to_owned(),
            })?
        }
        "move" => {
            let p = parse(
                words,
                &Spec {
                    values: &["revision", "parent"],
                    repeated: &[],
                    switches: &["top"],
                    positionals: 1,
                },
            )?;
            let parent_id = contract::destination(
                p.value("parent").map(str::to_owned),
                p.has("top"),
                ("--parent ID", "--top"),
            )?;
            store.move_note_folder(MoveNoteFolder {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                parent_id,
            })?
        }
        "reorder" => {
            let p = parse(
                words,
                &Spec {
                    values: &["revision", "after", "parent"],
                    repeated: &[],
                    switches: &["first", "top"],
                    positionals: 1,
                },
            )?;
            let after_id = contract::destination(
                p.value("after").map(str::to_owned),
                p.has("first"),
                ("--after ID", "--first"),
            )?;
            let parent = p.value("parent").map(str::to_owned);
            let parent_id = if parent.is_none() && !p.has("top") {
                // Without a new parent it stays among its siblings.
                store
                    .get_note_folder(p.id())?
                    .ok_or(TaskError::NoteFolderNotFound)?
                    .parent_id
            } else {
                contract::destination(parent, p.has("top"), ("--parent ID", "--top"))?
            };
            store.place_note_folder(PlaceNoteFolder {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                parent_id,
                after_id,
            })?
        }
        "remove" | "restore" => {
            let p = parse(words, &flags(&["revision"], 1))?;
            store.set_note_folder_removed(SetRemoved {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                removed: verb == "remove",
            })?
        }
        _ => return Err(CliError::invalid(USAGE)),
    };
    Ok(contract::success_item(contract::folder_read(
        store, &folder,
    )?))
}

pub fn link_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let link = match verb {
        "create" => {
            let p = parse(words, &flags(&["task", "note"], 0))?;
            store.link_note(p.required("task")?, p.required("note")?)?
        }
        "get" => {
            let p = parse(words, &ONE_ID)?;
            store.get_link(p.id())?.ok_or(TaskError::LinkNotFound)?
        }
        "remove" => store.unlink_note(parse(words, &ONE_ID)?.id())?,
        "restore" => store.restore_link(parse(words, &ONE_ID)?.id())?,
        _ => return Err(CliError::invalid(USAGE)),
    };
    Ok(contract::success_item(contract::link(&link)))
}

pub fn removed_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    if verb != "list" {
        return Err(CliError::invalid(USAGE));
    }
    let p = parse(words, &flags(&["limit", "cursor", "text"], 0))?;
    let page = store.list_removed(
        p.value("text").unwrap_or_default(),
        p.limit()?,
        p.value("cursor"),
    )?;
    Ok(contract::success_page(
        page.items.iter().map(contract::removed_item).collect(),
        page.next_cursor,
    ))
}
