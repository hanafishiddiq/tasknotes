//! Bulk Changes (story 55; owner decisions A and B, 2026-10-26): a change
//! to several Tasks or Notes at once, or removing a List or Note Folder that
//! holds anything, asked for by an AI interface (CLI, MCP). The AI can only
//! *request* it; the person approves or declines it in the desktop app.
//!
//! - `request_bulk` runs the whole change in one write transaction through
//!   the same per-item code as single changes and rolls it back, so what the
//!   AI is shown is exactly what would happen, refused (naming the item's
//!   position) if any one item would be. It then stores a pending request:
//!   the action, every item with the revision it had and, for a move, the
//!   target List's revision. Requests live in the database, so they wait
//!   while the desktop is closed; they expire after `BULK_REQUEST_TTL_MS`.
//! - `bulk_request` reports a request's status to any interface:
//!   pending, applied, declined, stale or expired.
//! - Only the desktop lists (`pending_bulk_requests`), approves or declines.
//!   Approving replays the stored plan in one transaction, all or nothing,
//!   under the AI's privacy rules, and records every item's Audit Entry
//!   under the desktop with `requested_by` naming the AI. If any item, or the
//!   target List, changed since the request, nothing is applied: the request
//!   becomes stale and one generic Audit Entry about the request (never an
//!   item) records the refusal, so a Note excluded in between is not singled
//!   out.
//! - For a List or folder removal the AI is never told how much it holds:
//!   no count and no items, only that it waits for approval. The desktop
//!   preview lists everything that would be hidden, private Notes included.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::{AuditAction, Interface};
use crate::notes::Note;
use crate::removed::SetRemoved;
use crate::tasks::{
    ChangeTask, MoveTask, SetListRemoved, Task, TaskError, TaskStore, WorkStatus, current_millis,
};

/// The most items one bulk change may name.
pub const BULK_MAX_ITEMS: usize = 100;
/// How long a request waits for the person: one day.
pub const BULK_REQUEST_TTL_MS: i64 = 24 * 60 * 60 * 1000;
/// How many lines one pending request shows in the desktop; `total` counts all.
pub const APPROVAL_LINES: usize = 100;
/// How many pending requests the desktop lists at once, newest first.
const PENDING_SHOWN: i64 = 50;

/// What a Bulk Change does. Its serde form (`{"action": "move_tasks",
/// "list_id": ..}`) is the one name every interface uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BulkAction {
    RemoveTasks,
    RestoreTasks,
    /// To an active List, or to Inbox with `None`.
    MoveTasks {
        list_id: Option<String>,
    },
    /// Under the same transition rules as `change_task`.
    SetTaskStatus {
        status: WorkStatus,
    },
    RemoveNotes,
    RestoreNotes,
    /// One List, with every Task in it.
    RemoveList,
    /// One Note Folder, with everything below it.
    RemoveFolder,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Subject {
    Tasks,
    Notes,
    Container,
}

impl BulkAction {
    fn subject(&self) -> Subject {
        match self {
            Self::RemoveNotes | Self::RestoreNotes => Subject::Notes,
            Self::RemoveList | Self::RemoveFolder => Subject::Container,
            _ => Subject::Tasks,
        }
    }

    /// The action's name, such as `move_tasks`.
    pub fn name(&self) -> String {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(fields)) => fields
                .get("action")
                .and_then(|name| name.as_str())
                .unwrap_or_default()
                .to_owned(),
            _ => String::new(),
        }
    }
}

/// One item before and after the change. A List or folder removal has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkItem {
    Task { before: Task, after: Task },
    Note { before: Note, after: Note },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulkRequestStatus {
    /// Waiting for the person in the desktop app.
    Pending,
    Applied,
    Declined,
    /// Something in it changed before it was approved; nothing was applied.
    Stale,
    /// Nobody decided within `BULK_REQUEST_TTL_MS`; nothing was applied.
    Expired,
}

impl BulkRequestStatus {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Declined => "declined",
            Self::Stale => "stale",
            Self::Expired => "expired",
        }
    }

    fn from_sql(value: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Applied,
            Self::Declined,
            Self::Stale,
            Self::Expired,
        ]
        .into_iter()
        .find(|status| status.as_sql() == value)
    }
}

