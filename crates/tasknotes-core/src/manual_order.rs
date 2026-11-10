//! Manual order of Lists and Note Folders (ADR-0007), and the Custom Order
//! of a List's Tasks (ADR 0010): where one sits among its siblings, the same
//! rule for all. Siblings keep the order of their `position`, the id
//! settling ties; one in Removed keeps its place, so restoring it brings it
//! back there.

use rusqlite::{Connection, Transaction, params};

use crate::tasks::{TaskError, WorkStatus};

/// The rows that are ordered against each other.
#[derive(Clone, Copy)]
pub(crate) enum Siblings<'a> {
    Lists,
    /// The children of one folder, or the top level.
    Folders {
        parent_id: Option<&'a str>,
    },
    /// Every Task of one List (or Inbox, `None`), numbered as one sequence;
    /// only the shown Tasks of one work-status group are the ones a move
    /// is placed among.
    Tasks {
        list_id: Option<&'a str>,
        status: WorkStatus,
    },
}

/// One sibling as stored.
struct Sibling {
    id: String,
    position: i64,
    /// One a move may be placed beside, and whose order a person sees:
    /// not in Removed and, for a Task, in the moving Task's work-status
    /// group. Only these can be an anchor, and only their order changing
    /// counts as a move.
    anchorable: bool,
}

impl Siblings<'_> {
    const fn table(self) -> &'static str {
        match self {
            Self::Lists => "lists",
            Self::Folders { .. } => "note_folders",
            Self::Tasks { .. } => "tasks",
        }
    }

    /// The error for an anchor that is gone, in Removed or elsewhere.
    const fn anchor_changed(self) -> TaskError {
        match self {
            Self::Tasks { .. } => TaskError::TaskOrderAnchorChanged,
            _ => TaskError::OrderAnchorChanged,
        }
    }

    /// Where renumbering starts. Lists and folders count from 1. Tasks keep
    /// the lowest position they have, since a new Task takes one below it:
    /// a move then renumbers only the Tasks it passes.
    fn first_position(self, rows: &[Sibling]) -> i64 {
        match self {
            Self::Tasks { .. } => rows.first().map_or(1, |row| row.position),
            _ => 1,
        }
    }

    /// Every sibling, in order, Removed ones in their places.
    fn rows(self, conn: &Connection) -> Result<Vec<Sibling>, TaskError> {
        let read = |row: &rusqlite::Row<'_>| {
            Ok(Sibling {
                id: row.get(0)?,
                position: row.get(1)?,
                anchorable: row.get(2)?,
            })
        };
        let rows = match self {
            Self::Lists => conn
                .prepare(
                    "SELECT id, position, removed_at IS NULL FROM lists ORDER BY position, id",
                )?
                .query_map([], read)?
                .collect::<rusqlite::Result<_>>()?,
            Self::Folders { parent_id } => conn
                .prepare(
                    "SELECT id, position, removed_at IS NULL FROM note_folders
                     WHERE coalesce(parent_id, '') = coalesce(?1, '') ORDER BY position, id",
                )?
                .query_map(params![parent_id], read)?
                .collect::<rusqlite::Result<_>>()?,
            Self::Tasks { list_id, status } => conn
                .prepare(
                    "SELECT id, position, removed_at IS NULL AND status = ?2 FROM tasks
                     WHERE list_id IS ?1 ORDER BY position, id",
                )?
                .query_map(params![list_id, status.as_sql()], read)?
                .collect::<rusqlite::Result<_>>()?,
        };
        Ok(rows)
    }
}

/// Schema v16: Lists keep showing newest first and each folder's children in
/// name order, as they did, until the person moves one.
pub(crate) fn migrate_v16(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE lists ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
         UPDATE lists SET position = ordered.place FROM (
             SELECT id, row_number() OVER (ORDER BY created_at DESC, id DESC) AS place FROM lists
         ) AS ordered WHERE lists.id = ordered.id;
         DROP INDEX IF EXISTS lists_created_order;
         CREATE INDEX lists_manual_order ON lists(position, id);
         ALTER TABLE note_folders ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
         UPDATE note_folders SET position = ordered.place FROM (
             SELECT id, row_number() OVER (
                 PARTITION BY coalesce(parent_id, '') ORDER BY name_fold, id
             ) AS place FROM note_folders
         ) AS ordered WHERE note_folders.id = ordered.id;
         CREATE INDEX note_folders_manual_order
           ON note_folders(coalesce(parent_id, ''), position, id);
         PRAGMA user_version = 16;",
    )?;
    Ok(())
}

/// The position that puts a new sibling at the bottom.
pub(crate) fn bottom(conn: &Connection, siblings: Siblings) -> Result<i64, TaskError> {
    let last = siblings.rows(conn)?.last().map_or(0, |row| row.position);
    Ok(last + 1)
}

/// The anchor that puts `id` directly before the anchorable sibling
/// `before_id`: the anchorable sibling before it, or `None` (first).
pub(crate) fn after_for_before(
    conn: &Connection,
    siblings: Siblings,
    id: &str,
    before_id: &str,
) -> Result<Option<String>, TaskError> {
    let rows = siblings.rows(conn)?;
    let shown: Vec<&Sibling> = rows
        .iter()
        .filter(|row| row.anchorable && row.id != id)
        .collect();
    let at = shown
        .iter()
        .position(|row| row.id == before_id)
        .ok_or(siblings.anchor_changed())?;
    Ok(at.checked_sub(1).map(|before| shown[before].id.clone()))
}

/// Puts `id` directly after the anchorable sibling `after_id`, or first among
/// all siblings without one. `id` need not be among `siblings` yet: a folder
/// moving in from another parent. Only positions change, so a neighbour's
/// open edit never conflicts with something moving past it. Returns whether
/// the order a person sees changed; the caller rejects a move that did not.
pub(crate) fn place(
    tx: &Transaction,
    siblings: Siblings,
    id: &str,
    after_id: Option<&str>,
) -> Result<bool, TaskError> {
    let rows = siblings.rows(tx)?;
    if after_id == Some(id) {
        return if rows.iter().any(|row| row.id == id) {
            Ok(false)
        } else {
            Err(siblings.anchor_changed())
        };
    }
    let order: Vec<&Sibling> = rows.iter().filter(|row| row.id != id).collect();
    let at = match after_id {
        None => 0,
        Some(anchor) => {
            order
                .iter()
                .position(|row| row.anchorable && row.id == anchor)
                .ok_or(siblings.anchor_changed())?
                + 1
        }
    };
    let shown = |row: &&&Sibling| row.anchorable;
    let shown_before = rows
        .iter()
        .filter(|row| row.anchorable)
        .map(|row| row.id.as_str());
    let shown_after = order[..at]
        .iter()
        .filter(shown)
        .map(|row| row.id.as_str())
        .chain([id])
        .chain(order[at..].iter().filter(shown).map(|row| row.id.as_str()));
    if shown_before.eq(shown_after) {
        return Ok(false);
    }
    let mut update = tx.prepare(&format!(
        "UPDATE {} SET position = ?1 WHERE id = ?2",
        siblings.table()
    ))?;
    // Positions count up in the new order; the moved row has no stored
    // position among these siblings worth keeping.
    let stored = |row: &&Sibling| (row.id.clone(), Some(row.position));
    let new_order = order[..at]
        .iter()
        .map(stored)
        .chain([(id.to_owned(), None)])
        .chain(order[at..].iter().map(stored));
    for (wanted, (row_id, position)) in (siblings.first_position(&rows)..).zip(new_order) {
        if position != Some(wanted) {
            update.execute(params![wanted, row_id])?;
        }
    }
    Ok(true)
}
