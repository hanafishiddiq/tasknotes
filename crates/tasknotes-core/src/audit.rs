//! Audit Entries (ADR-0005): an append-only record that an official
//! interface made, or tried and was refused, a consequential change.
//!
//! An entry names the interface, the action, the entity id, the outcome, an
//! error code and a revision. It never holds a title, description, note or
//! any other user text, so reading the audit log cannot leak content.

use rusqlite::{Connection, Transaction, params};
use serde::Serialize;
use uuid::Uuid;

use crate::tasks::{TaskError, TaskStore, current_millis};

/// The official interface a store acts for. The desktop is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Interface {
    #[default]
    Desktop,
    Cli,
    /// Reserved for the MCP adapter (Ticket 12).
    Mcp,
}

impl Interface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Cli => "cli",
            Self::Mcp => "mcp",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "desktop" => Some(Self::Desktop),
            "cli" => Some(Self::Cli),
            "mcp" => Some(Self::Mcp),
            _ => None,
        }
    }
}

macro_rules! actions {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        /// What was changed, named `<entity>.<verb>`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        pub enum AuditAction { $(#[serde(rename = $text)] $variant),+ }

        impl AuditAction {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            fn from_sql(value: &str) -> Option<Self> {
                match value { $($text => Some(Self::$variant),)+ _ => None }
            }

            /// The table holding the entity this action changes.
            fn entity_table(self) -> &'static str {
                match self.as_str().split_once('.') {
                    Some(("list", _)) => "lists",
                    Some(("note", _)) => "notes",
                    Some(("folder", _)) => "note_folders",
                    Some(("link", _)) => "task_note_links",
                    _ => "tasks",
                }
            }
        }
    };
}

actions! {
    TaskCreate => "task.create",
    TaskUpdate => "task.update",
    TaskChange => "task.change",
    TaskProgress => "task.progress",
    TaskDetails => "task.details",
    TaskMove => "task.move",
    TaskRemove => "task.remove",
    TaskRestore => "task.restore",
    ListCreate => "list.create",
    ListRename => "list.rename",
    ListRemove => "list.remove",
    ListRestore => "list.restore",
    NoteCreate => "note.create",
    NoteUpdate => "note.update",
    NoteMove => "note.move",
    NotePin => "note.pin",
    NoteRemove => "note.remove",
    NoteRestore => "note.restore",
    NotePrivacy => "note.privacy",
    FolderCreate => "folder.create",
    FolderRename => "folder.rename",
    FolderMove => "folder.move",
    FolderRemove => "folder.remove",
    FolderRestore => "folder.restore",
    LinkCreate => "link.create",
    LinkRemove => "link.remove",
    LinkRestore => "link.restore",
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Applied,
    Rejected,
}

impl AuditOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "applied" => Some(Self::Applied),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditEntry {
    /// Append order; also the page cursor.
    pub sequence: i64,
    pub id: String,
    pub created_at: i64,
    pub interface: Interface,
    pub action: AuditAction,
    pub entity_id: String,
    pub outcome: AuditOutcome,
    /// The stable error code of a rejected change.
    pub error_code: Option<String>,
    /// The revision a change produced, or the stale one a rejected change named.
    pub revision: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditPage {
    pub items: Vec<AuditEntry>,
    pub next_cursor: Option<String>,
}

/// Schema v10: the audit table. Triggers refuse UPDATE and DELETE, so no
/// writer of this SQLite file can rewrite history.
pub(crate) fn migrate_v10(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE audit_entries (
            sequence INTEGER PRIMARY KEY AUTOINCREMENT,
            id TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL,
            interface TEXT NOT NULL CHECK(interface IN ('desktop', 'cli', 'mcp')),
            action TEXT NOT NULL CHECK(length(action) BETWEEN 1 AND 40),
            entity_id TEXT NOT NULL CHECK(length(entity_id) BETWEEN 1 AND 64),
            outcome TEXT NOT NULL CHECK(outcome IN ('applied', 'rejected')),
            error_code TEXT CHECK(error_code IS NULL OR length(error_code) <= 40),
            revision INTEGER
         );
         CREATE INDEX audit_entries_entity ON audit_entries(entity_id, sequence DESC);
         CREATE TRIGGER audit_entries_append_only_update BEFORE UPDATE ON audit_entries
         BEGIN SELECT RAISE(ABORT, 'audit entries are append-only'); END;
         CREATE TRIGGER audit_entries_append_only_delete BEFORE DELETE ON audit_entries
         BEGIN SELECT RAISE(ABORT, 'audit entries are append-only'); END;
         PRAGMA user_version = 10;",
    )?;
    Ok(())
}

/// One entry to append; the store adds the id, time and interface.
struct NewAuditEntry<'a> {
    action: AuditAction,
    entity_id: &'a str,
    outcome: AuditOutcome,
    error_code: Option<&'static str>,
    revision: Option<i64>,
}

fn insert(conn: &Connection, interface: Interface, entry: &NewAuditEntry) -> Result<(), TaskError> {
    conn.execute(
        "INSERT INTO audit_entries
           (id, created_at, interface, action, entity_id, outcome, error_code, revision)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            Uuid::new_v4().to_string(),
            current_millis()?,
            interface.as_str(),
            entry.action.as_str(),
            entry.entity_id,
            entry.outcome.as_str(),
            entry.error_code,
            entry.revision,
        ],
    )?;
    Ok(())
}

/// A refusal worth keeping in history: a stale revision, or a write to
/// something in Removed. Input mistakes are not consequential changes.
fn meaningful_rejection(error: &TaskError) -> bool {
    matches!(error.code(), "revision_conflict" | "removed")
}

