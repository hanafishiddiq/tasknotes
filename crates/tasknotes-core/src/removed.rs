//! Removed (ADR-0002): a reversible disposition of Tasks, Notes, Lists and
//! Note Folders. Nothing here deletes a row. Removing a container hides what
//! is inside it without touching their own disposition, so restoring the
//! container leaves anything Removed on its own in Removed.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;

use crate::tasks::{COLUMNS, Task, TaskError, TaskStore, current_millis, read_task};

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
}
