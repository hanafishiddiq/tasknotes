//! Nested Note Folders: a tree independent of Task Lists, at most five levels
//! deep, with case-insensitively unique names among siblings.

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::removed::SetRemoved;
use crate::tasks::{TaskError, TaskStore, current_millis};

/// A top-level folder is level 1.
pub const MAX_FOLDER_DEPTH: i64 = 5;
const MAX_NAME_CHARS: usize = 100;
/// Recursion guard for walks over a damaged tree; far above any valid depth.
const WALK_LIMIT: i64 = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFolder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Whether the sidebar can expand this folder.
    pub has_children: bool,
    /// In Removed (ADR-0002); its subfolders and Notes are hidden with it.
    pub removed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteFolder {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameNoteFolder {
    pub id: String,
    pub expected_revision: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveNoteFolder {
    pub id: String,
    pub expected_revision: i64,
    /// None moves the folder to the top level.
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFolderPage {
    pub items: Vec<NoteFolder>,
    pub next_cursor: Option<String>,
}

/// Schema v8, folder part: the folder tree. The same step adds Note placement
/// and search in `notes::migrate_v8` and re-folds Tag names in
/// `organization::refold_tags`; the runner in `tasks.rs` runs all three in one
/// transaction.
pub(crate) fn migrate_v8(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(&format!(
        "CREATE TABLE note_folders (
            id TEXT PRIMARY KEY NOT NULL,
            parent_id TEXT REFERENCES note_folders(id) ON DELETE RESTRICT,
            name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND {MAX_NAME_CHARS}),
            name_fold TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision >= 1),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            CHECK(parent_id IS NULL OR parent_id <> id)
         );
         CREATE UNIQUE INDEX note_folders_sibling_name
           ON note_folders(coalesce(parent_id, ''), name_fold);"
    ))?;
    Ok(())
}

fn folder_name(value: &str) -> Result<&str, TaskError> {
    let name = value.trim();
    if name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(TaskError::InvalidNoteFolderName);
    }
    Ok(name)
}

/// A folder name's identity among its siblings (see `text_fold`).
fn name_fold(name: &str) -> String {
    crate::text_fold::fold(name)
}

const FOLDER_COLUMNS: &str = "f.id, f.parent_id, f.name, f.revision, f.created_at, f.updated_at,
    EXISTS(SELECT 1 FROM note_folders c WHERE coalesce(c.parent_id, '') = f.id
      AND c.removed_at IS NULL), f.removed_at";

fn read_folder(row: &Row<'_>) -> rusqlite::Result<NoteFolder> {
    Ok(NoteFolder {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        revision: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        has_children: row.get(6)?,
        removed: row.get::<_, Option<i64>>(7)?.is_some(),
    })
}

fn get_folder(conn: &Connection, id: &str) -> Result<Option<NoteFolder>, TaskError> {
    if Uuid::parse_str(id).is_err() {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!("SELECT {FOLDER_COLUMNS} FROM note_folders f WHERE f.id = ?1"),
            params![id],
            read_folder,
        )
        .optional()?)
}

/// Whether the folder holds any subfolder or Note, in Removed or not, and
/// whatever the Note's Exclude from AI flag.
pub(crate) fn folder_holds_anything(conn: &Connection, id: &str) -> Result<bool, TaskError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM note_folders WHERE parent_id = ?1)
             OR EXISTS(SELECT 1 FROM notes WHERE folder_id = ?1)",
        params![id],
        |row| row.get(0),
    )?)
}

/// The folder's own level plus every ancestor: a top-level folder is 1.
/// The walk also lists the ancestors, which is how a cycle is detected.
fn ancestors(conn: &Connection, id: &str) -> Result<Vec<String>, TaskError> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE up(id, parent_id, level) AS (
            SELECT id, parent_id, 1 FROM note_folders WHERE id = ?1
            UNION ALL
            SELECT f.id, f.parent_id, up.level + 1
              FROM note_folders f JOIN up ON f.id = up.parent_id
             WHERE up.level < ?2
         ) SELECT id FROM up",
    )?;
    let ids = stmt
        .query_map(params![id, WALK_LIMIT], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

/// Levels in the subtree rooted at `id`, the folder itself counting as 1.
fn subtree_height(conn: &Connection, id: &str) -> Result<i64, TaskError> {
    Ok(conn.query_row(
        "WITH RECURSIVE down(id, level) AS (
            SELECT id, 1 FROM note_folders WHERE id = ?1
            UNION ALL
            SELECT f.id, down.level + 1
              FROM note_folders f JOIN down ON coalesce(f.parent_id, '') = down.id
             WHERE down.level < ?2
         ) SELECT max(level) FROM down",
        params![id, WALK_LIMIT],
        |row| row.get(0),
    )?)
}