/// What any interface may know about a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkRequest {
    pub id: String,
    pub action: BulkAction,
    /// How many items it names; `None` for a List or folder removal, so an
    /// AI is never told how much a container holds.
    pub count: Option<usize>,
    pub requested_by: Interface,
    pub created_at: i64,
    pub expires_at: i64,
    pub status: BulkRequestStatus,
    pub decided_at: Option<i64>,
}

/// A new request and what it would do (no items for a container).
#[derive(Debug, Clone)]
pub struct BulkRequested {
    pub request: BulkRequest,
    pub items: Vec<BulkItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    Task,
    Note,
    Folder,
}

/// One thing a pending request would change, as the person sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalLine {
    pub kind: ApprovalKind,
    pub id: String,
    pub title: String,
    pub excluded_from_ai: bool,
}

/// A request waiting in the desktop, with its preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingBulkRequest {
    pub request: BulkRequest,
    /// Approving would be refused: something changed since the request.
    pub stale: bool,
    /// The target List's name for a move, or the List or folder removed.
    pub target: Option<String>,
    /// Up to `APPROVAL_LINES` of what it changes, or would hide.
    pub items: Vec<ApprovalLine>,
    pub total: usize,
}

/// An item and the revision it had when the request was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Target {
    id: String,
    revision: i64,
}

/// What approving replays, stored as JSON with the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    action: BulkAction,
    targets: Vec<Target>,
    /// For a move to a List: that List as it was.
    depends_on: Option<Target>,
}

/// Schema v14: pending Bulk Change requests, and the AI interface that asked
/// for a change the person approved.
pub(crate) fn migrate_v14(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE bulk_requests (
            id TEXT PRIMARY KEY NOT NULL,
            requested_by TEXT NOT NULL CHECK(requested_by IN ('desktop', 'cli', 'mcp')),
            plan TEXT NOT NULL,
            item_count INTEGER NOT NULL CHECK(item_count BETWEEN 1 AND 100),
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending'
              CHECK(status IN ('pending', 'applied', 'declined', 'stale', 'expired')),
            decided_at INTEGER
         );
         CREATE INDEX bulk_requests_pending ON bulk_requests(status, created_at DESC);
         ALTER TABLE audit_entries ADD COLUMN requested_by TEXT
           CHECK(requested_by IS NULL OR requested_by IN ('cli', 'mcp'));
         PRAGMA user_version = 14;",
    )?;
    Ok(())
}

