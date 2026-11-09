//! A Style: the optional icon and colour of a List or Note Folder (Ticket
//! 01). Without one, a List or Note Folder keeps the default look.
//!
//! `style.json` is the one allow-list of icon names (Lucide names) and colour
//! keys. The core reads it here to validate every write; the desktop imports
//! the same file to draw its picker, so neither list is written twice.

use std::sync::OnceLock;

use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};

use crate::audit::AuditAction;
use crate::organization::{LIST_COLUMNS, List, get_list, read_list};
use crate::tasks::{TaskError, TaskStore, current_millis, present};

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

    /// The icon and colour after this change over the current ones.
    fn over(&self, icon: &Option<String>, color: &Option<String>) -> Look {
        Look {
            icon: self.icon.clone().unwrap_or_else(|| icon.clone()),
            color: self.color.clone().unwrap_or_else(|| color.clone()),
        }
    }
}

struct Look {
    icon: Option<String>,
    color: Option<String>,
}

impl Look {
    fn is(&self, icon: &Option<String>, color: &Option<String>) -> bool {
        &self.icon == icon && &self.color == color
    }
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
        let look = input.over(&old.icon, &old.color);
        if look.is(&old.icon, &old.color) {
            return Err(TaskError::ListUnchanged);
        }
        let now = current_millis()?;
        let updated = tx.query_row(
            &format!(
                "UPDATE lists SET icon=?1,color=?2,revision=revision+1,updated_at=max(updated_at+1,?3)
                 WHERE id=?4 AND revision=?5 RETURNING {LIST_COLUMNS}"
            ),
            params![look.icon, look.color, now, input.id, input.expected_revision],
            read_list,
        )?;
        self.audit_applied(&tx, AuditAction::ListStyle, &updated.id, updated.revision)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn set_note_folder_style(
        &self,
        input: SetStyle,
    ) -> Result<crate::note_folders::NoteFolder, TaskError> {
        self.audited(AuditAction::FolderStyle, input, |input| {
            self.set_note_folder_style_inner(input)
        })
    }

    fn set_note_folder_style_inner(
        &self,
        input: SetStyle,
    ) -> Result<crate::note_folders::NoteFolder, TaskError> {
        input.check()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let old = crate::note_folders::current_folder(&tx, &input.id, input.expected_revision)?;
        let look = input.over(&old.icon, &old.color);
        if look.is(&old.icon, &old.color) {
            return Err(TaskError::NoteFolderUnchanged);
        }
        let now = current_millis()?;
        tx.execute(
            "UPDATE note_folders SET icon = ?1, color = ?2, revision = revision + 1,
               updated_at = max(updated_at + 1, ?3)
             WHERE id = ?4 AND revision = ?5",
            params![look.icon, look.color, now, old.id, old.revision],
        )?;
        let styled =
            crate::note_folders::get_folder(&tx, &old.id)?.ok_or(TaskError::NoteFolderNotFound)?;
        self.audit_applied(&tx, AuditAction::FolderStyle, &styled.id, styled.revision)?;
        tx.commit()?;
        Ok(styled)
    }
}