fn require_unique_name(
    conn: &Connection,
    parent_id: Option<&str>,
    fold: &str,
    except: Option<&str>,
) -> Result<(), TaskError> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM note_folders
          WHERE coalesce(parent_id, '') = coalesce(?1, '') AND name_fold = ?2
            AND id IS NOT coalesce(?3, '') AND removed_at IS NULL)",
        params![parent_id, fold, except],
        |row| row.get(0),
    )?;
    if taken {
        return Err(TaskError::NoteFolderNameConflict);
    }
    Ok(())
}

/// Verifies a folder that is to be shown or written into, inside the
/// caller's transaction: it exists and neither it nor a folder above it is
/// in Removed.
pub(crate) fn require_folder(conn: &Connection, id: &str) -> Result<(), TaskError> {
    match get_folder(conn, id)? {
        None => Err(TaskError::NoteFolderNotFound),
        Some(_) if crate::removed::folder_hidden(conn, id)? => Err(TaskError::NoteFolderRemoved),
        Some(_) => Ok(()),
    }
}

/// The folder as stored, provided nobody changed it since `expected_revision`.
fn current_folder(
    conn: &Connection,
    id: &str,
    expected_revision: i64,
) -> Result<NoteFolder, TaskError> {
    require_folder(conn, id)?;
    let folder = get_folder(conn, id)?.ok_or(TaskError::NoteFolderNotFound)?;
    if folder.revision != expected_revision {
        return Err(TaskError::NoteFolderRevisionConflict);
    }
    Ok(folder)
}

fn write_folder(
    conn: &Connection,
    old: &NoteFolder,
    parent_id: Option<&str>,
    name: &str,
) -> Result<NoteFolder, TaskError> {
    let now = current_millis()?;
    conn.execute(
        "UPDATE note_folders SET parent_id = ?1, name = ?2, name_fold = ?3,
           revision = revision + 1, updated_at = max(updated_at + 1, ?4)
         WHERE id = ?5 AND revision = ?6",
        params![parent_id, name, name_fold(name), now, old.id, old.revision],
    )?;
    get_folder(conn, &old.id)?.ok_or(TaskError::NoteFolderNotFound)
}

fn parse_cursor(value: &str, parent_key: &str) -> Result<(String, String), TaskError> {
    let mut fields = value.splitn(4, '|');
    match (fields.next(), fields.next(), fields.next(), fields.next()) {
        (Some("f1"), Some(parent), Some(id), Some(fold))
            if parent == parent_key && Uuid::parse_str(id).is_ok() =>
        {
            Ok((fold.to_owned(), id.to_owned()))
        }
        _ => Err(TaskError::InvalidNoteFolderCursor),
    }
}

impl TaskStore {
    pub fn create_note_folder(&self, input: CreateNoteFolder) -> Result<NoteFolder, TaskError> {
        let name = folder_name(&input.name)?;
        let fold = name_fold(name);
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        if let Some(parent) = input.parent_id.as_deref() {
            require_folder(&tx, parent)?;
            if ancestors(&tx, parent)?.len() as i64 >= MAX_FOLDER_DEPTH {
                return Err(TaskError::NoteFolderTooDeep);
            }
        }
        require_unique_name(&tx, input.parent_id.as_deref(), &fold, None)?;
        let now = current_millis()?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO note_folders(id, parent_id, name, name_fold, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
            params![id, input.parent_id, name, fold, now],
        )?;
        let created = get_folder(&tx, &id)?.ok_or(TaskError::NoteFolderNotFound)?;
        self.audit_applied(
            &tx,
            AuditAction::FolderCreate,
            &created.id,
            created.revision,
        )?;
        tx.commit()?;
        Ok(created)
    }

    pub fn get_note_folder(&self, id: &str) -> Result<Option<NoteFolder>, TaskError> {
        get_folder(&self.connect()?, id)
    }