impl TaskStore {
    /// Ask for a Bulk Change of 1–`BULK_MAX_ITEMS` distinct ids (exactly one
    /// for a List or folder removal). Changes nothing but the new pending
    /// request; refused, naming the item's position, if any item would be.
    pub fn request_bulk(
        &self,
        action: BulkAction,
        ids: Vec<String>,
    ) -> Result<BulkRequested, TaskError> {
        let distinct: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let most = match action.subject() {
            Subject::Container => 1,
            _ => BULK_MAX_ITEMS,
        };
        if ids.is_empty() || ids.len() > most || distinct.len() != ids.len() {
            return Err(TaskError::InvalidBulkSelection);
        }
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let (plan, items) = {
            let tx = self.begin_write(&mut conn)?;
            let plan = self.plan_in(&tx, action, ids)?;
            let items = self
                .run_plan(&tx, &plan)
                .map_err(|(index, error)| refused(index, error))?;
            // Dropping the transaction rolls every change and Audit Entry back.
            (plan, items)
        };
        let request = BulkRequest {
            id: Uuid::new_v4().to_string(),
            count: (plan.action.subject() != Subject::Container).then_some(plan.targets.len()),
            action: plan.action.clone(),
            requested_by: self.interface(),
            created_at: now,
            expires_at: now + BULK_REQUEST_TTL_MS,
            status: BulkRequestStatus::Pending,
            decided_at: None,
        };
        let stored =
            serde_json::to_string(&plan).map_err(|e| TaskError::Io(std::io::Error::other(e)))?;
        // A change between the preview above and this insert only makes the
        // request stale: it holds the revisions the preview was made from.
        let tx = self.begin_write(&mut conn)?;
        tx.execute(
            "INSERT INTO bulk_requests
               (id, requested_by, plan, item_count, created_at, expires_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')",
            params![
                request.id,
                request.requested_by.as_str(),
                stored,
                plan.targets.len() as i64,
                request.created_at,
                request.expires_at,
            ],
        )?;
        self.audit_applied(&tx, AuditAction::BulkRequest, &request.id, None::<i64>)?;
        tx.commit()?;
        Ok(BulkRequested { request, items })
    }

    /// A request's status, for any interface.
    pub fn bulk_request(&self, id: &str) -> Result<BulkRequest, TaskError> {
        self.bulk_request_at(id, current_millis()?)
    }

    /// `bulk_request` at the given Unix millisecond.
    pub fn bulk_request_at(&self, id: &str, now: i64) -> Result<BulkRequest, TaskError> {
        Ok(read_request(&self.connect()?, id, now)?.0)
    }

    /// How many requests wait for the person (the desktop's sidebar count).
    pub fn count_pending_bulk_requests(&self) -> Result<i64, TaskError> {
        self.require_desktop()?;
        Ok(self.connect()?.query_row(
            "SELECT count(*) FROM bulk_requests WHERE status = 'pending' AND expires_at >= ?1",
            params![current_millis()?],
            |row| row.get(0),
        )?)
    }

    /// The requests waiting for the person, newest first, each with what it
    /// would change. Desktop only.
    pub fn pending_bulk_requests(&self) -> Result<Vec<PendingBulkRequest>, TaskError> {
        self.pending_bulk_requests_at(current_millis()?)
    }

    /// `pending_bulk_requests` at the given Unix millisecond.
    pub fn pending_bulk_requests_at(&self, now: i64) -> Result<Vec<PendingBulkRequest>, TaskError> {
        self.require_desktop()?;
        let ids: Vec<String> = {
            let conn = self.connect()?;
            let mut statement = conn.prepare(
                "SELECT id FROM bulk_requests WHERE status = 'pending' AND expires_at >= ?1
                 ORDER BY created_at DESC, id DESC LIMIT ?2",
            )?;
            statement
                .query_map(params![now, PENDING_SHOWN], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        ids.iter()
            .map(|id| {
                let (request, plan) = read_request(&self.connect()?, id, now)?;
                self.approval_preview(request, &plan)
            })
            .collect()
    }

    /// Apply a pending request, all or nothing. Desktop only.
    pub fn approve_bulk_request(&self, id: &str) -> Result<BulkRequest, TaskError> {
        self.approve_bulk_request_at(id, current_millis()?)
    }

    /// `approve_bulk_request` at the given Unix millisecond.
    pub fn approve_bulk_request_at(&self, id: &str, now: i64) -> Result<BulkRequest, TaskError> {
        self.require_desktop()?;
        let mut conn = self.connect()?;
        let failure = {
            let tx = self.begin_write(&mut conn)?;
            let (request, plan) = read_request(&tx, id, now)?;
            let acting = self.acting_for(request.requested_by);
            match request.status {
                BulkRequestStatus::Pending => match acting.run_plan(&tx, &plan) {
                    Ok(_) => {
                        let applied = decide(&tx, request, BulkRequestStatus::Applied, now)?;
                        acting.audit_applied(&tx, AuditAction::BulkApprove, id, None::<i64>)?;
                        tx.commit()?;
                        return Ok(applied);
                    }
                    Err((_, error)) if error.code() == "storage_unavailable" => return Err(error),
                    Err(_) => (BulkRequestStatus::Stale, TaskError::StalePreview, acting),
                },
                BulkRequestStatus::Expired => (
                    BulkRequestStatus::Expired,
                    TaskError::PreviewExpired,
                    acting,
                ),
                _ => return Err(TaskError::BulkRequestDecided),
            }
            // Dropping the transaction rolls back every item applied so far.
        };
        let (status, error, acting) = failure;
        let tx = self.begin_write(&mut conn)?;
        let (request, _) = read_request(&tx, id, now)?;
        if matches!(
            request.status,
            BulkRequestStatus::Pending | BulkRequestStatus::Expired
        ) {
            decide(&tx, request, status, now)?;
            acting.audit_rejected(&tx, AuditAction::BulkApprove, id, error.code())?;
            tx.commit()?;
        }
        Err(error)
    }

    /// Discard a pending request. Desktop only.
    pub fn decline_bulk_request(&self, id: &str) -> Result<BulkRequest, TaskError> {
        self.require_desktop()?;
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let (request, _) = read_request(&tx, id, now)?;
        let acting = self.acting_for(request.requested_by);
        let declined = match request.status {
            BulkRequestStatus::Pending => decide(&tx, request, BulkRequestStatus::Declined, now)?,
            BulkRequestStatus::Expired => return Err(TaskError::PreviewExpired),
            _ => return Err(TaskError::BulkRequestDecided),
        };
        acting.audit_applied(&tx, AuditAction::BulkDecline, id, None::<i64>)?;
        tx.commit()?;
        Ok(declined)
    }

    fn require_desktop(&self) -> Result<(), TaskError> {
        if self.interface() == Interface::Desktop {
            Ok(())
        } else {
            Err(TaskError::DesktopOnly)
        }
    }

    /// The items and revisions a request names, read as this interface sees
    /// them; refusals name the item's position.
    fn plan_in(
        &self,
        tx: &Transaction,
        action: BulkAction,
        ids: Vec<String>,
    ) -> Result<Plan, TaskError> {
        let depends_on = match &action {
            BulkAction::MoveTasks { list_id: Some(id) } => {
                crate::organization::require_active_list(tx, id)?;
                let list = crate::organization::get_list(tx, id)?;
                Some(Target {
                    id: list.id,
                    revision: list.revision,
                })
            }
            _ => None,
        };
        let mut targets = Vec::with_capacity(ids.len());
        for (index, id) in ids.into_iter().enumerate() {
            let revision = self
                .current_revision(tx, &action, &id)
                .map_err(|error| refused(index, error))?;
            targets.push(Target { id, revision });
        }
        Ok(Plan {
            action,
            targets,
            depends_on,
        })
    }

    /// The item's revision as this interface sees it; an excluded Note is
    /// `NoteNotFound`, like an id never used.
    fn current_revision(
        &self,
        tx: &Transaction,
        action: &BulkAction,
        id: &str,
    ) -> Result<i64, TaskError> {
        let (table, missing) = match action {
            BulkAction::RemoveNotes | BulkAction::RestoreNotes => {
                self.require_note_shown(tx, id)?;
                ("notes", TaskError::NoteNotFound)
            }
            BulkAction::RemoveList => ("lists", TaskError::ListNotFound),
            BulkAction::RemoveFolder => ("note_folders", TaskError::NoteFolderNotFound),
            _ => ("tasks", TaskError::NotFound),
        };
        tx.query_row(
            &format!("SELECT revision FROM {table} WHERE id = ?1"),
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(missing)
    }

    /// Every item's change in `tx`, each through the same code as a single
    /// change; the first refusal comes back with its position. A target List
    /// changed since the plan was made refuses the whole plan.
    fn run_plan(&self, tx: &Transaction, plan: &Plan) -> Result<Vec<BulkItem>, (usize, TaskError)> {
        if let Some(list) = &plan.depends_on {
            let current = crate::organization::get_list(tx, &list.id).map_err(|e| (0, e))?;
            if current.revision != list.revision || current.removed {
                return Err((0, TaskError::StalePreview));
            }
        }
        let mut items = Vec::with_capacity(plan.targets.len());
        for (index, target) in plan.targets.iter().enumerate() {
            if let Some(item) = self
                .run_one(tx, &plan.action, target)
                .map_err(|error| (index, error))?
            {
                items.push(item);
            }
        }
        Ok(items)
    }

    fn run_one(
        &self,
        tx: &Transaction,
        action: &BulkAction,
        target: &Target,
    ) -> Result<Option<BulkItem>, TaskError> {
        let id = target.id.clone();
        let expected_revision = target.revision;
        let set = |removed| SetRemoved {
            id: id.clone(),
            expected_revision,
            removed,
        };
        let task_change = |after: Task, before: Task| Some(BulkItem::Task { before, after });
        Ok(match action {
            BulkAction::RemoveNotes | BulkAction::RestoreNotes => {
                self.require_note_shown(tx, &id)?;
                let before = read_note(tx, &id)?;
                let after = if *action == BulkAction::RemoveNotes {
                    self.set_note_removed_in(tx, set(true), AuditAction::NoteRemove)?
                } else {
                    self.set_note_removed_in(tx, set(false), AuditAction::NoteRestore)?
                };
                Some(BulkItem::Note { before, after })
            }
            BulkAction::RemoveList => {
                self.set_list_removed_in(
                    tx,
                    SetListRemoved {
                        id: id.clone(),
                        expected_revision,
                        removed: true,
                    },
                    AuditAction::ListRemove,
                )?;
                None
            }
            BulkAction::RemoveFolder => {
                self.set_note_folder_removed_in(tx, set(true), AuditAction::FolderRemove)?;
                None
            }
            BulkAction::RemoveTasks => {
                let before = self.get_task_in(tx, &id)?;
                task_change(
                    self.set_task_removed_in(tx, set(true), AuditAction::TaskRemove)?,
                    before,
                )
            }
            BulkAction::RestoreTasks => {
                let before = self.get_task_in(tx, &id)?;
                task_change(
                    self.set_task_removed_in(tx, set(false), AuditAction::TaskRestore)?,
                    before,
                )
            }
            BulkAction::MoveTasks { list_id } => {
                let before = self.get_task_in(tx, &id)?;
                task_change(
                    self.move_task_in(
                        tx,
                        MoveTask {
                            id: id.clone(),
                            expected_revision,
                            list_id: list_id.clone(),
                        },
                    )?,
                    before,
                )
            }
            BulkAction::SetTaskStatus { status } => {
                let before = self.get_task_in(tx, &id)?;
                let change = ChangeTask {
                    id: id.clone(),
                    expected_revision,
                    status: Some(*status),
                    priority: None,
                    progress: None,
                };
                // The same checks `change_task` makes before its transaction.
                let note = crate::tasks::checked_change(&change, None, None)?;
                task_change(
                    self.apply_task_change_in(tx, AuditAction::TaskChange, change, None, note)?,
                    before,
                )
            }
        })
    }

    /// The desktop's view of a pending request: what it changes or hides,
    /// and whether approving it now would be refused.
    fn approval_preview(
        &self,
        request: BulkRequest,
        plan: &Plan,
    ) -> Result<PendingBulkRequest, TaskError> {
        let conn = self.connect()?;
        let (target, lines) = approval_lines(&conn, plan)?;
        let total = lines.len();
        let stale = {
            let mut writer = self.connect()?;
            let tx = self.begin_write(&mut writer)?;
            // Dropped without committing: only whether it would apply.
            match self.acting_for(request.requested_by).run_plan(&tx, plan) {
                Ok(_) => false,
                Err((_, error)) if error.code() == "storage_unavailable" => return Err(error),
                Err(_) => true,
            }
        };
        Ok(PendingBulkRequest {
            request,
            stale,
            target,
            items: lines.into_iter().take(APPROVAL_LINES).collect(),
            total,
        })
    }
}

/// Record a decision on a request inside `tx`.
fn decide(
    tx: &Transaction,
    mut request: BulkRequest,
    status: BulkRequestStatus,
    now: i64,
) -> Result<BulkRequest, TaskError> {
    tx.execute(
        "UPDATE bulk_requests SET status = ?1, decided_at = ?2 WHERE id = ?3",
        params![status.as_sql(), now, request.id],
    )?;
    request.status = status;
    request.decided_at = Some(now);
    Ok(request)
}

/// The request and its plan; a pending request past its expiry reads as
/// expired.
fn read_request(conn: &Connection, id: &str, now: i64) -> Result<(BulkRequest, Plan), TaskError> {
    let row = conn
        .query_row(
            "SELECT id, requested_by, plan, item_count, created_at, expires_at, status, decided_at
             FROM bulk_requests WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or(TaskError::BulkRequestNotFound)?;
    let (id, requested_by, plan, count, created_at, expires_at, status, decided_at) = row;
    let plan: Plan = serde_json::from_str(&plan)
        .map_err(|_| TaskError::Sqlite(crate::tasks::invalid_db_text(2, plan)))?;
    let requested_by = Interface::from_sql(&requested_by)
        .ok_or_else(|| TaskError::Sqlite(crate::tasks::invalid_db_text(1, requested_by)))?;
    let mut status = BulkRequestStatus::from_sql(&status)
        .ok_or_else(|| TaskError::Sqlite(crate::tasks::invalid_db_text(6, status)))?;
    if status == BulkRequestStatus::Pending && now > expires_at {
        status = BulkRequestStatus::Expired;
    }
    let request = BulkRequest {
        id,
        count: (plan.action.subject() != Subject::Container).then_some(count as usize),
        action: plan.action.clone(),
        requested_by,
        created_at,
        expires_at,
        status,
        decided_at,
    };
    Ok((request, plan))
}

/// What the person is shown: the target or container name, and every item
/// the request changes or hides, private Notes included.
fn approval_lines(
    conn: &Connection,
    plan: &Plan,
) -> Result<(Option<String>, Vec<ApprovalLine>), TaskError> {
    let line = |kind, row: &rusqlite::Row<'_>| -> rusqlite::Result<ApprovalLine> {
        Ok(ApprovalLine {
            kind,
            id: row.get(0)?,
            title: row.get(1)?,
            excluded_from_ai: row.get(2)?,
        })
    };
    let one = |sql: &str, kind, id: &str| -> Result<Option<ApprovalLine>, TaskError> {
        Ok(conn
            .query_row(sql, params![id], |row| line(kind, row))
            .optional()?)
    };
    let id = plan
        .targets
        .first()
        .map(|t| t.id.as_str())
        .unwrap_or_default();
    let name = |sql: &str| -> Result<Option<String>, TaskError> {
        Ok(conn
            .query_row(sql, params![id], |row| row.get(0))
            .optional()?)
    };
    let many = |sql: &str, kind| -> Result<Vec<ApprovalLine>, TaskError> {
        let mut statement = conn.prepare(sql)?;
        Ok(statement
            .query_map(params![id], |row| line(kind, row))?
            .collect::<rusqlite::Result<_>>()?)
    };
    Ok(match plan.action.subject() {
        Subject::Tasks => {
            let target = match &plan.depends_on {
                Some(list) => conn
                    .query_row(
                        "SELECT name FROM lists WHERE id = ?1",
                        params![list.id],
                        |row| row.get(0),
                    )
                    .optional()?,
                None => None,
            };
            let mut lines = Vec::new();
            for t in &plan.targets {
                lines.extend(one(
                    "SELECT id, title, 0 FROM tasks WHERE id = ?1",
                    ApprovalKind::Task,
                    &t.id,
                )?);
            }
            (target, lines)
        }
        Subject::Notes => {
            let mut lines = Vec::new();
            for t in &plan.targets {
                lines.extend(one(
                    "SELECT id, title, exclude_from_ai FROM notes WHERE id = ?1",
                    ApprovalKind::Note,
                    &t.id,
                )?);
            }
            (None, lines)
        }
        Subject::Container if plan.action == BulkAction::RemoveList => (
            name("SELECT name FROM lists WHERE id = ?1")?,
            many(
                "SELECT id, title, 0 FROM tasks WHERE list_id = ?1
                 ORDER BY created_at DESC, id DESC",
                ApprovalKind::Task,
            )?,
        ),
        Subject::Container => {
            let below = "WITH RECURSIVE below(id) AS (
                    SELECT ?1 UNION
                    SELECT f.id FROM note_folders f JOIN below b ON f.parent_id = b.id
                 )";
            let mut lines = many(
                &format!(
                    "{below} SELECT id, name, 0 FROM note_folders
                     WHERE id IN (SELECT id FROM below) AND id <> ?1 ORDER BY name"
                ),
                ApprovalKind::Folder,
            )?;
            lines.extend(many(
                &format!(
                    "{below} SELECT id, title, exclude_from_ai FROM notes
                     WHERE folder_id IN (SELECT id FROM below) ORDER BY updated_at DESC, id"
                ),
                ApprovalKind::Note,
            )?);
            (name("SELECT name FROM note_folders WHERE id = ?1")?, lines)
        }
    })
}

fn refused(index: usize, error: TaskError) -> TaskError {
    TaskError::BulkItemRefused {
        position: index + 1,
        error: Box::new(error),
    }
}

fn read_note(tx: &Transaction, id: &str) -> Result<Note, TaskError> {
    tx.query_row(
        &format!("SELECT {} FROM notes WHERE id = ?1", crate::notes::COLUMNS),
        params![id],
        crate::notes::read_note,
    )
    .optional()?
    .ok_or(TaskError::NoteNotFound)
}
