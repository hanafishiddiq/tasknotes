use rusqlite::{OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::tasks::{COLUMNS, Task, TaskError, TaskStore, current_millis, read_task};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub id: String,
    pub name: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub removed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateList {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameList {
    pub id: String,
    pub expected_revision: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetListRemoved {
    pub id: String,
    pub expected_revision: i64,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPage {
    pub items: Vec<List>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "listId", rename_all = "snake_case")]
pub enum TaskView {
    #[default]
    All,
    Inbox,
    List(String),
}

impl TaskView {
    pub(crate) fn cursor_key(&self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::Inbox => "inbox".to_owned(),
            Self::List(id) => format!("list:{id}"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTask {
    pub id: String,
    pub expected_revision: i64,
    pub list_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskDetails {
    pub id: String,
    pub expected_revision: i64,
    pub tags: Vec<String>,
    pub due_date: Option<String>,
}

/// A partial Tags/Deadline edit: `None` keeps the current value,
/// `tags: Some(vec![])` clears the Tags and `due_date: Some(None)` clears the
/// Deadline.
#[derive(Debug, Clone)]
pub struct PatchTaskDetails {
    pub id: String,
    pub expected_revision: i64,
    pub tags: Option<Vec<String>>,
    pub due_date: Option<Option<String>>,
}

fn clean_tags(raw: &[String]) -> Result<Vec<String>, TaskError> {
    if raw.len() > 16 {
        return Err(TaskError::InvalidTags);
    }
    let mut seen = HashSet::new();
    let mut clean = Vec::new();
    for value in raw {
        let text = value.trim();
        if text.is_empty() || text.chars().count() > 40 || text.chars().any(char::is_control) {
            return Err(TaskError::InvalidTags);
        }
        if seen.insert(tag_fold(text)) {
            clean.push(text.to_owned());
        }
    }
    clean.sort_by_key(|item| tag_fold(item));
    Ok(clean)
}

/// A Tag's identity (see `text_fold`).
pub(crate) fn tag_fold(name: &str) -> String {
    crate::text_fold::fold(name)
}

/// Schema v8, Tag part: stored folds become full Unicode folds. Tags that now
/// fold alike become one: the first stored spelling stays, and every Task
/// that had either keeps the Tag once. Only the duplicate Tag name goes; no
/// Task loses a Tag.
pub(crate) fn refold_tags(tx: &rusqlite::Transaction) -> Result<(), TaskError> {
    let names: Vec<String> = tx
        .prepare("SELECT name FROM tags ORDER BY rowid")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut kept: HashMap<String, String> = HashMap::new();
    let mut merged = Vec::new();
    for name in names {
        match kept.get(&tag_fold(&name)) {
            Some(canonical) => merged.push((name, canonical.clone())),
            None => {
                kept.insert(tag_fold(&name), name);
            }
        }
    }
    for (duplicate, canonical) in &merged {
        tx.execute(
            "INSERT OR IGNORE INTO task_tags(task_id, tag_name)
             SELECT task_id, ?2 FROM task_tags WHERE tag_name = ?1",
            params![duplicate, canonical],
        )?;
        tx.execute(
            "DELETE FROM task_tags WHERE tag_name = ?1",
            params![duplicate],
        )?;
        tx.execute("DELETE FROM tags WHERE name = ?1", params![duplicate])?;
    }
    // Unique placeholders first, so no new fold collides with an old one.
    tx.execute("UPDATE tags SET fold = char(1) || name", [])?;
    for (fold, name) in &kept {
        tx.execute(
            "UPDATE tags SET fold = ?1 WHERE name = ?2",
            params![fold, name],
        )?;
    }
    Ok(())
}

/// Validate an ISO date-only value, avoiding midnight UTC/time-zone conversions.
fn calendar_date(value: Option<&str>) -> Result<Option<String>, TaskError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(n, b)| n != 4 && n != 7 && !b.is_ascii_digit())
    {
        return Err(TaskError::InvalidDeadline);
    }
    let year = value[0..4]
        .parse::<u32>()
        .map_err(|_| TaskError::InvalidDeadline)?;
    let month = value[5..7]
        .parse::<u32>()
        .map_err(|_| TaskError::InvalidDeadline)?;
    let day = value[8..10]
        .parse::<u32>()
        .map_err(|_| TaskError::InvalidDeadline)?;
    if year == 0 || !(1..=12).contains(&month) {
        return Err(TaskError::InvalidDeadline);
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day == 0 || day > maximum {
        return Err(TaskError::InvalidDeadline);
    }
    Ok(Some(value.to_owned()))
}

/// Bounded, single-query tag hydration: avoids one query per Task row.
pub(crate) fn hydrate_tags(
    conn: &rusqlite::Connection,
    tasks: &mut [Task],
) -> Result<(), TaskError> {
    if tasks.is_empty() {
        return Ok(());
    }
    let positions: HashMap<String, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.clone(), i))
        .collect();
    let placeholders = vec!["?"; tasks.len()].join(",");
    let sql = format!(
        "SELECT tt.task_id,t.name FROM task_tags tt JOIN tags t ON t.name=tt.tag_name
        WHERE tt.task_id IN ({placeholders}) ORDER BY t.fold ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(tasks.iter().map(|t| t.id.as_str())))?;
    while let Some(row) = rows.next()? {
        let task_id: String = row.get(0)?;
        let tag: String = row.get(1)?;
        if let Some(&i) = positions.get(&task_id) {
            tasks[i].tags.push(tag);
        }
    }
    Ok(())
}

/// Verify ownership inside the same transaction as the Task mutation.
pub(crate) fn require_active_list(conn: &rusqlite::Connection, id: &str) -> Result<(), TaskError> {
    if Uuid::parse_str(id).is_err() {
        return Err(TaskError::ListNotFound);
    }
    let removed: Option<Option<i64>> = conn
        .query_row(
            "SELECT removed_at FROM lists WHERE id=?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    match removed {
        None => Err(TaskError::ListNotFound),
        Some(Some(_)) => Err(TaskError::ListRemoved),
        Some(None) => Ok(()),
    }
}

fn list_name(value: &str) -> Result<&str, TaskError> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(TaskError::InvalidListName);
    }
    Ok(name)
}

fn read_list(row: &rusqlite::Row<'_>) -> rusqlite::Result<List> {
    Ok(List {
        id: row.get(0)?,
        name: row.get(1)?,
        revision: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        removed: row.get::<_, Option<i64>>(5)?.is_some(),
    })
}

impl TaskStore {
    /// Edit only the named details; the caller's revision is still checked.
    pub fn patch_task_details(&self, patch: PatchTaskDetails) -> Result<Task, TaskError> {
        if patch.tags.is_none() && patch.due_date.is_none() {
            return Err(TaskError::EmptyPatch);
        }
        let current = self.get_task(&patch.id)?.ok_or(TaskError::NotFound)?;
        self.update_task_details(UpdateTaskDetails {
            id: patch.id,
            expected_revision: patch.expected_revision,
            tags: patch.tags.unwrap_or(current.tags),
            due_date: patch.due_date.unwrap_or(current.due_date),
        })
    }

    pub fn update_task_details(&self, input: UpdateTaskDetails) -> Result<Task, TaskError> {
        let (id, revision) = (input.id.clone(), input.expected_revision);
        self.audited(AuditAction::TaskDetails, &id, revision, || {
            self.update_task_details_inner(input)
        })
    }

    fn update_task_details_inner(&self, input: UpdateTaskDetails) -> Result<Task, TaskError> {
        let tags = clean_tags(&input.tags)?;
        let due_date = calendar_date(input.due_date.as_deref())?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut before = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id=?1"),
                params![input.id],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;
        crate::removed::require_writable_task(&tx, &before.id)?;
        hydrate_tags(&tx, std::slice::from_mut(&mut before))?;
        if before.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if before.due_date == due_date
            && before.tags.iter().map(|s| tag_fold(s)).collect::<Vec<_>>()
                == tags.iter().map(|s| tag_fold(s)).collect::<Vec<_>>()
        {
            return Err(TaskError::TaskUnchanged);
        }
        let now = current_millis()?;
        let mut updated = tx.query_row(
            &format!(
                "UPDATE tasks SET due_date=?1,revision=revision+1,
                updated_at=max(updated_at+1,?2) WHERE id=?3 AND revision=?4
                RETURNING {COLUMNS}"
            ),
            params![due_date, now, input.id, input.expected_revision],
            read_task,
        )?;
        tx.execute("DELETE FROM task_tags WHERE task_id=?1", params![input.id])?;
        let mut canonical = Vec::with_capacity(tags.len());
        for tag in &tags {
            let fold = tag_fold(tag);
            tx.execute(
                "INSERT INTO tags(name,fold) VALUES (?1,?2) ON CONFLICT DO NOTHING",
                params![tag, fold],
            )?;
            // The first stored spelling of a Tag is canonical for every Task.
            let name: String = tx.query_row(
                "SELECT name FROM tags WHERE fold=?1",
                params![fold],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO task_tags(task_id,tag_name) VALUES (?1,?2)",
                params![input.id, name],
            )?;
            canonical.push(name);
        }
        self.audit_applied(&tx, AuditAction::TaskDetails, &updated.id, updated.revision)?;
        tx.commit()?;
        updated.tags = canonical;
        Ok(updated)
    }

    pub fn move_task(&self, input: MoveTask) -> Result<Task, TaskError> {
        let (id, revision) = (input.id.clone(), input.expected_revision);
        self.audited(AuditAction::TaskMove, &id, revision, || {
            self.move_task_inner(input)
        })
    }

    fn move_task_inner(&self, input: MoveTask) -> Result<Task, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id=?1"),
                params![input.id],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;
        crate::removed::require_writable_task(&tx, &old.id)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.list_id == input.list_id {
            return Err(TaskError::TaskUnchanged);
        }
        if let Some(ref id) = input.list_id {
            require_active_list(&tx, id)?;
        }
        let now = current_millis()?;
        let updated = tx.query_row(
            &format!(
                "UPDATE tasks SET list_id=?1,revision=revision+1,
                updated_at=max(updated_at+1,?2) WHERE id=?3 AND revision=?4
                RETURNING {COLUMNS}"
            ),
            params![input.list_id, now, input.id, input.expected_revision],
            read_task,
        )?;
        self.audit_applied(&tx, AuditAction::TaskMove, &updated.id, updated.revision)?;
        tx.commit()?;
        let mut updated = updated;
        hydrate_tags(&conn, std::slice::from_mut(&mut updated))?;
        Ok(updated)
    }

    pub fn create_list(&self, input: CreateList) -> Result<List, TaskError> {
        let name = list_name(&input.name)?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM lists WHERE name = ?1 COLLATE NOCASE AND removed_at IS NULL)",
            params![name], |r| r.get::<_, bool>(0)
        )? {
            return Err(TaskError::ListNameConflict);
        }
        let now = current_millis()?;
        let list = List {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            revision: 1,
            created_at: now,
            updated_at: now,
            removed: false,
        };
        tx.execute(
            "INSERT INTO lists(id, name, revision, created_at, updated_at, removed_at)
             VALUES (?1, ?2, 1, ?3, ?3, NULL)",
            params![list.id, list.name, list.created_at],
        )?;
        self.audit_applied(&tx, AuditAction::ListCreate, &list.id, list.revision)?;
        tx.commit()?;
        Ok(list)
    }

    pub fn rename_list(&self, input: RenameList) -> Result<List, TaskError> {
        let (id, revision) = (input.id.clone(), input.expected_revision);
        self.audited(AuditAction::ListRename, &id, revision, || {
            self.rename_list_inner(input)
        })
    }

    fn rename_list_inner(&self, input: RenameList) -> Result<List, TaskError> {
        let name = list_name(&input.name)?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old = tx
            .query_row(
                "SELECT id,name,revision,created_at,updated_at,removed_at FROM lists WHERE id=?1",
                params![input.id],
                read_list,
            )
            .optional()?
            .ok_or(TaskError::ListNotFound)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.removed {
            return Err(TaskError::ListRemoved);
        }
        if old.name == name {
            return Err(TaskError::ListUnchanged);
        }
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM lists WHERE name=?1 COLLATE NOCASE AND id != ?2 AND removed_at IS NULL)",
            params![name, input.id], |r| r.get::<_, bool>(0),
        )? { return Err(TaskError::ListNameConflict); }
        let now = current_millis()?;
        let updated = tx.query_row(
            "UPDATE lists SET name=?1,revision=revision+1,updated_at=max(updated_at+1,?2)
             WHERE id=?3 AND revision=?4
             RETURNING id,name,revision,created_at,updated_at,removed_at",
            params![name, now, input.id, input.expected_revision],
            read_list,
        )?;
        self.audit_applied(&tx, AuditAction::ListRename, &updated.id, updated.revision)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn set_list_removed(&self, input: SetListRemoved) -> Result<List, TaskError> {
        let action = if input.removed {
            AuditAction::ListRemove
        } else {
            AuditAction::ListRestore
        };
        let (id, revision) = (input.id.clone(), input.expected_revision);
        self.audited(action, &id, revision, || {
            self.set_list_removed_inner(input, action)
        })
    }

    fn set_list_removed_inner(
        &self,
        input: SetListRemoved,
        action: AuditAction,
    ) -> Result<List, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old = tx
            .query_row(
                "SELECT id,name,revision,created_at,updated_at,removed_at FROM lists WHERE id=?1",
                params![input.id],
                read_list,
            )
            .optional()?
            .ok_or(TaskError::ListNotFound)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.removed == input.removed {
            return Err(TaskError::ListUnchanged);
        }
        if !input.removed && tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM lists WHERE name=?1 COLLATE NOCASE AND id!=?2 AND removed_at IS NULL)",
            params![old.name,old.id], |r| r.get::<_, bool>(0),
        )? { return Err(TaskError::ListNameConflict); }
        let now = current_millis()?;
        let removed_at = input.removed.then_some(now);
        let changed = tx.query_row(
            "UPDATE lists SET removed_at=?1,revision=revision+1,updated_at=max(updated_at+1,?2)
             WHERE id=?3 AND revision=?4
             RETURNING id,name,revision,created_at,updated_at,removed_at",
            params![removed_at, now, input.id, input.expected_revision],
            read_list,
        )?;
        self.audit_applied(&tx, action, &changed.id, changed.revision)?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn list_lists(
        &self,
        limit: u32,
        cursor: Option<&str>,
        include_removed: bool,
    ) -> Result<ListPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidListPage);
        }
        let (after_time, after_id) = match cursor {
            None => (None, None),
            Some(raw) => {
                let bits: Vec<_> = raw.split('|').collect();
                if bits.len() != 4
                    || bits[0] != "v2"
                    || bits[3] != if include_removed { "all" } else { "active" }
                    || Uuid::parse_str(bits[2]).is_err()
                {
                    return Err(TaskError::InvalidListCursor);
                }
                let timestamp = bits[1]
                    .parse::<i64>()
                    .map_err(|_| TaskError::InvalidListCursor)?;
                if timestamp < 0 {
                    return Err(TaskError::InvalidListCursor);
                }
                (Some(timestamp), Some(bits[2].to_owned()))
            }
        };
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id,name,revision,created_at,updated_at,removed_at FROM lists
             WHERE (?1 OR removed_at IS NULL)
               AND (?2 IS NULL OR created_at < ?2 OR (created_at=?2 AND id < ?3))
             ORDER BY created_at DESC,id DESC LIMIT ?4",
        )?;
        let mut rows = stmt.query(params![
            include_removed,
            after_time,
            after_id,
            i64::from(limit) + 1
        ])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(read_list(row)?);
        }
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|l| {
                format!(
                    "v2|{}|{}|{}",
                    l.created_at,
                    l.id,
                    if include_removed { "all" } else { "active" },
                )
            })
        } else {
            None
        };
        Ok(ListPage { items, next_cursor })
    }
}
