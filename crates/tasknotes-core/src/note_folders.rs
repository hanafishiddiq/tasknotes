//! Nested Note Folders: a tree independent of Task Lists, at most five levels
//! deep, with case-insensitively unique names among siblings.

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::manual_order::{self, Siblings};
use crate::removed::SetRemoved;
use crate::style::Style;
use crate::tasks::{TaskError, TaskStore, current_millis};
use crate::tree::{Tree, WALK_LIMIT};

/// A top-level folder is level 1; the same limit as Lists (`tree`).
pub const MAX_FOLDER_DEPTH: i64 = crate::tree::MAX_DEPTH;
const MAX_NAME_CHARS: usize = 100;

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
    /// Its icon and colour; serialised as top-level `icon` and `color`.
    #[serde(flatten)]
    pub style: Style,
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

/// Puts a folder under a parent, directly after a sibling there or first
/// without one, in one step (ADR-0007).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceNoteFolder {
    pub id: String,
    pub expected_revision: i64,
    /// None is the top level.
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub after_id: Option<String>,
}

/// Puts a folder directly after a sibling, or first, under the parent an
/// interface names, or under the one it has when the write happens if it
/// names none. The one entry point for the CLI and MCP (ADR-0007).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderNoteFolder {
    pub id: String,
    pub expected_revision: i64,
    #[serde(default)]
    pub after_id: Option<String>,
    #[serde(default)]
    pub parent: FolderParent,
}

/// Which parent a placed folder ends up under.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderParent {
    /// The parent it has when the write happens.
    #[default]
    Current,
    /// The top level.
    Top,
    /// Under this folder.
    Folder(String),
}

impl FolderParent {
    /// A parent named as an interface does: a folder id, or None for the top.
    pub fn named(parent_id: Option<String>) -> Self {
        parent_id.map_or(Self::Top, Self::Folder)
    }

    /// The parent id this resolves to for a folder now under `current`.
    fn resolve<'a>(&'a self, current: Option<&'a str>) -> Option<&'a str> {
        match self {
            Self::Current => current,
            Self::Top => None,
            Self::Folder(id) => Some(id),
        }
    }
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
      AND c.removed_at IS NULL), f.removed_at, f.icon, f.color";
/// Where the first column a query selects after `FOLDER_COLUMNS` sits.
const FIRST_EXTRA_FOLDER_COLUMN: usize = 10;

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
        style: Style::read(row, 8)?,
    })
}

