//! Subtasks (owner, batch 4, 2026-11-18): a simple checklist inside one Task.
//! Each item is a text of up to 10,000 characters that is done or not. The
//! Task carries how many are done (`subtasks_done` of `subtasks_total`); its
//! progress and status stay the person's own. Removing an item marks it, as a
//! Note Link is unlinked: it is never deleted, and restoring brings it back in
//! its place. Every interface writes through here, audited.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::tasks::{Task, TaskError, TaskStore, current_millis};

/// The longest item: long enough for a paragraph, so a thought is never cut.
pub const SUBTASK_TEXT_CHARS: usize = 10_000;
/// The most items one Task's checklist holds, Removed ones not counted.
pub const SUBTASKS_PER_TASK: i64 = 500;

/// One item of a Task's checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
    pub id: String,
    pub task_id: String,
    pub text: String,
    pub done: bool,
    /// Removed from the checklist; `restore_subtask` brings it back.
    pub removed: bool,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSubtask {
    pub task_id: String,
    pub text: String,
}

/// A change to one item: its text, whether it is done, or both.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubtask {
    pub id: String,
    pub expected_revision: i64,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub done: Option<bool>,
}

/// Moves an item to `index` among the shown items (0 is first; past the end is last).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSubtask {
    pub id: String,
    pub expected_revision: i64,
    pub index: u32,
}

/// Removes or restores one item, naming the revision last read.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSubtaskRemoved {
    pub id: String,
    pub expected_revision: i64,
}

/// Schema v25: Subtasks. `position` orders a Task's items; the CHECKs hold
/// whoever writes the file to the same text rules as the core. `IF NOT
/// EXISTS`: a file set back to an older version keeps its checklists.
pub(crate) fn migrate_v25(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE IF NOT EXISTS task_subtasks (
            id TEXT PRIMARY KEY NOT NULL,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
            text TEXT NOT NULL CHECK(length(trim(text)) BETWEEN 1 AND 10000),
            done INTEGER NOT NULL DEFAULT 0 CHECK(done IN (0, 1)),
            position INTEGER NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            removed_at INTEGER
         );
         CREATE INDEX IF NOT EXISTS task_subtasks_order ON task_subtasks(task_id, position, id);
         PRAGMA user_version = 25;",
    )?;
    Ok(())
}

const COLUMNS: &str = "id, task_id, text, done, removed_at, revision, created_at, updated_at";

fn read_subtask(row: &Row<'_>) -> rusqlite::Result<Subtask> {
    Ok(Subtask {
        id: row.get(0)?,
        task_id: row.get(1)?,
        text: row.get(2)?,
        done: row.get(3)?,
        removed: row.get::<_, Option<i64>>(4)?.is_some(),
        revision: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// The text as it is kept: trimmed at both ends, line breaks inside kept.
fn checked_text(text: &str) -> Result<String, TaskError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(TaskError::EmptySubtask);
    }
    if text.chars().count() > SUBTASK_TEXT_CHARS {
        return Err(TaskError::SubtaskTooLong);
    }
    Ok(text.to_owned())
}

fn read(conn: &Connection, id: &str) -> Result<Subtask, TaskError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM task_subtasks WHERE id = ?1"),
        params![id],
        read_subtask,
    )
    .optional()?
    .ok_or(TaskError::SubtaskNotFound)
}

/// The item as a write finds it, once it exists, its Task takes changes and
/// the caller read its latest revision. A Removed item may still be found
/// here: each write decides what that means for it.
fn item_for_write(
    tx: &Transaction,
    id: &str,
    expected_revision: i64,
) -> Result<Subtask, TaskError> {
    let item = read(tx, id)?;
    crate::removed::require_writable_task(tx, &item.task_id)?;
    if item.revision != expected_revision {
        return Err(TaskError::RevisionConflict);
    }
    Ok(item)
}

/// Refuses a checklist that already holds the most items a Task may have.
fn require_room(tx: &Transaction, task_id: &str) -> Result<(), TaskError> {
    let shown: i64 = tx.query_row(
        "SELECT count(*) FROM task_subtasks WHERE task_id = ?1 AND removed_at IS NULL",
        params![task_id],
        |row| row.get(0),
    )?;
    if shown >= SUBTASKS_PER_TASK {
        return Err(TaskError::TooManySubtasks);
    }
    Ok(())
}

