//! Manual order of Lists and Note Folders (ADR-0007): where one sits among
//! its siblings, the same rule for both. Siblings keep the order of their
//! `position`, the id settling ties; a List or Folder in Removed keeps its
//! place, so restoring it brings it back there.

use rusqlite::{Connection, Transaction, params};

use crate::tasks::TaskError;

/// The rows that are ordered against each other.
#[derive(Clone, Copy)]
pub(crate) enum Siblings<'a> {
    Lists,
    /// The children of one folder, or the top level.
    Folders {
        parent_id: Option<&'a str>,
    },
}

/// One sibling as stored.
struct Sibling {
    id: String,
    position: i64,
    /// Not in Removed: what a person sees.
    active: bool,
}

impl Siblings<'_> {
    const fn table(self) -> &'static str {
        match self {
            Self::Lists => "lists",
            Self::Folders { .. } => "note_folders",
        }
    }

    /// Every sibling, in order, Removed ones in their places.
    fn rows(self, conn: &Connection) -> Result<Vec<Sibling>, TaskError> {
        let read = |row: &rusqlite::Row<'_>| {
            Ok(Sibling {
                id: row.get(0)?,
                position: row.get(1)?,
                active: row.get(2)?,
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

/// Puts `id` directly after the active sibling `after_id`, or first among
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
            Err(TaskError::OrderAnchorChanged)
        };
    }
    let order: Vec<&Sibling> = rows.iter().filter(|row| row.id != id).collect();
    let at = match after_id {
        None => 0,
        Some(anchor) => {
            order
                .iter()
                .position(|row| row.active && row.id == anchor)
                .ok_or(TaskError::OrderAnchorChanged)?
                + 1
        }
    };
    let shown = |row: &&&Sibling| row.active;
    let active_before = rows
        .iter()
        .filter(|row| row.active)
        .map(|row| row.id.as_str());
    let active_after = order[..at]
        .iter()
        .filter(shown)
        .map(|row| row.id.as_str())
        .chain([id])
        .chain(order[at..].iter().filter(shown).map(|row| row.id.as_str()));
    if active_before.eq(active_after) {
        return Ok(false);
    }
    let mut update = tx.prepare(&format!(
        "UPDATE {} SET position = ?1 WHERE id = ?2",
        siblings.table()
    ))?;
    let moved_to = i64::try_from(at).unwrap_or(i64::MAX) + 1;
    update.execute(params![moved_to, id])?;
    for (index, row) in order.iter().enumerate() {
        let wanted = i64::try_from(index).unwrap_or(i64::MAX) + if index < at { 1 } else { 2 };
        if row.position != wanted {
            update.execute(params![wanted, row.id])?;
        }
    }
    Ok(true)
}
