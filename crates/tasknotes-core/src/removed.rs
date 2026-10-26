//! Removed (ADR-0002): a reversible disposition of Tasks, Notes, Lists and
//! Note Folders. Nothing here deletes a row. Removing a container hides what
//! is inside it without touching their own disposition, so restoring the
//! container leaves anything Removed on its own in Removed.

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::audit::AuditAction;
use crate::notes::{Note, read_note};
use crate::search::{fingerprint, parse_time_id_cursor, search_terms};
use crate::tasks::{COLUMNS, Task, TaskError, TaskStore, current_millis, read_task};

const HIDDEN_FOLDERS: &str = hidden_folders!();

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemovedKind {
    Task,
    Note,
    List,
    NoteFolder,
}

impl RemovedKind {
    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "task" => Some(Self::Task),
            "note" => Some(Self::Note),
            "list" => Some(Self::List),
            "note_folder" => Some(Self::NoteFolder),
            _ => None,
        }
    }
}

/// One restorable record as the Removed view lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedItem {
    pub kind: RemovedKind,
    pub id: String,
    /// A Task's or Note's title, a List's or folder's name.
    pub title: String,
    /// The revision a restore must name.
    pub revision: i64,
    pub removed_at: i64,
    /// Its List, or a folder above it, is in Removed too: that comes back first.
    pub in_removed_container: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedPage {
    pub items: Vec<RemovedItem>,
    pub next_cursor: Option<String>,
}

/// Everything Removed on its own; what a Removed container merely hides is
/// not listed, since it comes back with the container. Each `/*kind*/` marker
/// takes that kind's search filter.
const REMOVED_ITEMS: &str = concat!(
    "SELECT 'task' AS kind, t.id, t.title, t.revision, t.removed_at, ",
    task_in_removed_list!("t"),
    " AS blocked FROM tasks t WHERE t.removed_at IS NOT NULL /*task*/
     UNION ALL
     SELECT 'note', n.id, n.title, n.revision, n.removed_at, ",
    note_in_hidden_folder!("n"),
    "
       FROM notes n WHERE n.removed_at IS NOT NULL /*note*/
     UNION ALL
     SELECT 'list', id, name, revision, removed_at, 0 FROM lists
       WHERE removed_at IS NOT NULL /*list*/
     UNION ALL
     SELECT 'note_folder', f.id, f.name, f.revision, f.removed_at,
            coalesce(f.parent_id IN (",
    hidden_folders!(),
    "), 0)
       FROM note_folders f WHERE f.removed_at IS NOT NULL /*folder*/"
);

/// A Task can be written only while it is shown: not in Removed itself and
/// not in a Removed List.
pub(crate) fn require_writable_task(conn: &Connection, id: &str) -> Result<(), TaskError> {
    let state: Option<(bool, bool)> = conn
        .query_row(
            concat!(
                "SELECT t.removed_at IS NOT NULL, ",
                task_in_removed_list!("t"),
                " FROM tasks t WHERE t.id = ?1"
            ),
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
    Ok(conn.query_row(
        concat!(
            "SELECT ",
            task_in_removed_list!("t"),
            " FROM tasks t WHERE t.id = ?1"
        ),
        params![task.id],
        |row| row.get(0),
    )?)
}

impl TaskStore {
    /// The Removed view: every Task, Note, List and folder Removed on its own,
    /// most recently removed first, as a bounded keyset page. Nothing in it is
    /// ever purged; it leaves only by being restored.
    ///
    /// `text` searches it (story 36: ordinary search never shows Removed, this
    /// is where you include it): every word must match, as a word prefix in a
    /// Task's fields and Progress Entries or a Note's text, or anywhere in a
    /// List's or folder's name. Blank text lists everything.
    pub fn list_removed(
        &self,
        text: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<RemovedPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let terms = search_terms(text)?;
        // The cursor names the search it continues, so it cannot skip into another.
        let version = format!("r1.{}", fingerprint(&[&terms.join(" ")]));
        let after = cursor
            .map(|value| parse_time_id_cursor(value, &version, TaskError::InvalidRemovedCursor))
            .transpose()?;
        // Text with no words in it ("!!!") matches nothing.
        if terms.is_empty() && !text.trim().is_empty() {
            return Ok(RemovedPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let mut values: Vec<Value> = Vec::new();
        let mut bind = |value: Value| {
            values.push(value);
            format!("?{}", values.len())
        };
        let (mut task, mut note, mut list, mut folder) =
            (String::new(), String::new(), String::new(), String::new());
        for term in &terms {
            // A term is a quoted FTS prefix (`"word"*`); names match its bare word,
            // which holds only letters and digits, so LIKE sees no wildcards.
            let word = bind(Value::Text(term.trim_matches(['"', '*']).to_owned()));
            let term = bind(Value::Text(term.clone()));
            task.push_str(&format!(
                " AND (t.rowid IN (SELECT rowid FROM task_fields_fts WHERE task_fields_fts MATCH {term})
                   OR t.rowid IN (SELECT task_rowid FROM progress_fts WHERE progress_fts MATCH {term}))"
            ));
            note.push_str(&format!(
                " AND n.rowid IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH {term})"
            ));
            list.push_str(&format!(" AND name LIKE '%' || {word} || '%'"));
            folder.push_str(&format!(" AND f.name LIKE '%' || {word} || '%'"));
        }
        let page_filter = match after {
            None => "1".to_owned(),
            Some((time, id)) => format!(
                "(removed_at, id) < ({}, {})",
                bind(Value::Integer(time)),
                bind(Value::Text(id))
            ),
        };
        let limit_param = bind(Value::Integer(i64::from(limit) + 1));
        let items_sql = REMOVED_ITEMS
            .replace("/*task*/", &task)
            .replace("/*note*/", &note)
            .replace("/*list*/", &list)
            .replace("/*folder*/", &folder);
        let conn = self.connect()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT kind, id, title, revision, removed_at, blocked FROM ({items_sql})
             WHERE {page_filter}
             ORDER BY removed_at DESC, id DESC LIMIT {limit_param}"
        ))?;
        let mut items = stmt
            .query_map(params_from_iter(values), |row| {
                let kind: String = row.get(0)?;
                Ok(RemovedItem {
                    kind: RemovedKind::from_sql(&kind)
                        .ok_or_else(|| crate::tasks::invalid_db_text(0, kind))?,
                    id: row.get(1)?,
                    title: row.get(2)?,
                    revision: row.get(3)?,
                    removed_at: row.get(4)?,
                    in_removed_container: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|last| format!("{version}|{}|{}", last.removed_at, last.id))
        } else {
            None
        };
        Ok(RemovedPage { items, next_cursor })
    }

    pub fn set_task_removed(&self, input: SetRemoved) -> Result<Task, TaskError> {
        let action = if input.removed {
            AuditAction::TaskRemove
        } else {
            AuditAction::TaskRestore
        };
        let (id, revision) = (input.id.clone(), input.expected_revision);
        self.audited(action, &id, revision, || {
            self.set_task_removed_inner(input, action)
        })
    }

    fn set_task_removed_inner(
        &self,
        input: SetRemoved,
        action: AuditAction,
    ) -> Result<Task, TaskError> {
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
        // While its List is in Removed the Task is hidden with it: restoring
        // would leave it hidden, and removing it would make it stay in Removed
        // when the List comes back. The List comes first either way.
        if task_list_removed(&tx, &old)? {
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
        self.audit_applied(&tx, action, &changed.id, changed.revision)?;
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
        // Under a Removed folder the Note is hidden with it; the folder comes
        // first, whether restoring or removing the Note on its own.
        note_folder_free(&tx, &old)?;
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
