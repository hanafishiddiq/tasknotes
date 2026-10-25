//! Removed (ADR-0002): a reversible disposition of Tasks, Notes, Lists and
//! Note Folders. Nothing here deletes a row. Removing a container hides what
//! is inside it without touching their own disposition, so restoring the
//! container leaves anything Removed on its own in Removed.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;

use crate::notes::{Note, read_note};
use crate::tasks::{COLUMNS, Task, TaskError, TaskStore, current_millis, read_task};

pub(crate) const HIDDEN_FOLDERS: &str = hidden_folders!();

/// Schema v9: Removed for Notes and Note Folders (Tasks and Lists have had it
/// since v5 and v4). Folder names stay unique among active siblings only, as
/// List names are among active Lists. Partial indexes feed the Removed view.
pub(crate) fn migrate_v9(migration: &rusqlite::Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE notes ADD COLUMN removed_at INTEGER;
         ALTER TABLE note_folders ADD COLUMN removed_at INTEGER;
         DROP INDEX note_folders_sibling_name;
         CREATE UNIQUE INDEX note_folders_sibling_name
           ON note_folders(coalesce(parent_id, ''), name_fold) WHERE removed_at IS NULL;
         CREATE INDEX tasks_removed_order ON tasks(removed_at DESC, id DESC)
           WHERE removed_at IS NOT NULL;
         CREATE INDEX notes_removed_order ON notes(removed_at DESC, id DESC)
           WHERE removed_at IS NOT NULL;
         CREATE INDEX lists_removed_order ON lists(removed_at DESC, id DESC)
           WHERE removed_at IS NOT NULL;
         CREATE INDEX note_folders_removed_order ON note_folders(removed_at DESC, id DESC)
           WHERE removed_at IS NOT NULL;
         PRAGMA user_version = 9;",
    )?;
    Ok(())
}

/// Whether a folder is hidden: Removed itself or below a Removed folder.
pub(crate) fn folder_hidden(conn: &Connection, id: &str) -> Result<bool, TaskError> {
    Ok(conn.query_row(
        &format!("SELECT ?1 IN ({HIDDEN_FOLDERS})"),
        params![id],
        |row| row.get(0),
    )?)
}

/// A Note can be written only while it is shown: not in Removed itself and
/// not in (or below) a Removed folder.
pub(crate) fn require_writable_note(conn: &Connection, id: &str) -> Result<(), TaskError> {
    let note = conn
        .query_row(
            &format!("SELECT {} FROM notes WHERE id = ?1", crate::notes::COLUMNS),
            params![id],
            read_note,
        )
        .optional()?
        .ok_or(TaskError::NoteNotFound)?;
    if note.removed {
        return Err(TaskError::NoteRemoved);
    }
    note_folder_free(conn, &note)
}

/// Refuses when the Note sits in or below a Removed folder.
fn note_folder_free(conn: &Connection, note: &Note) -> Result<(), TaskError> {
    match &note.folder_id {
        Some(folder) if folder_hidden(conn, folder)? => Err(TaskError::NoteFolderRemoved),
        _ => Ok(()),
    }
}

/// Move one item into Removed (`removed: true`) or restore it, provided it is
/// still at `expected_revision`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRemoved {
    pub id: String,
    pub expected_revision: i64,
    pub removed: bool,
}

/// A Task can be written only while it is shown: not in Removed itself and
/// not in a Removed List.
pub(crate) fn require_writable_task(conn: &Connection, id: &str) -> Result<(), TaskError> {
    let state: Option<(bool, bool)> = conn
        .query_row(
            "SELECT t.removed_at IS NOT NULL, coalesce(l.removed_at IS NOT NULL, 0)
             FROM tasks t LEFT JOIN lists l ON l.id = t.list_id WHERE t.id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match state {
        None => Err(TaskError::NotFound),
        Some((true, _)) => Err(TaskError::TaskRemoved),
        Some((false, true)) => Err(TaskError::TaskListRemoved),
        Some((false, false)) => Ok(()),
    }
}

/// Whether the Task's List is in Removed.
fn task_list_removed(conn: &Connection, task: &Task) -> Result<bool, TaskError> {
    Ok(match &task.list_id {
        None => false,
        Some(list) => conn.query_row(
            "SELECT removed_at IS NOT NULL FROM lists WHERE id = ?1",
            params![list],
            |row| row.get(0),
        )?,
    })
}

impl TaskStore {
    pub fn set_task_removed(&self, input: SetRemoved) -> Result<Task, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1"),
                params![input.id],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.removed == input.removed {
            return Err(TaskError::RemovedUnchanged);
        }
        // Restoring into a Removed List would leave the Task hidden; the List comes first.
        if !input.removed && task_list_removed(&tx, &old)? {
            return Err(TaskError::TaskListRemoved);
        }
        let now = current_millis()?;
        let mut changed = tx.query_row(
            &format!(
                "UPDATE tasks SET removed_at = ?1, revision = revision + 1,
                   updated_at = max(updated_at + 1, ?2)
                 WHERE id = ?3 AND revision = ?4 RETURNING {COLUMNS}"
            ),
            params![
                input.removed.then_some(now),
                now,
                input.id,
                input.expected_revision
            ],
            read_task,
        )?;
        crate::organization::hydrate_tags(&tx, std::slice::from_mut(&mut changed))?;
        tx.commit()?;
        Ok(changed)
    }

    /// Removes or restores a Note. Its revision changes, so an editor open
    /// elsewhere notices; its edit time does not, so restoring is not an edit.
    pub fn set_note_removed(&self, input: SetRemoved) -> Result<Note, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old = tx
            .query_row(
                &format!("SELECT {} FROM notes WHERE id = ?1", crate::notes::COLUMNS),
                params![input.id],
                read_note,
            )
            .optional()?
            .ok_or(TaskError::NoteNotFound)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::NoteRevisionConflict);
        }
        if old.removed == input.removed {
            return Err(TaskError::RemovedUnchanged);
        }
        if !input.removed {
            note_folder_free(&tx, &old)?;
        }
        let now = current_millis()?;
        let changed = tx.query_row(
            &format!(
                "UPDATE notes SET removed_at = ?1, revision = revision + 1
                 WHERE id = ?2 AND revision = ?3 RETURNING {}",
                crate::notes::COLUMNS
            ),
            params![
                input.removed.then_some(now),
                input.id,
                input.expected_revision
            ],
            read_note,
        )?;
        tx.commit()?;
        Ok(changed)
    }
}
