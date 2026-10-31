use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::Serialize;
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::tasks::{TaskError, TaskStore, WorkStatus, current_millis, invalid_db_text};

/// A reversible association between a Task and a Note (GLOSSARY: Note Link).
/// One row per pair, ever: unlinking marks it, linking again brings the same
/// row and ID back, and neither the Task nor the Note is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLink {
    pub id: String,
    pub task_id: String,
    pub note_id: String,
    pub linked_at: i64,
    pub unlinked_at: Option<i64>,
}

/// A Note as a Task's page lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedNote {
    pub link_id: String,
    pub note_id: String,
    pub title: String,
    pub updated_at: i64,
}

/// A Task as an open Note lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedTask {
    pub link_id: String,
    pub task_id: String,
    pub title: String,
    pub status: WorkStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// Schema v7: Task–Note links. Foreign keys keep every writer of the file from
/// leaving a link to a Task or Note that does not exist.
pub(crate) fn migrate_v7(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE task_note_links (
            id TEXT PRIMARY KEY NOT NULL,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE RESTRICT,
            linked_at INTEGER NOT NULL,
            unlinked_at INTEGER,
            UNIQUE(task_id, note_id)
         );
         CREATE INDEX task_note_links_task_order
           ON task_note_links(task_id, linked_at DESC, id DESC) WHERE unlinked_at IS NULL;
         CREATE INDEX task_note_links_note_order
           ON task_note_links(note_id, linked_at DESC, id DESC) WHERE unlinked_at IS NULL;
         PRAGMA user_version = 7;",
    )?;
    Ok(())
}

const COLUMNS: &str = "id, task_id, note_id, linked_at, unlinked_at";

fn read_link(row: &Row<'_>) -> rusqlite::Result<NoteLink> {
    Ok(NoteLink {
        id: row.get(0)?,
        task_id: row.get(1)?,
        note_id: row.get(2)?,
        linked_at: row.get(3)?,
        unlinked_at: row.get(4)?,
    })
}

fn parse_cursor(value: &str) -> Result<(i64, String), TaskError> {
    crate::search::parse_time_id_cursor(value, "k1", TaskError::InvalidLinkCursor)
}

/// Which way a list of links is read: `owner` is the column holding the
/// Task or Note asked about, `select` and `join` bring in the other side.
struct Side {
    owner: &'static str,
    select: &'static str,
    join: &'static str,
}

const NOTES_OF_TASK: Side = Side {
    owner: "task_id",
    select: "n.id, n.title, n.updated_at",
    // Notes hidden by Removed (their own, or a Removed folder above them) are
    // left out; the link row stays, so restoring brings the Note back here.
    join: concat!("JOIN notes n ON n.id = l.note_id AND ", note_visible!("n")),
};

const TASKS_OF_NOTE: Side = Side {
    owner: "note_id",
    select: "t.id, t.title, t.status",
    // Tasks hidden by Removed (their own, or their List's) are left out; the
    // link row stays, so restoring brings the Task back into this list.
    join: concat!("JOIN tasks t ON t.id = l.task_id AND ", task_visible!("t")),
};

