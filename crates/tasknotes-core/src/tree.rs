//! The one tree rule Lists and Note Folders share (ADR 0009): a row may sit
//! under another of its kind, never below itself, and never deeper than five
//! levels. Both tables keep the tree the same way, in a nullable `parent_id`
//! naming a row of the same table, so one walk serves both; each kind keeps
//! its own refusals, so an interface still says "List" or "folder".

use rusqlite::{Connection, params};

use crate::tasks::TaskError;

/// A top-level List or folder is level 1.
pub const MAX_DEPTH: i64 = 5;
/// Recursion guard for walks over a damaged tree; far above any valid depth.
pub(crate) const WALK_LIMIT: i64 = 64;

/// Which tree a row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tree {
    Lists,
    NoteFolders,
}

impl Tree {
    pub(crate) const fn table(self) -> &'static str {
        match self {
            Self::Lists => "lists",
            Self::NoteFolders => "note_folders",
        }
    }

    fn cycle(self) -> TaskError {
        match self {
            Self::Lists => TaskError::ListCycle,
            Self::NoteFolders => TaskError::NoteFolderCycle,
        }
    }

    fn too_deep(self) -> TaskError {
        match self {
            Self::Lists => TaskError::ListTooDeep,
            Self::NoteFolders => TaskError::NoteFolderTooDeep,
        }
    }

    /// The row's own level plus every ancestor, nearest first: a top-level
    /// row is a path of one. The walk is also how a cycle is found.
    pub(crate) fn path_up(self, conn: &Connection, id: &str) -> Result<Vec<String>, TaskError> {
        let table = self.table();
        let mut stmt = conn.prepare(&format!(
            "WITH RECURSIVE up(id, parent_id, level) AS (
                SELECT id, parent_id, 1 FROM {table} WHERE id = ?1
                UNION ALL
                SELECT t.id, t.parent_id, up.level + 1
                  FROM {table} t JOIN up ON t.id = up.parent_id
                 WHERE up.level < ?2
             ) SELECT id FROM up ORDER BY level"
        ))?;
        let ids = stmt
            .query_map(params![id, WALK_LIMIT], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(ids)
    }

    /// Levels in the subtree rooted at `id`, the row itself counting as 1.
    fn subtree_height(self, conn: &Connection, id: &str) -> Result<i64, TaskError> {
        let table = self.table();
        Ok(conn.query_row(
            &format!(
                "WITH RECURSIVE down(id, level) AS (
                    SELECT id, 1 FROM {table} WHERE id = ?1
                    UNION ALL
                    SELECT t.id, down.level + 1
                      FROM {table} t JOIN down ON t.parent_id = down.id
                     WHERE down.level < ?2
                 ) SELECT max(level) FROM down"
            ),
            params![id, WALK_LIMIT],
            |row| row.get(0),
        )?)
    }

    /// Refuses a new child under `parent` when the parent already sits at
    /// the deepest level.
    pub(crate) fn require_room_under(
        self,
        conn: &Connection,
        parent: &str,
    ) -> Result<(), TaskError> {
        if self.path_up(conn, parent)?.len() as i64 >= MAX_DEPTH {
            return Err(self.too_deep());
        }
        Ok(())
    }

    /// Refuses moving `id`, with everything below it, under `parent`: it
    /// would become its own ancestor, or a row in its subtree would sit
    /// deeper than `MAX_DEPTH`. The caller checks that `parent` is shown.
    pub(crate) fn require_fits_under(
        self,
        conn: &Connection,
        id: &str,
        parent: &str,
    ) -> Result<(), TaskError> {
        let path = self.path_up(conn, parent)?;
        if path.iter().any(|ancestor| ancestor == id) {
            return Err(self.cycle());
        }
        if path.len() as i64 + self.subtree_height(conn, id)? > MAX_DEPTH {
            return Err(self.too_deep());
        }
        Ok(())
    }
}
