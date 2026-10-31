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
//! (`note_shown_sql`, `require_note_shown`, `link_shown_sql`,
//! `ai_audit_filter_sql`, `shared_with_ai_sql`); none writes
//! `exclude_from_ai` itself. A new Note read that skips them would leak, so
//! `tests/note_privacy.rs` walks every public Note read with an AI store
//! against an excluded Note, and `tests/export.rs` checks every export.
//!
//! Audit history (coordinator decision 2026-10-26, the owner may revisit):
//! an AI never sees a `note.privacy` entry, nor any entry about a Note, or a
//! link to it, recorded while that Note was excluded, even after the Note is
//! shared again. Entries carry `hidden_from_ai`, fixed when they are written.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;

use crate::audit::{AuditAction, Interface};
use crate::notes::{COLUMNS, Note, read_note};
use crate::tasks::{TaskError, TaskStore};

// Test seam: a step run between two reads of one operation, standing in for
// another process writing at that moment. Per thread, so tests stay apart.
#[cfg(test)]
thread_local! {
    static BETWEEN_READS: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn between_reads() {
    if let Some(step) = BETWEEN_READS.take() {
        step();
    }
}

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

/// SQL that is true when the Note aliased `alias` is shared with AI: what
/// an AI interface may read, and what a human export holds unless the
/// person includes private Notes (`export`).
pub(crate) fn shared_with_ai_sql(alias: &str) -> String {
    format!("NOT ({})", excluded_sql(alias))
}

/// True when `entity` (an SQL expression holding an id) names an excluded
/// Note or a link to one.
fn about_excluded_note_sql(entity: &str) -> String {
    about_excluded_note_in("main", entity)
}

/// `about_excluded_note_sql` against the database `schema` (`main`, or one
/// attached under that name).
fn about_excluded_note_in(schema: &str, entity: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM {schema}.notes xn WHERE xn.id = {entity} AND {excluded})
          OR EXISTS (SELECT 1 FROM {schema}.task_note_links xl
                     JOIN {schema}.notes xn ON xn.id = xl.note_id
                     WHERE xl.id = {entity} AND {excluded}))",
        excluded = excluded_sql("xn"),
    )
}

/// 1 when an existing Audit Entry (its `hidden_from_ai`, action and entity
/// id given as SQL expressions) is hidden from AI right now in the database
/// `schema`, else 0: `ai_audit_filter_sql` as one value. A restore freezes
/// it into the entries it carries over, since the Note that hides them may
/// be missing, or shared, in the restored data.
pub(crate) fn hidden_now_sql(schema: &str, hidden: &str, action: &str, entity: &str) -> String {
    format!(
        "CASE WHEN {hidden} = 1 OR {action} = '{privacy}' OR {about} THEN 1 ELSE 0 END",
        privacy = AuditAction::NotePrivacy.as_str(),
        about = about_excluded_note_in(schema, entity),
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
    /// see a Note marked Exclude from AI, or applies a Bulk Change one asked
    /// for (whatever the person approved, the AI's request cannot reach a
    /// Note it may not see).
    pub(crate) fn hides_private(&self) -> bool {
        self.interface() != Interface::Desktop || self.requested_by().is_some()
    }

    /// SQL that is true when the Note aliased `alias` may be shown to this
    /// interface. Every Note read, listing, link list and count uses it.
    pub(crate) fn note_shown_sql(&self, alias: &str) -> String {
        if self.hides_private() {
            shared_with_ai_sql(alias)
        } else {
            "1".to_owned()
        }
    }

    /// SQL that is true when the link aliased `alias` may be shown: a link
    /// to an excluded Note does not exist to an AI interface.
    pub(crate) fn link_shown_sql(&self, alias: &str) -> String {
        format!(
            "EXISTS (SELECT 1 FROM notes ln WHERE ln.id = {alias}.note_id AND {})",
            self.note_shown_sql("ln")
        )
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
        let tx = self.begin_write(&mut conn)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{CreateNote, NoteQuery, UpdateNote};
    use crate::tasks::CreateTask;

    fn stores() -> (tempfile::TempDir, TaskStore, TaskStore) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("privacy.db");
        let desktop = TaskStore::open(&db).unwrap();
        let ai = TaskStore::open_as(&db, Interface::Cli).unwrap();
        (dir, desktop, ai)
    }

    fn exclude(desktop: &TaskStore, id: &str) {
        desktop
            .set_note_exclude_from_ai(SetNoteExcludeFromAi {
                id: id.to_owned(),
                exclude_from_ai: true,
            })
            .unwrap();
    }

    /// Review finding (2026-10-30): the page and its snippets were separate
    /// reads, so the person could mark the Note private and its autosave land
    /// between them, and the AI's snippet showed the private text.
    #[test]
    fn an_ai_search_cuts_snippets_from_the_state_it_chose_the_page_in() {
        let (_dir, desktop, ai) = stores();
        let note = desktop
            .create_note(CreateNote {
                markdown: "# Lease\n\nlease public words".into(),
                folder_id: None,
            })
            .unwrap();
        BETWEEN_READS.set(Some(Box::new(move || {
            exclude(&desktop, &note.id);
            desktop
                .update_note(UpdateNote {
                    id: note.id.clone(),
                    markdown: "# Lease\n\nlease zzsecretword private".into(),
                    expected_revision: note.revision,
                })
                .unwrap();
        })));
        let page = ai
            .query_notes(
                &NoteQuery {
                    text: "lease".into(),
                    ..NoteQuery::default()
                },
                100,
                None,
            )
            .unwrap();
        assert_eq!(page.items.len(), 1);
        let snippet = page.items[0].snippet.as_deref().unwrap();
        assert!(snippet.contains("public"), "{snippet}");
        assert!(!snippet.contains("zzsecretword"), "{snippet}");
    }

    /// The same finding for the Tasks of a Note: the privacy check and the
    /// page were separate reads, so a link made after the Note was excluded
    /// could reach the AI.
    #[test]
    fn an_ai_link_list_reads_the_state_it_checked_privacy_in() {
        let (_dir, desktop, ai) = stores();
        let task = |title: &str| {
            desktop
                .create_task(CreateTask {
                    title: title.into(),
                    description: String::new(),
                })
                .unwrap()
        };
        let note = desktop
            .create_note(CreateNote {
                markdown: "# Shared".into(),
                folder_id: None,
            })
            .unwrap();
        let shared = task("Synthetic shared errand");
        let private = task("Synthetic private errand");
        desktop.link_note(&shared.id, &note.id).unwrap();
        let note_id = note.id.clone();
        BETWEEN_READS.set(Some(Box::new(move || {
            exclude(&desktop, &note_id);
            desktop.link_note(&private.id, &note_id).unwrap();
        })));
        let page = ai.list_note_tasks(&note.id, 100, None).unwrap();
        let titles: Vec<&str> = page.items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, ["Synthetic shared errand"]);
    }
}
