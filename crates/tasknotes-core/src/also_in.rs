//! One Task in several Lists (owner, batch 4, 2026-11-18). A Task keeps its
//! own List (`tasks.list_id`, or Inbox) and can also be shown in other Lists:
//! one row in `task_list_links` per pair. It is the same Task everywhere, so
//! a change in one List shows in all of them. Each List orders it in its own
//! Custom Order: its own List by `tasks.position`, another by the link's
//! `position`. Taking it out of one List marks the link, never deletes it;
//! taking it out of its own List makes the oldest other List its own. Both
//! change the Task: they name its revision and are audited.

use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use serde::Deserialize;

use crate::audit::AuditAction;
use crate::tasks::{COLUMNS, Task, TaskError, TaskStore, current_millis, read_task};

/// Shows a Task in one more List, or takes it out of one.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowTaskIn {
    pub id: String,
    pub expected_revision: i64,
    pub list_id: String,
}

/// Schema v27: the Lists a Task is also shown in. `IF NOT EXISTS`: a file
/// set back to an older version keeps them.
pub(crate) fn migrate_v27(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE IF NOT EXISTS task_list_links (
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
            list_id TEXT NOT NULL REFERENCES lists(id) ON DELETE RESTRICT,
            position INTEGER NOT NULL,
            linked_at INTEGER NOT NULL,
            unlinked_at INTEGER,
            PRIMARY KEY (task_id, list_id)
         );
         CREATE INDEX IF NOT EXISTS task_list_links_list
           ON task_list_links(list_id, position) WHERE unlinked_at IS NULL;
         PRAGMA user_version = 27;",
    )?;
    Ok(())
}

/// SQL: the Task aliased `alias` is in the List bound as `list`: its own, or
/// one it is also shown in.
pub(crate) fn in_list_sql(alias: &str, list: &str) -> String {
    format!(
        "({alias}.list_id = {list} OR EXISTS (SELECT 1 FROM task_list_links k
           WHERE k.task_id = {alias}.id AND k.list_id = {list} AND k.unlinked_at IS NULL))"
    )
}

/// SQL: the place of the Task aliased `alias` in the Custom Order of the
/// List bound as `list`.
pub(crate) fn position_in_sql(alias: &str, list: &str) -> String {
    format!(
        "CASE WHEN {alias}.list_id = {list} THEN {alias}.position ELSE (SELECT k.position
           FROM task_list_links k WHERE k.task_id = {alias}.id AND k.list_id = {list}) END"
    )
}

/// The place above every Task the List shows, its own and those also shown in it.
pub(crate) fn top_of(conn: &Connection, list_id: &str) -> Result<i64, TaskError> {
    Ok(conn.query_row(
        "SELECT coalesce(min(position), 1) - 1 FROM (
             SELECT position FROM tasks WHERE list_id = ?1
             UNION ALL
             SELECT position FROM task_list_links WHERE list_id = ?1 AND unlinked_at IS NULL)",
        params![list_id],
        |row| row.get(0),
    )?)
}