/// Fills each Task's checklist counts, Removed items left out.
pub(crate) fn hydrate_counts(conn: &Connection, tasks: &mut [Task]) -> Result<(), TaskError> {
    if tasks.is_empty() {
        return Ok(());
    }
    for chunk in tasks.chunks_mut(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let mut stmt = conn.prepare(&format!(
            "SELECT task_id, count(*), coalesce(sum(done), 0) FROM task_subtasks
             WHERE removed_at IS NULL AND task_id IN ({marks}) GROUP BY task_id"
        ))?;
        let ids: Vec<&str> = chunk.iter().map(|task| task.id.as_str()).collect();
        let counts = stmt
            .query_map(params_from_iter(ids), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, u32>(2)?,
                ))
            })?
            .map(|row| row.map(|(id, total, done)| (id, (total, done))))
            .collect::<Result<HashMap<String, (u32, u32)>, _>>()?;
        for task in chunk.iter_mut() {
            let (total, done) = counts.get(&task.id).copied().unwrap_or((0, 0));
            task.subtasks_total = total;
            task.subtasks_done = done;
        }
    }
    Ok(())
}

impl TaskStore {
    /// A Task's checklist in its order, Removed items left out.
    pub fn list_subtasks(&self, task_id: &str) -> Result<Vec<Subtask>, TaskError> {
        let conn = self.connect()?;
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
            params![task_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(TaskError::NotFound);
        }
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM task_subtasks
             WHERE task_id = ?1 AND removed_at IS NULL ORDER BY position, id"
        ))?;
        let items = stmt
            .query_map(params![task_id], read_subtask)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// Adds an item at the end of the Task's checklist.
    pub fn add_subtask(&self, input: AddSubtask) -> Result<Subtask, TaskError> {
        let text = checked_text(&input.text)?;
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        crate::removed::require_writable_task(&tx, &input.task_id)?;
        require_room(&tx, &input.task_id)?;
        let item = tx.query_row(
            &format!(
                "INSERT INTO task_subtasks (id, task_id, text, done, position, revision, created_at, updated_at)
                 SELECT ?1, ?2, ?3, 0, coalesce(max(position) + 1, 0), 1, ?4, ?4
                 FROM task_subtasks WHERE task_id = ?2
                 RETURNING {COLUMNS}"
            ),
            params![Uuid::new_v4().to_string(), input.task_id, text, now],
            read_subtask,
        )?;
        self.audit_applied(&tx, AuditAction::SubtaskCreate, &item.id, item.revision)?;
        tx.commit()?;
        Ok(item)
    }

    /// Changes an item's text, whether it is done, or both.
    pub fn update_subtask(&self, input: UpdateSubtask) -> Result<Subtask, TaskError> {
        self.audited(AuditAction::SubtaskUpdate, input, |input| {
            if input.text.is_none() && input.done.is_none() {
                return Err(TaskError::EmptyChange);
            }
            let text = input.text.as_deref().map(checked_text).transpose()?;
            let now = current_millis()?;
            let mut conn = self.connect()?;
            let tx = self.begin_write(&mut conn)?;
            let before = item_for_write(&tx, &input.id, input.expected_revision)?;
            // Off the checklist: nothing to change until it is restored.
            if before.removed {
                return Err(TaskError::SubtaskNotFound);
            }
            let text = text.unwrap_or_else(|| before.text.clone());
            let done = input.done.unwrap_or(before.done);
            if text == before.text && done == before.done {
                return Err(TaskError::SubtaskUnchanged);
            }
            let item = tx.query_row(
                &format!(
                    "UPDATE task_subtasks SET text = ?2, done = ?3, revision = revision + 1,
                       updated_at = max(updated_at + 1, ?4)
                     WHERE id = ?1 RETURNING {COLUMNS}"
                ),
                params![input.id, text, done, now],
                read_subtask,
            )?;
            self.audit_applied(&tx, AuditAction::SubtaskUpdate, &item.id, item.revision)?;
            tx.commit()?;
            Ok(item)
        })
    }

    /// Moves an item to another place in its checklist.
    pub fn move_subtask(&self, input: MoveSubtask) -> Result<Subtask, TaskError> {
        self.audited(AuditAction::SubtaskMove, input, |input| {
            let now = current_millis()?;
            let mut conn = self.connect()?;
            let tx = self.begin_write(&mut conn)?;
            let before = item_for_write(&tx, &input.id, input.expected_revision)?;
            if before.removed {
                return Err(TaskError::SubtaskNotFound);
            }
            // Every item, Removed ones too, keeps its place relative to the
            // others, so a restored item comes back where it was.
            let all: Vec<(String, bool)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, removed_at IS NOT NULL FROM task_subtasks WHERE task_id = ?1
                     ORDER BY position, id",
                )?;
                stmt.query_map(params![before.task_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<Result<_, _>>()?
            };
            let all_ids: Vec<String> = all.iter().map(|(id, _)| id.clone()).collect();
            let mut order: Vec<(String, bool)> =
                all.into_iter().filter(|(id, _)| *id != before.id).collect();
            // In front of the shown item now at `index`, or after the last shown one.
            let shown_at: Vec<usize> = order
                .iter()
                .enumerate()
                .filter(|(_, (_, removed))| !removed)
                .map(|(at, _)| at)
                .collect();
            let at = match shown_at.get(input.index as usize) {
                Some(at) => *at,
                None => shown_at.last().map_or(0, |last| last + 1),
            };
            order.insert(at, (before.id.clone(), false));
            if order.iter().map(|(id, _)| id).eq(all_ids.iter()) {
                return Err(TaskError::SubtaskUnchanged);
            }
            for (position, (id, _)) in order.iter().enumerate() {
                tx.execute(
                    "UPDATE task_subtasks SET position = ?2 WHERE id = ?1",
                    params![id, position as i64],
                )?;
            }
            let item = tx.query_row(
                &format!(
                    "UPDATE task_subtasks SET revision = revision + 1,
                       updated_at = max(updated_at + 1, ?2)
                     WHERE id = ?1 RETURNING {COLUMNS}"
                ),
                params![before.id, now],
                read_subtask,
            )?;
            self.audit_applied(&tx, AuditAction::SubtaskMove, &item.id, item.revision)?;
            tx.commit()?;
            Ok(item)
        })
    }

    /// Takes an item off the checklist; `restore_subtask` brings it back.
    pub fn remove_subtask(&self, id: &str, expected_revision: i64) -> Result<Subtask, TaskError> {
        let input = SetSubtaskRemoved {
            id: id.to_owned(),
            expected_revision,
        };
        self.audited(AuditAction::SubtaskRemove, input, |input| {
            let now = current_millis()?;
            let mut conn = self.connect()?;
            let tx = self.begin_write(&mut conn)?;
            let before = item_for_write(&tx, &input.id, input.expected_revision)?;
            if before.removed {
                return Err(TaskError::SubtaskUnchanged);
            }
            let item = tx.query_row(
                &format!(
                    "UPDATE task_subtasks SET removed_at = ?2, revision = revision + 1,
                       updated_at = max(updated_at + 1, ?2)
                     WHERE id = ?1 RETURNING {COLUMNS}"
                ),
                params![input.id, now],
                read_subtask,
            )?;
            self.audit_applied(&tx, AuditAction::SubtaskRemove, &item.id, item.revision)?;
            tx.commit()?;
            Ok(item)
        })
    }

    /// Undo for a removal: the item comes back in the place it had, naming
    /// the revision the removal gave it. One on the checklist already is
    /// returned unchanged.
    pub fn restore_subtask(&self, id: &str, expected_revision: i64) -> Result<Subtask, TaskError> {
        let input = SetSubtaskRemoved {
            id: id.to_owned(),
            expected_revision,
        };
        self.audited(AuditAction::SubtaskRestore, input, |input| {
            let now = current_millis()?;
            let mut conn = self.connect()?;
            let tx = self.begin_write(&mut conn)?;
            let before = read(&tx, &input.id)?;
            if !before.removed {
                return Ok(before);
            }
            let before = item_for_write(&tx, &input.id, input.expected_revision)?;
            require_room(&tx, &before.task_id)?;
            let item = tx.query_row(
                &format!(
                    "UPDATE task_subtasks SET removed_at = NULL, revision = revision + 1,
                       updated_at = max(updated_at + 1, ?2)
                     WHERE id = ?1 RETURNING {COLUMNS}"
                ),
                params![input.id, now],
                read_subtask,
            )?;
            self.audit_applied(&tx, AuditAction::SubtaskRestore, &item.id, item.revision)?;
            tx.commit()?;
            Ok(item)
        })
    }
}
