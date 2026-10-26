//! Exclude from AI (ADR-0004): the one place the core decides what an
//! AI-facing interface (CLI, MCP) may see of a Note.
//!
//! To an AI interface an excluded Note does not exist: every read leaves it
//! out and every write, restore or link naming it fails exactly as for an id
//! that was never used (`NoteNotFound`, or `LinkNotFound` for its links).
//! Audit Entries about it are left out too. The desktop is the person
//! themself and sees and edits everything; only it can change the flag.
//!
//! Every AI-facing Note read goes through one of the helpers below
//! (`note_shown_sql`, `require_note_shown`, `ai_audit_filter_sql`).
//!
//! Audit history (coordinator decision 2026-10-26, the owner may revisit):
//! an AI never sees a `note.privacy` entry, nor any entry about a Note, or a
//! link to it, recorded while that Note was excluded, even after the Note is
//! shared again. Entries carry `hidden_from_ai`, fixed when they are written.

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Deserialize;

use crate::audit::{AuditAction, Interface};
use crate::notes::{COLUMNS, Note, read_note};
use crate::tasks::{TaskError, TaskStore};

/// Schema v11: the per-Note flag. Existing Notes stay visible to AI, as they
/// were before the flag existed.
pub(crate) fn migrate_v11(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE notes ADD COLUMN exclude_from_ai INTEGER NOT NULL DEFAULT 0
           CHECK(exclude_from_ai IN (0, 1));
         PRAGMA user_version = 11;",
    )?;
    Ok(())
}

/// Schema v12: each Audit Entry remembers whether it was written about an
/// excluded Note (or a link to one), so re-sharing the Note does not reveal
/// what happened while it was private. Entries written before v12 cannot
/// know and default to 0; the list still hides every `note.privacy` entry
/// and everything about a Note that is excluded now.
pub(crate) fn migrate_v12(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE audit_entries ADD COLUMN hidden_from_ai INTEGER NOT NULL DEFAULT 0
           CHECK(hidden_from_ai IN (0, 1));
         PRAGMA user_version = 12;",
    )?;
    Ok(())
}

/// The one predicate: true when the Note aliased `alias` is excluded.
fn excluded_sql(alias: &str) -> String {
    format!("{alias}.exclude_from_ai = 1")
}

/// True when `entity` (an SQL expression holding an id) names an excluded
/// Note or a link to one.
fn about_excluded_note_sql(entity: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM notes xn WHERE xn.id = {entity} AND {excluded})
          OR EXISTS (SELECT 1 FROM task_note_links xl JOIN notes xn ON xn.id = xl.note_id
                     WHERE xl.id = {entity} AND {excluded}))",
        excluded = excluded_sql("xn"),
    )
}

/// The value of `hidden_from_ai` for an entry being written, given the SQL
/// expressions holding its action and entity id.
pub(crate) fn hidden_at_record_sql(action: &str, entity: &str) -> String {
    format!(
        "CASE WHEN {action} = '{privacy}' OR {about} THEN 1 ELSE 0 END",
        privacy = AuditAction::NotePrivacy.as_str(),
        about = about_excluded_note_sql(entity),
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetNoteExcludeFromAi {
    pub id: String,
    pub exclude_from_ai: bool,
}

impl TaskStore {
    /// Whether this store serves an AI-facing interface, which must never
    /// see a Note marked Exclude from AI.
    pub(crate) fn hides_private(&self) -> bool {
        self.interface() != Interface::Desktop
    }

    /// SQL that is true when the Note aliased `alias` may be shown to this
    /// interface. Every Note read, listing, link list and count uses it.
    pub(crate) fn note_shown_sql(&self, alias: &str) -> String {
        if self.hides_private() {
            format!("NOT ({})", excluded_sql(alias))
        } else {
            "1".to_owned()
        }
    }

    /// Extra `AND …` conditions on `audit_entries` for this interface; empty
    /// for the desktop. An AI sees no `note.privacy` entry, nothing written
    /// while a Note was excluded, and nothing about a Note excluded now.
    pub(crate) fn ai_audit_filter_sql(&self) -> String {
        if !self.hides_private() {
            return String::new();
        }
        format!(
            "AND hidden_from_ai = 0 AND action <> '{}' AND NOT {}",
            AuditAction::NotePrivacy.as_str(),
            about_excluded_note_sql("entity_id")
        )
    }

    /// Inside the caller's transaction, before any other check on the Note:
    /// to an AI interface an excluded Note is `NoteNotFound`, exactly like
    /// an id that does not exist, so no later refusal (Removed, revision)
    /// can confirm it is there.
    pub(crate) fn require_note_shown(&self, conn: &Connection, id: &str) -> Result<(), TaskError> {
        if !self.hides_private() {
            return Ok(());
        }
        let excluded: Option<bool> = conn
            .query_row(
                "SELECT exclude_from_ai FROM notes WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        match excluded {
            Some(false) => Ok(()),
            _ => Err(TaskError::NoteNotFound),
        }
    }

    /// Marks a Note Exclude from AI, or shares it with AI again. A setting
    /// like a pin: the text, revision and edit time stay, so an editor open
    /// on the Note keeps saving. It is a plain value, not an edit on top of
    /// what the caller last read, so it needs no revision; the flag and
    /// every AI write that checks it run in their own write transactions,
    /// so an AI change lands entirely before or entirely after it.
    ///
    /// Only the desktop may change it: to an AI interface an excluded Note
    /// is missing, and a shared one cannot be hidden or shown by an AI.
    pub fn set_note_exclude_from_ai(&self, input: SetNoteExcludeFromAi) -> Result<Note, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.require_note_shown(&tx, &input.id)?;
        if self.hides_private() {
            return Err(TaskError::DesktopOnly);
        }
        let note = tx
            .query_row(
                &format!("UPDATE notes SET exclude_from_ai = ?1 WHERE id = ?2 RETURNING {COLUMNS}"),
                params![input.exclude_from_ai, input.id],
                read_note,
            )
            .optional()?
            .ok_or(TaskError::NoteNotFound)?;
        self.audit_applied(&tx, AuditAction::NotePrivacy, &note.id, note.revision)?;
        tx.commit()?;
        Ok(note)
    }
}