/// One keyset page of active links on one side, most recently linked first.
/// Columns 0 and 1 are the link's `linked_at` and `id`; `read` sees the rest.
/// `shown`, when given, is an extra condition on the joined side.
fn page<T>(
    conn: &Connection,
    side: &Side,
    owner: &str,
    shown: Option<&str>,
    limit: u32,
    cursor: Option<&str>,
    read: impl Fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<LinkPage<T>, TaskError> {
    if !(1..=100).contains(&limit) {
        return Err(TaskError::InvalidPageSize);
    }
    let (time, id) = cursor.map(parse_cursor).transpose()?.unzip();
    let shown = shown.map(|sql| format!("AND {sql}")).unwrap_or_default();
    let mut stmt = conn.prepare(&format!(
        "SELECT l.linked_at, l.id, {} FROM task_note_links l {}
         WHERE l.{} = ?1 AND l.unlinked_at IS NULL {shown} AND {}
         ORDER BY l.linked_at DESC, l.id DESC LIMIT ?4",
        side.select,
        side.join,
        side.owner,
        if time.is_some() {
            "(l.linked_at, l.id) < (?2, ?3)"
        } else {
            "?2 IS NULL AND ?3 IS NULL"
        }
    ))?;
    let mut rows = stmt
        .query_map(params![owner, time, id, i64::from(limit) + 1], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, read(row)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let next_cursor = if rows.len() > limit as usize {
        rows.pop();
        rows.last().map(|(time, id, _)| format!("k1|{time}|{id}"))
    } else {
        None
    };
    Ok(LinkPage {
        items: rows.into_iter().map(|(_, _, item)| item).collect(),
        next_cursor,
    })
}

impl TaskStore {
    /// Link stamps never repeat among the links this interface sees, so lists
    /// follow the order links were made even when several land within one
    /// millisecond. `?4` is the current time. As for Note stamps, an AI
    /// interface stamps over links to shared Notes only, so a private Note's
    /// link time never shows through.
    fn next_link_stamp_sql(&self) -> String {
        format!(
            "SELECT max(?4, coalesce(max(s.linked_at) + 1, 0)) AS stamp
             FROM task_note_links s WHERE {}",
            self.link_shown_sql("s")
        )
    }

    /// Links a Note to a Task. Linking a pair that is already linked returns
    /// that link; a pair unlinked earlier gets its own row back, same ID, with
    /// a fresh stamp so it lists first. Undo is `restore_link`.
    pub fn link_note(&self, task_id: &str, note_id: &str) -> Result<NoteLink, TaskError> {
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        crate::removed::require_writable_task(&tx, task_id)?;
        self.require_note_shown(&tx, note_id)?;
        crate::removed::require_writable_note(&tx, note_id)?;
        let link = tx.query_row(
            &format!(
                "INSERT INTO task_note_links ({COLUMNS})
                 SELECT ?1, ?2, ?3, stamp, NULL FROM ({}) WHERE true
                 ON CONFLICT(task_id, note_id) DO UPDATE SET unlinked_at = NULL,
                   linked_at = CASE WHEN unlinked_at IS NULL THEN linked_at
                                    ELSE excluded.linked_at END
                 RETURNING {COLUMNS}",
                self.next_link_stamp_sql()
            ),
            params![Uuid::new_v4().to_string(), task_id, note_id, now],
            read_link,
        )?;
        self.audit_applied(&tx, AuditAction::LinkCreate, &link.id, None)?;
        tx.commit()?;
        Ok(link)
    }

    /// Ends a link without touching the Task or the Note. `restore_link`
    /// (Undo) brings it back with the same ID and place.
    pub fn unlink_note(&self, link_id: &str) -> Result<NoteLink, TaskError> {
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let link = tx
            .query_row(
                &format!(
                    "UPDATE task_note_links SET unlinked_at = ?2
                     WHERE id = ?1 AND unlinked_at IS NULL AND {}
                     RETURNING {COLUMNS}",
                    self.link_shown_sql("task_note_links")
                ),
                params![link_id, now],
                read_link,
            )
            .optional()?
            .ok_or(TaskError::LinkNotFound)?;
        self.audit_applied(&tx, AuditAction::LinkRemove, &link.id, None)?;
        tx.commit()?;
        Ok(link)
    }

    /// Undo for an unlink: the link comes back with its original stamp, so
    /// it returns to the place it had. An active link is returned unchanged.
    /// An unlinked link stays unlinked while its Task or Note is Removed.
    pub fn restore_link(&self, link_id: &str) -> Result<NoteLink, TaskError> {
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let link = tx
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM task_note_links WHERE id = ?1 AND {}",
                    self.link_shown_sql("task_note_links")
                ),
                params![link_id],
                read_link,
            )
            .optional()?
            // To an AI interface a link to an excluded Note does not exist.
            .ok_or(TaskError::LinkNotFound)?;
        if link.unlinked_at.is_none() {
            return Ok(link);
        }
        crate::removed::require_writable_task(&tx, &link.task_id)?;
        crate::removed::require_writable_note(&tx, &link.note_id)?;
        let link = tx.query_row(
            &format!(
                "UPDATE task_note_links SET unlinked_at = NULL
                 WHERE id = ?1 RETURNING {COLUMNS}"
            ),
            params![link_id],
            read_link,
        )?;
        self.audit_applied(&tx, AuditAction::LinkRestore, &link.id, None)?;
        tx.commit()?;
        Ok(link)
    }

    /// To an AI interface a link to an excluded Note does not exist.
    pub fn get_link(&self, link_id: &str) -> Result<Option<NoteLink>, TaskError> {
        Ok(self
            .connect()?
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM task_note_links WHERE id = ?1 AND {}",
                    self.link_shown_sql("task_note_links")
                ),
                params![link_id],
                read_link,
            )
            .optional()?)
    }

    /// Notes linked to a Task, most recently linked first.
    pub fn list_task_notes(
        &self,
        task_id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<LinkPage<LinkedNote>, TaskError> {
        let conn = self.connect()?;
        let shown = self.note_shown_sql("n");
        page(
            &conn,
            &NOTES_OF_TASK,
            task_id,
            Some(&shown),
            limit,
            cursor,
            |row| {
                Ok(LinkedNote {
                    link_id: row.get(1)?,
                    note_id: row.get(2)?,
                    title: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
    }

    /// Tasks linked to a Note, most recently linked first.
    pub fn list_note_tasks(
        &self,
        note_id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<LinkPage<LinkedTask>, TaskError> {
        let mut conn = self.connect()?;
        // One read transaction: the page comes from the state the check saw.
        // Nothing is written, so dropping it needs no commit.
        let tx = conn.transaction()?;
        // To an AI interface an excluded Note is missing, Tasks and all.
        self.require_note_shown(&tx, note_id)?;
        #[cfg(test)]
        crate::privacy::between_reads();
        page(&tx, &TASKS_OF_NOTE, note_id, None, limit, cursor, |row| {
            let status: String = row.get(4)?;
            Ok(LinkedTask {
                link_id: row.get(1)?,
                task_id: row.get(2)?,
                title: row.get(3)?,
                status: WorkStatus::from_sql(&status).ok_or_else(|| invalid_db_text(4, status))?,
            })
        })
    }
}
