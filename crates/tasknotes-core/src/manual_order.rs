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

impl Siblings<'_> {
    const fn table(self) -> &'static str {
        match self {
            Self::Lists => "lists",
            Self::Folders { .. } => "note_folders",
        }
    }

    /// `(id, position, active)` of every sibling, in order.
    fn rows(self, conn: &Connection) -> Result<Vec<(String, i64, bool)>, TaskError> {
        let filter = match self {
            Self::Lists => "?1 IS NULL",
            Self::Folders { .. } => "coalesce(parent_id, '') = coalesce(?1, '')",
        };
        let parent_id = match self {
            Self::Lists => None,
            Self::Folders { parent_id } => parent_id,
        };
        let mut statement = conn.prepare(&format!(
            "SELECT id, position, removed_at IS NULL FROM {} WHERE {filter}
             ORDER BY position, id",
            self.table()
        ))?;
        let rows = statement
            .query_map(params![parent_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}

/// Schema v16: Lists keep showing newest first and each folder's children in
/// name order, as they did, until the person moves one.
pub(crate) fn migrate_v16(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE lists ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
         UPDATE lists SET position = ranked.place FROM (
             SELECT id, row_number() OVER (ORDER BY created_at DESC, id DESC) AS place FROM lists
         ) AS ranked WHERE lists.id = ranked.id;
         DROP INDEX IF EXISTS lists_created_order;
         CREATE INDEX lists_manual_order ON lists(position, id);
         ALTER TABLE note_folders ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
         UPDATE note_folders SET position = ranked.place FROM (
             SELECT id, row_number() OVER (
                 PARTITION BY coalesce(parent_id, '') ORDER BY name_fold, id
             ) AS place FROM note_folders
         ) AS ranked WHERE note_folders.id = ranked.id;
         CREATE INDEX note_folders_manual_order
           ON note_folders(coalesce(parent_id, ''), position, id);
         PRAGMA user_version = 16;",
    )?;
    Ok(())
}

/// The position that puts a new sibling at the bottom.
pub(crate) fn bottom(conn: &Connection, siblings: Siblings) -> Result<i64, TaskError> {
    let last = siblings.rows(conn)?.last().map_or(0, |row| row.1);
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
        return if rows.iter().any(|row| row.0 == id) {
            Ok(false)
        } else {
            Err(TaskError::OrderAnchorChanged)
        };
    }
    let active_before: Vec<&str> = rows
        .iter()
        .filter(|row| row.2)
        .map(|row| row.0.as_str())
        .collect();
    let order: Vec<&(String, i64, bool)> = rows.iter().filter(|row| row.0 != id).collect();
    let at = match after_id {
        None => 0,
        Some(anchor) => {
            order
                .iter()
                .position(|row| row.2 && row.0 == anchor)
                .ok_or(TaskError::OrderAnchorChanged)?
                + 1
        }
    };
    let mut active_after: Vec<&str> = order
        .iter()
        .take(at)
        .filter(|row| row.2)
        .map(|row| row.0.as_str())
        .collect();
    active_after.push(id);
    active_after.extend(
        order
            .iter()
            .skip(at)
            .filter(|row| row.2)
            .map(|row| row.0.as_str()),
    );
    if active_before == active_after {
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
        if row.1 != wanted {
            update.execute(params![wanted, row.0])?;
        }
    }
    Ok(true)
}
