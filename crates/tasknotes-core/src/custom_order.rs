//! Custom Order (ADR 0010): the order a person gives the Tasks of one List
//! (or Inbox) by dragging them within a work-status group, and the sort each
//! List remembers.
//!
//! Every Task of a List holds one `position` in a single sequence; a query
//! sorted by Custom shows each work-status group in that sequence. A move
//! only ever places a Task among the shown Tasks of its own group, so it
//! never changes a Task's status.

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::audit::AuditAction;
use crate::manual_order::{self, Siblings};
use crate::search::{self, STATUS_RANK};
use crate::tasks::{
    COLUMNS, SortMode, Task, TaskError, TaskStore, TaskView, current_millis, read_task,
};

/// Places one Task among the Tasks of its group in its List's Custom Order:
/// directly after `after_id`, directly before `before_id`, or first with
/// neither. Only the moved Task's revision is checked and bumped.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderTask {
    pub id: String,
    pub expected_revision: i64,
    #[serde(default)]
    pub after_id: Option<String>,
    /// The desktop names the row a Task was dropped above, whose
    /// predecessor may sit on a page it has not loaded.
    #[serde(default)]
    pub before_id: Option<String>,
    /// The sort the desktop was showing. Another sort than Custom first sets
    /// the List's Custom Order to that order. Left out (the CLI and MCP,
    /// which read the Custom Order), the move is made within it as it is.
    /// Either way the List then shows Custom.
    #[serde(default)]
    pub shown_sort: Option<SortMode>,
}

/// The sort one view remembers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewSort {
    pub view: TaskView,
    pub sort: SortMode,
}

/// Schema v20: every Task's place in its List's Custom Order, and the sort
/// each view remembers. The places start Newest first, where a new Task
/// lands; no List shows them until it is switched to Custom, which starts
/// it from the order on screen. No view remembers a sort yet, so every
/// view still opens by Priority. `custom_kept` says a List has a Custom
/// Order of its own, kept while it shows another sort.
pub(crate) fn migrate_v20(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(&format!(
        "ALTER TABLE tasks ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
         UPDATE tasks SET position = ordered.place FROM (
             SELECT id, row_number() OVER (
                 PARTITION BY coalesce(list_id, '') ORDER BY created_at DESC, id DESC
             ) AS place FROM tasks
         ) AS ordered WHERE tasks.id = ordered.id;
         CREATE INDEX tasks_custom_order ON tasks(list_id, {STATUS_RANK}, position, id);
         CREATE TABLE view_sorts (
             view TEXT PRIMARY KEY NOT NULL CHECK (length(view) BETWEEN 1 AND 64),
             sort TEXT NOT NULL CHECK (sort IN ('priority', 'newest', 'difficulty', 'custom')),
             custom_kept INTEGER NOT NULL DEFAULT 0 CHECK (custom_kept IN (0, 1))
         );
         PRAGMA user_version = 20;"
    ))?;
    Ok(())
}

/// The position that puts a new Task at the top of its group: above every
/// Task of the List.
pub(crate) fn top(tx: &Transaction, list_id: Option<&str>) -> Result<i64, TaskError> {
    Ok(tx.query_row(
        "SELECT coalesce(min(position), 1) - 1 FROM tasks WHERE list_id IS ?1",
        params![list_id],
        |row| row.get(0),
    )?)
}

fn view_of(list_id: Option<&str>) -> TaskView {
    list_id.map_or(TaskView::Inbox, |id| TaskView::List(id.to_owned()))
}

fn parse_view(key: &str) -> Option<TaskView> {
    match key {
        "all" => Some(TaskView::All),
        "inbox" => Some(TaskView::Inbox),
        _ => key
            .strip_prefix("list:")
            .map(|id| TaskView::List(id.to_owned())),
    }
}

/// Remembers `sort` for `view`. Showing Custom also marks the List's
/// Custom Order as its own from then on.
fn store_sort(tx: &Transaction, view: &TaskView, sort: SortMode) -> Result<(), TaskError> {
    tx.execute(
        "INSERT INTO view_sorts(view, sort, custom_kept) VALUES (?1, ?2, ?3)
         ON CONFLICT(view) DO UPDATE SET
             sort = excluded.sort, custom_kept = max(custom_kept, excluded.custom_kept)",
        params![
            view.cursor_key(),
            sort.as_cursor(),
            i64::from(sort == SortMode::Custom)
        ],
    )?;
    Ok(())
}