    pub fn rename_note_folder(&self, input: RenameNoteFolder) -> Result<NoteFolder, TaskError> {
        self.audited(AuditAction::FolderRename, input, |input| {
            self.rename_note_folder_inner(input)
        })
    }

    fn rename_note_folder_inner(&self, input: RenameNoteFolder) -> Result<NoteFolder, TaskError> {
        let name = folder_name(&input.name)?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let old = current_folder(&tx, &input.id, input.expected_revision)?;
        if old.name == name {
            return Err(TaskError::NoteFolderUnchanged);
        }
        require_unique_name(
            &tx,
            old.parent_id.as_deref(),
            &name_fold(name),
            Some(&old.id),
        )?;
        let renamed = write_folder(&tx, &old, old.parent_id.as_deref(), name)?;
        self.audit_applied(
            &tx,
            AuditAction::FolderRename,
            &renamed.id,
            renamed.revision,
        )?;
        tx.commit()?;
        Ok(renamed)
    }

    /// Moves a folder, with its subfolders and Notes, under another folder or
    /// to the top level. Refused when the folder would become its own ancestor
    /// or any folder in its subtree would sit below level 5.
    pub fn move_note_folder(&self, input: MoveNoteFolder) -> Result<NoteFolder, TaskError> {
        self.audited(AuditAction::FolderMove, input, |input| {
            self.move_note_folder_inner(input)
        })
    }

    fn move_note_folder_inner(&self, input: MoveNoteFolder) -> Result<NoteFolder, TaskError> {
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let old = current_folder(&tx, &input.id, input.expected_revision)?;
        if old.parent_id == input.parent_id {
            return Err(TaskError::NoteFolderUnchanged);
        }
        if let Some(parent) = input.parent_id.as_deref() {
            require_folder(&tx, parent)?;
            let path = ancestors(&tx, parent)?;
            if path.contains(&old.id) {
                return Err(TaskError::NoteFolderCycle);
            }
            if path.len() as i64 + subtree_height(&tx, &old.id)? > MAX_FOLDER_DEPTH {
                return Err(TaskError::NoteFolderTooDeep);
            }
        }
        require_unique_name(
            &tx,
            input.parent_id.as_deref(),
            &name_fold(&old.name),
            Some(&old.id),
        )?;
        let moved = write_folder(&tx, &old, input.parent_id.as_deref(), &old.name)?;
        self.audit_applied(&tx, AuditAction::FolderMove, &moved.id, moved.revision)?;
        tx.commit()?;
        Ok(moved)
    }