/// Record a refusal only against a row that exists, so an entry never holds
/// an arbitrary string the caller passed as an id.
fn insert_rejection(
    conn: &Connection,
    interface: Interface,
    entry: &NewAuditEntry,
) -> Result<(), TaskError> {
    let table = entry.action.entity_table();
    let exists: bool = conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
        params![entry.entity_id],
        |row| row.get(0),
    )?;
    if exists {
        insert(conn, interface, entry)?;
    }
    Ok(())
}

/// A change request that names the entity it changes and the revision the
/// caller last read; `audited` records a refusal against them.
pub(crate) trait Revisioned {
    fn target(&self) -> (&str, Option<i64>);
}

macro_rules! revisioned {
    ($($input:ty),+ $(,)?) => {
        $(impl Revisioned for $input {
            fn target(&self) -> (&str, Option<i64>) {
                (&self.id, Some(self.expected_revision))
            }
        })+
    };
}

revisioned!(
    crate::tasks::UpdateTask,
    crate::tasks::ChangeTask,
    crate::tasks::RecordProgress,
    crate::organization::UpdateTaskDetails,
    crate::organization::MoveTask,
    crate::organization::RenameList,
    crate::organization::SetListRemoved,
    crate::removed::SetRemoved,
    crate::notes::UpdateNote,
    crate::note_folders::RenameNoteFolder,
    crate::note_folders::MoveNoteFolder,
);

impl Revisioned for crate::notes::MoveNote {
    fn target(&self) -> (&str, Option<i64>) {
        (&self.id, self.expected_revision)
    }
}

impl TaskStore {
    /// Record an applied change inside the transaction that makes it, so the
    /// change and its Audit Entry commit or roll back together.
    pub(crate) fn audit_applied(
        &self,
        tx: &Transaction,
        action: AuditAction,
        entity_id: &str,
        revision: impl Into<Option<i64>>,
    ) -> Result<(), TaskError> {
        let revision = revision.into();
        insert(
            tx,
            self.interface(),
            &NewAuditEntry {
                action,
                entity_id,
                outcome: AuditOutcome::Applied,
                error_code: None,
                revision,
            },
        )
    }

    /// Run a change; when it is refused for a meaningful reason, record the
    /// refusal in its own transaction (the failed one has rolled back). The
    /// insert waits out the busy timeout like any write; if it still fails,
    /// the caller gets the original refusal wrapped in `AuditUnrecorded` so
    /// the adapter can say the history is incomplete.
    pub(crate) fn audited<I: Revisioned, T>(
        &self,
        action: AuditAction,
        input: I,
        change: impl FnOnce(I) -> Result<T, TaskError>,
    ) -> Result<T, TaskError> {
        let (entity_id, revision) = {
            let (id, revision) = input.target();
            (id.to_owned(), revision)
        };
        match change(input) {
            Err(error) if meaningful_rejection(&error) => {
                let entry = NewAuditEntry {
                    action,
                    entity_id: &entity_id,
                    outcome: AuditOutcome::Rejected,
                    error_code: Some(error.code()),
                    revision,
                };
                match self
                    .connect()
                    .and_then(|conn| insert_rejection(&conn, self.interface(), &entry))
                {
                    Ok(()) => Err(error),
                    Err(_) => Err(TaskError::AuditUnrecorded(Box::new(error))),
                }
            }
            result => result,
        }
    }

    /// Audit Entries newest first, optionally for one entity, in keyset pages
    /// of 1–100 entries.
    pub fn list_audit_entries(
        &self,
        entity_id: Option<&str>,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<AuditPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let before = match cursor {
            None => None,
            Some(raw) => Some(
                raw.strip_prefix("a1|")
                    .and_then(|n| n.parse::<i64>().ok())
                    .filter(|n| *n > 0)
                    .ok_or(TaskError::InvalidAuditCursor)?,
            ),
        };
        let conn = self.connect()?;
        // To an AI interface an excluded Note has no history, nor do its links.
        let private = if self.hides_private() {
            "AND entity_id NOT IN (SELECT id FROM notes WHERE exclude_from_ai = 1)
             AND entity_id NOT IN (SELECT l.id FROM task_note_links l
                                   JOIN notes n ON n.id = l.note_id WHERE n.exclude_from_ai = 1)"
        } else {
            ""
        };
        let mut statement = conn.prepare(&format!(
            "SELECT sequence, id, created_at, interface, action, entity_id, outcome,
                    error_code, revision
             FROM audit_entries
             WHERE (?1 IS NULL OR entity_id = ?1) AND (?2 IS NULL OR sequence < ?2) {private}
             ORDER BY sequence DESC LIMIT ?3"
        ))?;
        let mut items = statement
            .query_map(params![entity_id, before, i64::from(limit) + 1], |row| {
                let interface: String = row.get(3)?;
                let action: String = row.get(4)?;
                let outcome: String = row.get(6)?;
                Ok(AuditEntry {
                    sequence: row.get(0)?,
                    id: row.get(1)?,
                    created_at: row.get(2)?,
                    interface: Interface::from_sql(&interface)
                        .ok_or_else(|| crate::tasks::invalid_db_text(3, interface))?,
                    action: AuditAction::from_sql(&action)
                        .ok_or_else(|| crate::tasks::invalid_db_text(4, action))?,
                    entity_id: row.get(5)?,
                    outcome: AuditOutcome::from_sql(&outcome)
                        .ok_or_else(|| crate::tasks::invalid_db_text(6, outcome))?,
                    error_code: row.get(7)?,
                    revision: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|e| format!("a1|{}", e.sequence))
        } else {
            None
        };
        Ok(AuditPage { items, next_cursor })
    }
}