/// The sort `view` shows, and whether its List has a Custom Order of its
/// own: one it has shown before.
fn remembered(tx: &Transaction, view: &TaskView) -> Result<(SortMode, bool), TaskError> {
    let row: Option<(String, bool)> = tx
        .query_row(
            "SELECT sort, custom_kept FROM view_sorts WHERE view = ?1",
            params![view.cursor_key()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(row.map_or((SortMode::Priority, false), |(sort, kept)| {
        (
            SortMode::from_sql(&sort).unwrap_or(SortMode::Priority),
            kept,
        )
    }))
}

/// Sets the List's Custom Order to the order `sort` lists it in, every Task
/// of it (Removed ones too, which keep a place) in one sequence from 1.
/// Only positions that change are written; no revision changes.
fn seed(tx: &Transaction, list_id: Option<&str>, sort: SortMode) -> Result<(), TaskError> {
    if sort == SortMode::Custom {
        return Ok(());
    }
    tx.execute(
        &format!(
            "UPDATE tasks SET position = ordered.place FROM (
                 SELECT id, row_number() OVER (ORDER BY {}) AS place
                 FROM ({})
             ) AS ordered WHERE tasks.id = ordered.id AND tasks.position <> ordered.place",
            search::order_by(sort),
            search::ranked(sort, "id, created_at", "list_id IS ?1"),
        ),
        params![list_id],
    )?;
    Ok(())
}

impl TaskStore {
    /// Moves one Task within its work-status group of its List's Custom
    /// Order (ADR 0010), and switches the List to Custom, from whichever
    /// interface, so the person sees the order made. An anchor that is
    /// gone, in Removed, in another group or List is `revision_conflict`; a
    /// move nothing shows is `unchanged`, and switches nothing. Never a Bulk
    /// Change: it is one Task.
    pub fn reorder_task(&self, input: ReorderTask) -> Result<Task, TaskError> {
        self.audited(AuditAction::TaskReorder, input, |input| {
            self.reorder_task_inner(input)
        })
    }

    fn reorder_task_inner(&self, input: ReorderTask) -> Result<Task, TaskError> {
        if input.after_id.is_some() && input.before_id.is_some() {
            return Err(TaskError::InvalidTaskPlace);
        }
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let task = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1"),
                params![input.id],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;
        crate::removed::require_writable_task(&tx, &task.id)?;
        if task.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        let list_id = task.list_id.as_deref();
        if let Some(sort) = input.shown_sort {
            seed(&tx, list_id, sort)?;
        }
        let siblings = Siblings::Tasks {
            list_id,
            status: task.status,
        };
        let after_id = match input.before_id {
            Some(before) if before == task.id => return Err(TaskError::TaskUnchanged),
            Some(before) => manual_order::after_for_before(&tx, siblings, &task.id, &before)?,
            None => input.after_id,
        };
        // Dropping `tx` on a refusal also undoes the seeding.
        if !manual_order::place(&tx, siblings, &task.id, after_id.as_deref())? {
            return Err(TaskError::TaskUnchanged);
        }
        store_sort(&tx, &view_of(list_id), SortMode::Custom)?;
        let now = current_millis()?;
        let mut updated = tx.query_row(
            &format!(
                "UPDATE tasks SET revision = revision + 1, updated_at = max(updated_at + 1, ?1)
                 WHERE id = ?2 AND revision = ?3 RETURNING {COLUMNS}"
            ),
            params![now, task.id, input.expected_revision],
            read_task,
        )?;
        self.audit_applied(&tx, AuditAction::TaskReorder, &updated.id, updated.revision)?;
        crate::organization::hydrate_tags(&tx, std::slice::from_mut(&mut updated))?;
        tx.commit()?;
        Ok(updated)
    }

    /// The sort each view remembers; a view not listed opens by Priority.
    pub fn view_sorts(&self) -> Result<Vec<ViewSort>, TaskError> {
        let conn = self.connect()?;
        let mut statement = conn.prepare("SELECT view, sort FROM view_sorts ORDER BY view")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(view, sort)| {
                Some(ViewSort {
                    view: parse_view(&view)?,
                    sort: SortMode::from_sql(&sort)?,
                })
            })
            .collect())
    }

    /// Remembers the sort the person chose for one view. Choosing Custom
    /// for a List without a Custom Order of its own first starts one from
    /// the order on screen (the sort the view remembers), as a drag does;
    /// a List that has one gets it back. Desktop only: it is how the person
    /// looks at their Tasks, so it is not audited. Custom needs one List or
    /// Inbox.
    pub fn set_view_sort(&self, view: &TaskView, sort: SortMode) -> Result<(), TaskError> {
        self.require_desktop()?;
        let list_id = match view {
            TaskView::All if sort == SortMode::Custom => {
                return Err(TaskError::CustomOrderNeedsOneList);
            }
            TaskView::List(id) => Some(id.as_str()),
            TaskView::All | TaskView::Inbox => None,
        };
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        if let Some(id) = list_id {
            crate::organization::require_active_list(&tx, id)?;
        }
        if sort == SortMode::Custom {
            let (shown, kept) = remembered(&tx, view)?;
            if !kept {
                seed(&tx, list_id, shown)?;
            }
        }
        store_sort(&tx, view, sort)?;
        tx.commit()?;
        Ok(())
    }
}