    /// One bounded page of a folder's direct subfolders (or the top level),
    /// sorted by name ignoring case.
    pub fn list_note_folders(
        &self,
        parent_id: Option<&str>,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<NoteFolderPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let parent_key = parent_id.unwrap_or("");
        let after = cursor.map(|c| parse_cursor(c, parent_key)).transpose()?;
        let (fold, id) = after.unzip();
        let conn = self.connect()?;
        if let Some(parent) = parent_id {
            require_folder(&conn, parent)?;
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT {FOLDER_COLUMNS}, f.name_fold FROM note_folders f
             WHERE coalesce(f.parent_id, '') = ?1 AND f.removed_at IS NULL
               AND (?2 IS NULL OR (f.name_fold, f.id) > (?2, ?3))
             ORDER BY f.name_fold, f.id LIMIT ?4"
        ))?;
        let mut rows = stmt.query(params![parent_key, fold, id, i64::from(limit) + 1])?;
        let mut items = Vec::new();
        let mut folds = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(read_folder(row)?);
            folds.push(row.get::<_, String>(8)?);
        }
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|last| format!("f1|{parent_key}|{}|{}", last.id, folds[items.len() - 1]))
        } else {
            None
        };
        Ok(NoteFolderPage { items, next_cursor })
    }

    /// The folder's names from the top level down, joined with `/`, as AI
    /// interfaces show where a Note lives (owner decision 2026-10-25). A
    /// name may itself contain `/`; the folder id is the stable reference.
    pub fn note_folder_path(&self, id: &str) -> Result<String, TaskError> {
        let conn = self.connect()?;
        if get_folder(&conn, id)?.is_none() {
            return Err(TaskError::NoteFolderNotFound);
        }
        let mut stmt = conn.prepare(
            "WITH RECURSIVE up(id, parent_id, name, level) AS (
                SELECT id, parent_id, name, 1 FROM note_folders WHERE id = ?1
                UNION ALL
                SELECT f.id, f.parent_id, f.name, up.level + 1
                  FROM note_folders f JOIN up ON f.id = up.parent_id
                 WHERE up.level < ?2
             ) SELECT name FROM up ORDER BY level DESC",
        )?;
        let names = stmt
            .query_map(params![id, WALK_LIMIT], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(names.join("/"))
    }

    /// How many Notes this interface would list directly in the folder (or
    /// in no folder): not Removed, none while the folder is hidden by Removed, and for an AI interface never a Note
    /// marked Exclude from AI, so a count cannot reveal one.
    pub fn count_folder_notes(&self, folder_id: Option<&str>) -> Result<i64, TaskError> {
        let conn = self.connect()?;
        if let Some(id) = folder_id {
            if get_folder(&conn, id)?.is_none() {
                return Err(TaskError::NoteFolderNotFound);
            }
            // A folder hidden by Removed hides its Notes with it.
            if crate::removed::folder_hidden(&conn, id)? {
                return Ok(0);
            }
        }
        Ok(conn.query_row(
            &format!(
                "SELECT count(*) FROM notes n
                 WHERE n.folder_id IS ?1 AND n.removed_at IS NULL AND {}",
                self.note_shown_sql("n")
            ),
            params![folder_id],
            |row| row.get(0),
        )?)
    }

    /// Removes or restores a folder. Its subfolders and Notes are hidden with
    /// it but keep their own disposition, so restoring it leaves anything
    /// Removed on its own in Removed. A folder below a Removed folder cannot
    /// be removed or restored on its own, nor restored over an active
    /// sibling's name.
    pub fn set_note_folder_removed(&self, input: SetRemoved) -> Result<NoteFolder, TaskError> {
        let action = if input.removed {
            AuditAction::FolderRemove
        } else {
            AuditAction::FolderRestore
        };
        self.audited(action, input, |input| {
            self.set_note_folder_removed_inner(input, action)
        })
    }

    fn set_note_folder_removed_inner(
        &self,
        input: SetRemoved,
        action: AuditAction,
    ) -> Result<NoteFolder, TaskError> {
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let removing = input.removed;
        let changed = self.set_note_folder_removed_in(&tx, input, action)?;
        // Owner decision B: through an AI interface, a folder holding any
        // subfolder or Note waits for approval (`bulk`). Notes marked Exclude
        // from AI count too, and the refusal never says how many there are.
        if removing && self.needs_approval_to_empty() && folder_holds_anything(&tx, &changed.id)? {
            return Err(TaskError::NeedsApproval);
        }
        tx.commit()?;
        Ok(changed)
    }

    /// The change inside the caller's write transaction (also used by an
    /// approved Bulk Change); the caller commits.
    pub(crate) fn set_note_folder_removed_in(
        &self,
        tx: &Transaction,
        input: SetRemoved,
        action: AuditAction,
    ) -> Result<NoteFolder, TaskError> {
        let old = get_folder(tx, &input.id)?.ok_or(TaskError::NoteFolderNotFound)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::NoteFolderRevisionConflict);
        }
        if old.removed == input.removed {
            return Err(TaskError::RemovedUnchanged);
        }
        // Below a Removed folder this one is hidden with it; that folder
        // comes first, whether restoring or removing this one on its own.
        if let Some(parent) = old.parent_id.as_deref() {
            require_folder(tx, parent)?;
        }
        if !input.removed {
            require_unique_name(
                tx,
                old.parent_id.as_deref(),
                &name_fold(&old.name),
                Some(&old.id),
            )?;
        }
        let now = current_millis()?;
        tx.execute(
            "UPDATE note_folders SET removed_at = ?1, revision = revision + 1,
               updated_at = max(updated_at + 1, ?2)
             WHERE id = ?3 AND revision = ?4",
            params![input.removed.then_some(now), now, old.id, old.revision],
        )?;
        let changed = get_folder(tx, &old.id)?.ok_or(TaskError::NoteFolderNotFound)?;
        self.audit_applied(tx, action, &changed.id, changed.revision)?;
        Ok(changed)
    }
}