/// Fills each Task's `also_in`: the Lists it is also shown in, oldest first.
pub(crate) fn hydrate_also_in(conn: &Connection, tasks: &mut [Task]) -> Result<(), TaskError> {
    for chunk in tasks.chunks_mut(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let mut stmt = conn.prepare(&format!(
            "SELECT task_id, list_id FROM task_list_links
             WHERE unlinked_at IS NULL AND task_id IN ({marks})
             ORDER BY linked_at, list_id"
        ))?;
        let ids: Vec<String> = chunk.iter().map(|task| task.id.clone()).collect();
        let rows = stmt
            .query_map(params_from_iter(ids), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for task in chunk.iter_mut() {
            task.also_in = rows
                .iter()
                .filter(|(id, _)| *id == task.id)
                .map(|(_, list)| list.clone())
                .collect();
        }
    }
    Ok(())
}

/// Drops the link to `list_id`: the Task now lives there as its own List.
pub(crate) fn forget_link(tx: &Transaction, task_id: &str, list_id: &str) -> Result<(), TaskError> {
    tx.execute(
        "DELETE FROM task_list_links WHERE task_id = ?1 AND list_id = ?2",
        params![task_id, list_id],
    )?;
    Ok(())
}

impl TaskStore {
    /// Shows a Task in one more List, at the top of its group there, or back
    /// in the place it had when it is shown there again.
    pub fn show_task_in(&self, input: ShowTaskIn) -> Result<Task, TaskError> {
        self.audited(AuditAction::TaskShowIn, input, |input| {
            let mut conn = self.connect()?;
            let tx = self.begin_write(&mut conn)?;
            let task = self.task_for_write(&tx, &input)?;
            crate::organization::require_active_list(&tx, &input.list_id)?;
            if task.list_id.as_deref() == Some(input.list_id.as_str())
                || task.also_in.contains(&input.list_id)
            {
                return Err(TaskError::TaskUnchanged);
            }
            let now = current_millis()?;
            let top = top_of(&tx, &input.list_id)?;
            tx.execute(
                "INSERT INTO task_list_links (task_id, list_id, position, linked_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(task_id, list_id) DO UPDATE SET unlinked_at = NULL, linked_at = ?4",
                params![task.id, input.list_id, top, now],
            )?;
            self.finish(tx, AuditAction::TaskShowIn, &input, now)
        })
    }

    /// Takes a Task out of one List and leaves it in the others. Out of its
    /// own List, the List it was first also shown in becomes its own. A Task
    /// in one List only stays: Move to Removed is for that.
    pub fn hide_task_from(&self, input: ShowTaskIn) -> Result<Task, TaskError> {
        self.audited(AuditAction::TaskHideFrom, input, |input| {
            let mut conn = self.connect()?;
            let tx = self.begin_write(&mut conn)?;
            let task = self.task_for_write(&tx, &input)?;
            let now = current_millis()?;
            if task.also_in.contains(&input.list_id) {
                tx.execute(
                    "UPDATE task_list_links SET unlinked_at = ?3 WHERE task_id = ?1 AND list_id = ?2",
                    params![task.id, input.list_id, now],
                )?;
            } else if task.list_id.as_deref() == Some(input.list_id.as_str()) {
                let Some(next) = task.also_in.first() else {
                    return Err(TaskError::TaskInOneListOnly);
                };
                let position: i64 = tx.query_row(
                    "SELECT position FROM task_list_links WHERE task_id = ?1 AND list_id = ?2",
                    params![task.id, next],
                    |row| row.get(0),
                )?;
                forget_link(&tx, &task.id, next)?;
                tx.execute(
                    "UPDATE tasks SET list_id = ?2, position = ?3 WHERE id = ?1",
                    params![task.id, next, position],
                )?;
            } else {
                return Err(TaskError::TaskUnchanged);
            }
            self.finish(tx, AuditAction::TaskHideFrom, &input, now)
        })
    }

    /// The Task as a write finds it, with the Lists it is also in.
    fn task_for_write(&self, tx: &Transaction, input: &ShowTaskIn) -> Result<Task, TaskError> {
        let mut task = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1"),
                params![input.id],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;
        crate::removed::require_writable_task(tx, &task.id)?;
        if task.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        hydrate_also_in(tx, std::slice::from_mut(&mut task))?;
        Ok(task)
    }

    /// The new revision of the Task, audited, committed and read back whole.
    fn finish(
        &self,
        tx: Transaction,
        action: AuditAction,
        input: &ShowTaskIn,
        now: i64,
    ) -> Result<Task, TaskError> {
        let mut updated = tx.query_row(
            &format!(
                "UPDATE tasks SET revision = revision + 1, updated_at = max(updated_at + 1, ?1)
                 WHERE id = ?2 AND revision = ?3 RETURNING {COLUMNS}"
            ),
            params![now, input.id, input.expected_revision],
            read_task,
        )?;
        self.audit_applied(&tx, action, &updated.id, updated.revision)?;
        crate::tasks::hydrate(&tx, std::slice::from_mut(&mut updated))?;
        tx.commit()?;
        Ok(updated)
    }
}
