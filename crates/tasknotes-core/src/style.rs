//! A Style: the optional icon and colour of a List or Note Folder (Ticket
//! 01). Without one, a List or Note Folder keeps the default look.
//!
//! `style.json` is the one allow-list of icon names (Lucide names) and colour
//! keys. The core reads it here to validate every write; the desktop imports
//! the same file to draw its picker, so neither list is written twice.

use std::sync::OnceLock;

use rusqlite::{Row, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::audit::AuditAction;
use crate::note_folders::{NoteFolder, current_folder, get_folder};
use crate::organization::{List, get_list};
use crate::tasks::{TaskError, TaskStore, current_millis, present};

/// A List's or Note Folder's icon and colour (GLOSSARY: Style). `None` in
/// either keeps the default look for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    /// A Lucide icon name from the allow-list, or the default folder icon.
    pub icon: Option<String>,
    /// One of the twelve colour keys, or the default accent.
    pub color: Option<String>,
}

impl Style {
    /// The `icon` and `color` columns, selected next to each other from `first`.
    pub(crate) fn read(row: &Row<'_>, first: usize) -> rusqlite::Result<Self> {
        Ok(Self {
            icon: row.get(first)?,
            color: row.get(first + 1)?,
        })
    }
}

/// Every icon name and colour key a Style may hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleOptions {
    /// Twelve named colours; each theme gives every key its own value.
    pub colors: Vec<String>,
    /// A curated set of Lucide icon names, in the order the picker shows them.
    pub icons: Vec<String>,
}

pub fn options() -> &'static StyleOptions {
    static OPTIONS: OnceLock<StyleOptions> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        serde_json::from_str(include_str!("style.json")).expect("style.json is a valid allow-list")
    })
}

pub fn is_icon(name: &str) -> bool {
    options().icons.iter().any(|icon| icon == name)
}

pub fn is_color(key: &str) -> bool {
    options().colors.iter().any(|color| color == key)
}

/// Sets or clears a List's or Note Folder's icon and colour. A field left
/// out (`None`) keeps its value; `Some(None)` clears it to the default look.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStyle {
    pub id: String,
    pub expected_revision: i64,
    #[serde(default, deserialize_with = "present")]
    pub icon: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub color: Option<Option<String>>,
}

impl SetStyle {
    /// Refuses an empty change or a name outside the allow-lists, before
    /// anything is read.
    fn check(&self) -> Result<(), TaskError> {
        if self.icon.is_none() && self.color.is_none() {
            return Err(TaskError::EmptyPatch);
        }
        if let Some(Some(icon)) = &self.icon
            && !is_icon(icon)
        {
            return Err(TaskError::InvalidStyleIcon);
        }
        if let Some(Some(color)) = &self.color
            && !is_color(color)
        {
            return Err(TaskError::InvalidStyleColor);
        }
        Ok(())
    }

    /// The Style after this change is applied to `current`.
    fn applied_to(&self, current: &Style) -> Style {
        Style {
            icon: self.icon.clone().unwrap_or_else(|| current.icon.clone()),
            color: self.color.clone().unwrap_or_else(|| current.color.clone()),
        }
    }
}

/// Writes `input` over `current` in `table` ("lists" or "note_folders"),
/// once the caller has checked the row's revision and that it is not
/// Removed. Refuses with `unchanged` when the Style would stay the same.
fn restyle(
    tx: &Transaction,
    table: &'static str,
    input: &SetStyle,
    current: &Style,
    unchanged: TaskError,
) -> Result<(), TaskError> {
    let style = input.applied_to(current);
    if &style == current {
        return Err(unchanged);
    }
    let now = current_millis()?;
    tx.execute(
        &format!(
            "UPDATE {table} SET icon = ?1, color = ?2, revision = revision + 1,
               updated_at = max(updated_at + 1, ?3)
             WHERE id = ?4 AND revision = ?5"
        ),
        params![
            style.icon,
            style.color,
            now,
            input.id,
            input.expected_revision
        ],
    )?;
    Ok(())
}

/// Schema v19: Lists and Note Folders may carry an icon and a colour. Every
/// existing one keeps the default look (NULL). The columns only bound the
/// length: the allow-lists are the core's (`style.json`), so a later icon
/// needs no migration.
pub(crate) fn migrate_v19(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE lists ADD COLUMN icon TEXT
           CHECK (icon IS NULL OR length(icon) BETWEEN 1 AND 64);
         ALTER TABLE lists ADD COLUMN color TEXT
           CHECK (color IS NULL OR length(color) BETWEEN 1 AND 32);
         ALTER TABLE note_folders ADD COLUMN icon TEXT
           CHECK (icon IS NULL OR length(icon) BETWEEN 1 AND 64);
         ALTER TABLE note_folders ADD COLUMN color TEXT
           CHECK (color IS NULL OR length(color) BETWEEN 1 AND 32);
         PRAGMA user_version = 19;",
    )?;
    Ok(())
}

impl TaskStore {
    pub fn set_list_style(&self, input: SetStyle) -> Result<List, TaskError> {
        self.audited(AuditAction::ListStyle, input, |input| {
            self.set_list_style_inner(input)
        })
    }

    fn set_list_style_inner(&self, input: SetStyle) -> Result<List, TaskError> {
        input.check()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let old = get_list(&tx, &input.id)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.removed {
            return Err(TaskError::ListRemoved);
        }
        restyle(&tx, "lists", &input, &old.style, TaskError::ListUnchanged)?;
        let styled = get_list(&tx, &old.id)?;
        self.audit_applied(&tx, AuditAction::ListStyle, &styled.id, styled.revision)?;
        tx.commit()?;
        Ok(styled)
    }

    pub fn set_note_folder_style(&self, input: SetStyle) -> Result<NoteFolder, TaskError> {
        self.audited(AuditAction::FolderStyle, input, |input| {
            self.set_note_folder_style_inner(input)
        })
    }

    fn set_note_folder_style_inner(&self, input: SetStyle) -> Result<NoteFolder, TaskError> {
        input.check()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        // Refuses a stale revision and a Removed folder.
        let old = current_folder(&tx, &input.id, input.expected_revision)?;
        restyle(
            &tx,
            "note_folders",
            &input,
            &old.style,
            TaskError::NoteFolderUnchanged,
        )?;
        let styled = get_folder(&tx, &old.id)?.ok_or(TaskError::NoteFolderNotFound)?;
        self.audit_applied(&tx, AuditAction::FolderStyle, &styled.id, styled.revision)?;
        tx.commit()?;
        Ok(styled)
    }
}