pub(crate) fn get_folder(conn: &Connection, id: &str) -> Result<Option<NoteFolder>, TaskError> {
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
pub(crate) fn current_folder(
    conn: &Connection,
    id: &str,
    expected_revision: i64,
) -> Result<NoteFolder, TaskError> {
    let folder = get_folder(conn, id)?.ok_or(TaskError::NoteFolderNotFound)?;
    require_current(conn, &folder, expected_revision)?;
    Ok(folder)
}

/// Refuses a write to `folder`, as just read, when it is hidden by Removed
/// or somebody changed it since `expected_revision`.
fn require_current(
    conn: &Connection,
    folder: &NoteFolder,
    expected_revision: i64,
) -> Result<(), TaskError> {
    if crate::removed::folder_hidden(conn, &folder.id)? {
        return Err(TaskError::NoteFolderRemoved);
    }
    if folder.revision != expected_revision {
        return Err(TaskError::NoteFolderRevisionConflict);
    }
    Ok(())
}

/// Whether `folder` may live under `parent_id`: the parent is shown, the
/// folder would not become its own ancestor or sit below level 5, and no
/// active sibling there has its name.
fn require_destination(
    conn: &Connection,
    folder: &NoteFolder,
    parent_id: Option<&str>,
) -> Result<(), TaskError> {
    if let Some(parent) = parent_id {
        require_folder(conn, parent)?;
        Tree::NoteFolders.require_fits_under(conn, &folder.id, parent)?;
    }
    require_unique_name(conn, parent_id, &name_fold(&folder.name), Some(&folder.id))
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

fn parse_cursor(value: &str, parent_key: &str) -> Result<(i64, String), TaskError> {
    let mut fields = value.splitn(4, '|');
    match (fields.next(), fields.next(), fields.next(), fields.next()) {
        (Some("f2"), Some(parent), Some(position), Some(id))
            if parent == parent_key && Uuid::parse_str(id).is_ok() =>
        {
            let position = position
                .parse()
                .map_err(|_| TaskError::InvalidNoteFolderCursor)?;
            Ok((position, id.to_owned()))
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
            Tree::NoteFolders.require_room_under(&tx, parent)?;
        }
        require_unique_name(&tx, input.parent_id.as_deref(), &fold, None)?;
        let now = current_millis()?;
        let id = Uuid::new_v4().to_string();
        let position = manual_order::bottom(&tx, Siblings::folders(input.parent_id.as_deref()))?;
        tx.execute(
            "INSERT INTO note_folders(id, parent_id, name, name_fold, revision, created_at, updated_at, position)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5, ?6)",
            params![id, input.parent_id, name, fold, now, position],
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
        require_destination(&tx, &old, input.parent_id.as_deref())?;
        let position = manual_order::bottom(&tx, Siblings::folders(input.parent_id.as_deref()))?;
        tx.execute(
            "UPDATE note_folders SET position = ?1 WHERE id = ?2",
            params![position, old.id],
        )?;
        let moved = write_folder(&tx, &old, input.parent_id.as_deref(), &old.name)?;
        self.audit_applied(&tx, AuditAction::FolderMove, &moved.id, moved.revision)?;
        tx.commit()?;
        Ok(moved)
    }

    /// Puts a folder, with its subfolders and Notes, exactly where the person
    /// dropped it. A new parent takes every rule `move_note_folder` has.
    pub fn place_note_folder(&self, input: PlaceNoteFolder) -> Result<NoteFolder, TaskError> {
        self.reorder_note_folder(ReorderNoteFolder {
            id: input.id,
            expected_revision: input.expected_revision,
            after_id: input.after_id,
            parent: FolderParent::named(input.parent_id),
        })
    }

    /// Puts a folder after a sibling, or first, under the parent `input`
    /// names. Recorded as `folder.move` when that is another parent than the
    /// folder has as the write finds it, else as `folder.reorder`; a refusal
    /// is recorded the same way, or as a reorder when the folder is gone.
    pub fn reorder_note_folder(&self, input: ReorderNoteFolder) -> Result<NoteFolder, TaskError> {
        #[cfg(test)]
        crate::privacy::between_reads();
        self.audited_as_decided(input, |input| self.place_inner(&input))
    }

    /// Reads the folder once inside the write and decides from it, before any
    /// check can refuse the change, whether it is a move or a reorder.
    fn place_inner(
        &self,
        input: &ReorderNoteFolder,
    ) -> Result<NoteFolder, (AuditAction, TaskError)> {
        let unread = |error| (AuditAction::FolderReorder, error);
        let mut conn = self.connect().map_err(unread)?;
        let tx = self.begin_write(&mut conn).map_err(unread)?;
        let old = get_folder(&tx, &input.id)
            .and_then(|stored| stored.ok_or(TaskError::NoteFolderNotFound))
            .map_err(unread)?;
        let parent_id = input.parent.resolve(old.parent_id.as_deref());
        let action = if parent_id == old.parent_id.as_deref() {
            AuditAction::FolderReorder
        } else {
            AuditAction::FolderMove
        };
        self.write_folder_placement(tx, &old, parent_id, input, action)
            .map_err(|error| (action, error))
    }

    fn write_folder_placement(
        &self,
        tx: Transaction,
        old: &NoteFolder,
        parent_id: Option<&str>,
        input: &ReorderNoteFolder,
        action: AuditAction,
    ) -> Result<NoteFolder, TaskError> {
        require_current(&tx, old, input.expected_revision)?;
        if parent_id != old.parent_id.as_deref() {
            require_destination(&tx, old, parent_id)?;
        }
        let after_id = input.after_id.as_deref();
        if !manual_order::place(&tx, Siblings::folders(parent_id), &old.id, after_id)? {
            return Err(TaskError::NoteFolderUnchanged);
        }
        let placed = write_folder(&tx, old, parent_id, &old.name)?;
        self.audit_applied(&tx, action, &placed.id, placed.revision)?;
        tx.commit()?;
        Ok(placed)
    }

    /// One bounded page of a folder's direct subfolders (or the top level),
    /// in manual order.
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
        let (position, id) = after.unzip();
        let conn = self.connect()?;
        if let Some(parent) = parent_id {
            require_folder(&conn, parent)?;
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT {FOLDER_COLUMNS}, f.position FROM note_folders f
             WHERE coalesce(f.parent_id, '') = ?1 AND f.removed_at IS NULL
               AND (?2 IS NULL OR (f.position, f.id) > (?2, ?3))
             ORDER BY f.position, f.id LIMIT ?4"
        ))?;
        let mut rows = stmt.query(params![parent_key, position, id, i64::from(limit) + 1])?;
        let mut items = Vec::new();
        let mut positions = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(read_folder(row)?);
            positions.push(row.get::<_, i64>(FIRST_EXTRA_FOLDER_COLUMN)?);
        }
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|last| format!("f2|{parent_key}|{}|{}", positions[items.len() - 1], last.id))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::Interface;

    /// Review finding (2026-11-09): whether a placement was a move or a
    /// reorder was decided from a read before the write, so another writer
    /// moving the folder in between left the history naming the wrong one.
    #[test]
    fn a_placement_is_recorded_from_the_folder_as_the_write_finds_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("place-race.db");
        let desktop = TaskStore::open(&db).unwrap();
        let ai = TaskStore::open_as(&db, Interface::Cli).unwrap();
        let folder = |name: &str, parent_id: Option<String>| {
            desktop
                .create_note_folder(CreateNoteFolder {
                    name: name.into(),
                    parent_id,
                })
                .unwrap()
        };
        let work = folder("Work", None);
        let home = folder("Home", None);
        let moving = folder("Moving", Some(work.id.clone()));
        let (id, home_id) = (moving.id.clone(), home.id.clone());
        crate::privacy::set_between_reads(move || {
            desktop
                .move_note_folder(MoveNoteFolder {
                    id,
                    expected_revision: moving.revision,
                    parent_id: Some(home_id),
                })
                .unwrap();
        });

        let stale = ai.place_note_folder(PlaceNoteFolder {
            id: moving.id.clone(),
            expected_revision: moving.revision,
            parent_id: Some(home.id.clone()),
            after_id: None,
        });

        assert!(matches!(stale, Err(TaskError::NoteFolderRevisionConflict)));
        let entries = ai.list_audit_entries(Some(&moving.id), 10, None).unwrap();
        let refused = entries
            .items
            .iter()
            .find(|entry| entry.interface == Interface::Cli)
            .expect("the refusal is recorded");
        assert_eq!(
            refused.action.as_str(),
            "folder.reorder",
            "the folder already sat under Home when the write ran"
        );
    }
}
