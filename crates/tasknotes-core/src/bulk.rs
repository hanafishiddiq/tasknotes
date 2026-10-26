//! High-risk bulk changes (story 55): change several Tasks or Notes at once
//! only after a preview, and only while nothing previewed has changed.
//!
//! `preview_bulk` runs the whole change inside one write transaction and
//! rolls it back, so the preview shows exactly what would happen, refused by
//! the same rules as one change at a time (Removed, transitions, Exclude from
//! AI). It returns a `BulkPlan`: the action, every item with the revision it
//! had, the interface and the time. `apply_bulk` runs the same change in one
//! transaction against those revisions and commits it all or nothing. If any
//! item, or what it depends on (such as the target List), changed in the
//! meantime, or the plan is older than `BULK_PREVIEW_TTL_MS`, nothing is
//! applied and the answer is `stale_preview` without saying which item, so
//! a Note that became excluded is not singled out.
//!
//! Each applied item gets its ordinary Audit Entry. A stale or expired plan
//! is not audited: nothing was attempted against a revision the caller named.
//! Adapters hold plans (for example behind a confirmation token); a plan can
//! only be made by `preview_bulk`.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::audit::{AuditAction, Interface};
use crate::notes::Note;
use crate::removed::SetRemoved;
use crate::tasks::{ChangeTask, MoveTask, Task, TaskError, TaskStore, WorkStatus, current_millis};

/// The most items one bulk change may name.
pub const BULK_MAX_ITEMS: usize = 100;
/// How long a preview may be confirmed: 10 minutes.
pub const BULK_PREVIEW_TTL_MS: i64 = 10 * 60 * 1000;

/// What a bulk change does to every item it names.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

impl BulkAction {
    fn on_notes(&self) -> bool {
        matches!(self, Self::RemoveNotes | Self::RestoreNotes)
    }
}

/// One item before and after the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkItem {
    Task { before: Task, after: Task },
    Note { before: Note, after: Note },
}

#[derive(Debug, Clone)]
pub struct BulkPreview {
    pub items: Vec<BulkItem>,
    pub plan: BulkPlan,
}

/// A previewed change bound to the exact items and revisions shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkPlan {
    action: BulkAction,
    targets: Vec<(String, i64)>,
    interface: Interface,
    made_at: i64,
}

impl BulkPlan {
    pub fn action(&self) -> &BulkAction {
        &self.action
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Unix milliseconds when the preview was made.
    pub fn made_at(&self) -> i64 {
        self.made_at
    }

    /// The last Unix millisecond at which the plan may be applied.
    pub fn expires_at(&self) -> i64 {
        self.made_at + BULK_PREVIEW_TTL_MS
    }
}

impl TaskStore {
    /// Preview a bulk change of 1–`BULK_MAX_ITEMS` distinct ids, changing
    /// nothing. Refused, naming the item's position, if any one item would be.
    pub fn preview_bulk(
        &self,
        action: BulkAction,
        ids: Vec<String>,
    ) -> Result<BulkPreview, TaskError> {
        let distinct: HashSet<&str> = ids.iter().map(String::as_str).collect();
        if ids.is_empty() || ids.len() > BULK_MAX_ITEMS || distinct.len() != ids.len() {
            return Err(TaskError::InvalidBulkSelection);
        }
        let made_at = current_millis()?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut targets = Vec::with_capacity(ids.len());
        for (index, id) in ids.into_iter().enumerate() {
            let revision = self
                .current_revision(&tx, &action, &id)
                .map_err(|error| refused(index, error))?;
            targets.push((id, revision));
        }
        let items = self
            .run_bulk(&tx, &action, &targets)
            .map_err(|(index, error)| refused(index, error))?;
        // Dropping the transaction rolls every change and Audit Entry back.
        drop(tx);
        Ok(BulkPreview {
            items,
            plan: BulkPlan {
                action,
                targets,
                interface: self.interface(),
                made_at,
            },
        })
    }

    /// Apply a previewed change, all or nothing.
    pub fn apply_bulk(&self, plan: &BulkPlan) -> Result<Vec<BulkItem>, TaskError> {
        self.apply_bulk_at(plan, current_millis()?)
    }

    /// `apply_bulk` with the current time given, in Unix milliseconds.
    pub fn apply_bulk_at(&self, plan: &BulkPlan, now: i64) -> Result<Vec<BulkItem>, TaskError> {
        if now > plan.expires_at() || now < plan.made_at {
            return Err(TaskError::PreviewExpired);
        }
        if plan.interface != self.interface() {
            return Err(TaskError::StalePreview);
        }
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let items = self
            .run_bulk(&tx, &plan.action, &plan.targets)
            .map_err(|(_, error)| match error.code() {
                "storage_unavailable" => error,
                _ => TaskError::StalePreview,
            })?;
        tx.commit()?;
        Ok(items)
    }

    /// The item's revision as this interface sees it; an excluded Note is
    /// `NoteNotFound`, like an id never used.
    fn current_revision(
        &self,
        tx: &Transaction,
        action: &BulkAction,
        id: &str,
    ) -> Result<i64, TaskError> {
        if action.on_notes() {
            self.require_note_shown(tx, id)?;
            return tx
                .query_row(
                    "SELECT revision FROM notes WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or(TaskError::NoteNotFound);
        }
        tx.query_row(
            "SELECT revision FROM tasks WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(TaskError::NotFound)
    }

    /// Every item's change in `tx`, each through the same code as a single
    /// change; the first refusal comes back with its position.
    fn run_bulk(
        &self,
        tx: &Transaction,
        action: &BulkAction,
        targets: &[(String, i64)],
    ) -> Result<Vec<BulkItem>, (usize, TaskError)> {
        targets
            .iter()
            .enumerate()
            .map(|(index, (id, revision))| {
                self.run_one(tx, action, id, *revision)
                    .map_err(|error| (index, error))
            })
            .collect()
    }

    fn run_one(
        &self,
        tx: &Transaction,
        action: &BulkAction,
        id: &str,
        expected_revision: i64,
    ) -> Result<BulkItem, TaskError> {
        let set = |removed| SetRemoved {
            id: id.to_owned(),
            expected_revision,
            removed,
        };
        if action.on_notes() {
            self.require_note_shown(tx, id)?;
            let before = read_note(tx, id)?;
            let after = match action {
                BulkAction::RemoveNotes => {
                    self.set_note_removed_in(tx, set(true), AuditAction::NoteRemove)?
                }
                _ => self.set_note_removed_in(tx, set(false), AuditAction::NoteRestore)?,
            };
            return Ok(BulkItem::Note { before, after });
        }
        let before = self.get_task_in(tx, id)?;
        let after = match action {
            BulkAction::RemoveTasks => {
                self.set_task_removed_in(tx, set(true), AuditAction::TaskRemove)?
            }
            BulkAction::RestoreTasks => {
                self.set_task_removed_in(tx, set(false), AuditAction::TaskRestore)?
            }
            BulkAction::MoveTasks { list_id } => self.move_task_in(
                tx,
                MoveTask {
                    id: id.to_owned(),
                    expected_revision,
                    list_id: list_id.clone(),
                },
            )?,
            BulkAction::SetTaskStatus { status } => self.apply_task_change_in(
                tx,
                AuditAction::TaskChange,
                ChangeTask {
                    id: id.to_owned(),
                    expected_revision,
                    status: Some(*status),
                    priority: None,
                    progress: None,
                },
                None,
                String::new(),
            )?,
            BulkAction::RemoveNotes | BulkAction::RestoreNotes => unreachable!("handled above"),
        };
        Ok(BulkItem::Task { before, after })
    }
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
