use rusqlite::{OptionalExtension, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::manual_order::{self, Siblings};
use crate::style::Style;
use crate::tasks::{COLUMNS, Task, TaskError, TaskStore, current_millis, read_task};
use crate::tree::{Tree, WALK_LIMIT};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub id: String,
    /// The List it sits inside, or None at the top level (ADR 0009).
    #[serde(default)]
    pub parent_id: Option<String>,
    pub name: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub removed: bool,
    /// Whether a List that is not in Removed sits inside it, so the sidebar
    /// can expand it.
    #[serde(default)]
    pub has_children: bool,
    /// Its icon and colour; serialised as top-level `icon` and `color`.
    #[serde(flatten)]
    pub style: Style,
}

/// A new List at the top level.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateList {
    pub name: String,
}

/// A new Sub-List: an empty List inside another (ADR 0009).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubList {
    pub name: String,
    pub parent_id: String,
}

/// Moves a List, with the Lists inside it, under another List or to the top
/// level, at the bottom there. Its Tasks stay its own.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveList {
    pub id: String,
    pub expected_revision: i64,
    /// None is the top level.
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// Puts a List under a parent, directly after a sibling there or first
/// without one, in one step (ADR-0007, ADR 0009): what a sidebar drop does.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceList {
    pub id: String,
    pub expected_revision: i64,
    /// None is the top level.
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub after_id: Option<String>,
}

/// Which parent a placed List ends up under, as `FolderParent` for Note
/// Folders. `Current` is what a reorder names: the desktop, the CLI and MCP
/// give no parent to keep a List among its siblings, and the core resolves
/// that inside the write, so a move made elsewhere in between is kept rather
/// than undone, and the history names a reorder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListParent {
    /// The parent it has when the write happens.
    #[default]
    Current,
    /// The top level.
    Top,
    /// Inside this List.
    List(String),
}

impl ListParent {
    /// A parent named as an interface does: a List id, or None for the top.
    pub fn named(parent_id: Option<String>) -> Self {
        parent_id.map_or(Self::Top, Self::List)
    }

    /// The parent id this resolves to for a List now under `current`.
    fn resolve<'a>(&'a self, current: Option<&'a str>) -> Option<&'a str> {
        match self {
            Self::Current => current,
            Self::Top => None,
            Self::List(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameList {
    pub id: String,
    pub expected_revision: i64,
    pub name: String,
}

/// Puts a List directly after a sibling, or first without one, under the
/// parent an interface names, or under the one it has when the write
/// happens if it names none (ADR-0007, ADR 0009). The one entry point for
/// the desktop, the CLI and MCP.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderList {
    pub id: String,
    pub expected_revision: i64,
    #[serde(default)]
    pub after_id: Option<String>,
    #[serde(default)]
    pub parent: ListParent,
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

/// Schema v21 (ADR 0009): a List may sit inside another. Every existing
/// List stays at the top level, in the order it had, and keeps its Tasks.
/// Names become unique among siblings only (owner decision Q33); names that
/// were unique among all Lists stay valid, so no row changes. Forward-only
/// and self-contained: it touches only `lists`.
pub(crate) fn migrate_v21(migration: &rusqlite::Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "ALTER TABLE lists ADD COLUMN parent_id TEXT
           REFERENCES lists(id) ON DELETE RESTRICT
           CHECK (parent_id IS NULL OR parent_id <> id);
         DROP INDEX IF EXISTS lists_active_name;
         CREATE UNIQUE INDEX lists_sibling_name
           ON lists(coalesce(parent_id, ''), name COLLATE NOCASE) WHERE removed_at IS NULL;
         DROP INDEX IF EXISTS lists_manual_order;
         CREATE INDEX lists_manual_order ON lists(coalesce(parent_id, ''), position, id);
         CREATE INDEX lists_parent ON lists(parent_id) WHERE parent_id IS NOT NULL;
         PRAGMA user_version = 21;",
    )?;
    Ok(())
}

/// Whether anything sits in the List: a Task or a List, in Removed or not.
/// Through an AI interface such a List needs approval to remove.
pub(crate) fn list_holds_anything(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<bool, TaskError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE list_id = ?1)
             OR EXISTS(SELECT 1 FROM lists WHERE parent_id = ?1)",
        params![id],
        |row| row.get(0),
    )?)
}

/// Whether the List is hidden: in Removed itself or inside a Removed List.
pub(crate) fn list_hidden(conn: &rusqlite::Connection, id: &str) -> Result<bool, TaskError> {
    Ok(conn.query_row(
        concat!("SELECT ?1 IN (", hidden_lists!(), ")"),
        params![id],
        |row| row.get(0),
    )?)
}

/// The List as stored, or `ListNotFound`.
pub(crate) fn get_list(conn: &rusqlite::Connection, id: &str) -> Result<List, TaskError> {
    conn.query_row(
        &format!("SELECT {LIST_COLUMNS} FROM lists l WHERE l.id=?1"),
        params![id],
        read_list,
    )
    .optional()?
    .ok_or(TaskError::ListNotFound)
}

/// Verify ownership inside the same transaction as the Task mutation: the
/// List exists and neither it nor a List around it is in Removed.
pub(crate) fn require_active_list(conn: &rusqlite::Connection, id: &str) -> Result<(), TaskError> {
    if Uuid::parse_str(id).is_err() {
        return Err(TaskError::ListNotFound);
    }
    get_list(conn, id)?;
    if list_hidden(conn, id)? {
        return Err(TaskError::ListRemoved);
    }
    Ok(())
}

/// The List as stored, provided nobody changed it since `expected_revision`
/// and it is shown: not in Removed and not inside a Removed List.
fn current_list(
    conn: &rusqlite::Connection,
    id: &str,
    expected_revision: i64,
) -> Result<List, TaskError> {
    let list = get_list(conn, id)?;
    require_current(conn, &list, expected_revision)?;
    Ok(list)
}

/// Refuses a write to `list`, as just read, when somebody changed it since
/// `expected_revision` or it is hidden by Removed.
fn require_current(
    conn: &rusqlite::Connection,
    list: &List,
    expected_revision: i64,
) -> Result<(), TaskError> {
    if list.revision != expected_revision {
        return Err(TaskError::RevisionConflict);
    }
    if list_hidden(conn, &list.id)? {
        return Err(TaskError::ListRemoved);
    }
    Ok(())
}

/// Whether `list` may live under `parent_id`: the parent is shown, the
/// shared tree rule holds (no cycle, five levels at most), and no sibling
/// there not in Removed has its name.
fn require_list_destination(
    conn: &rusqlite::Connection,
    list: &List,
    parent_id: Option<&str>,
) -> Result<(), TaskError> {
    if let Some(parent) = parent_id {
        require_active_list(conn, parent)?;
        Tree::Lists.require_fits_under(conn, &list.id, parent)?;
    }
    require_unique_list_name(conn, parent_id, &list.name, Some(&list.id))
}

/// Bumps the List's revision after a write to it, and reads it back.
fn bump_list(conn: &rusqlite::Connection, old: &List) -> Result<List, TaskError> {
    let now = current_millis()?;
    conn.execute(
        "UPDATE lists SET revision=revision+1,updated_at=max(updated_at+1,?1)
         WHERE id=?2 AND revision=?3",
        params![now, old.id, old.revision],
    )?;
    get_list(conn, &old.id)
}

fn list_name(value: &str) -> Result<&str, TaskError> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(TaskError::InvalidListName);
    }
    Ok(name)
}

/// What every read of a List selects, in `read_list`'s order, from `lists l`.
const LIST_COLUMNS: &str = "l.id,l.name,l.revision,l.created_at,l.updated_at,l.removed_at,
    l.icon,l.color,l.parent_id,
    EXISTS(SELECT 1 FROM lists c WHERE c.parent_id = l.id AND c.removed_at IS NULL)";

fn read_list(row: &rusqlite::Row<'_>) -> rusqlite::Result<List> {
    Ok(List {
        id: row.get(0)?,
        parent_id: row.get(8)?,
        name: row.get(1)?,
        revision: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        removed: row.get::<_, Option<i64>>(5)?.is_some(),
        has_children: row.get(9)?,
        style: Style::read(row, 6)?,
    })
}

/// Where a List sorts in the tree as `list_lists` reads it: each level's
/// position and id, from the top level down, so a List comes right after
/// its parent and before the parent's next sibling. Fixed-width parts keep
/// text order the same as (position, id) order on every level.
const TREE_KEY: &str = "printf('%020d', position) || id";

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
        self.audited(AuditAction::TaskDetails, input, |input| {
            self.update_task_details_inner(input)
        })
    }

    fn update_task_details_inner(&self, input: UpdateTaskDetails) -> Result<Task, TaskError> {
        let tags = clean_tags(&input.tags)?;
        let due_date = calendar_date(input.due_date.as_deref())?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
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
        self.audited(AuditAction::TaskMove, input, |input| {
            self.move_task_inner(input)
        })
    }

    fn move_task_inner(&self, input: MoveTask) -> Result<Task, TaskError> {
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let updated = self.move_task_in(&tx, input)?;
        tx.commit()?;
        Ok(updated)
    }

    /// The move inside the caller's write transaction (also used by bulk
    /// changes); the caller commits.
    pub(crate) fn move_task_in(
        &self,
        tx: &rusqlite::Transaction,
        input: MoveTask,
    ) -> Result<Task, TaskError> {
        let old = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM tasks WHERE id=?1"),
                params![input.id],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;
        crate::removed::require_writable_task(tx, &old.id)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.list_id == input.list_id {
            return Err(TaskError::TaskUnchanged);
        }
        if let Some(ref id) = input.list_id {
            require_active_list(tx, id)?;
        }
        let now = current_millis()?;
        // It lands at the top of its group in the new List's Custom Order.
        let position = crate::custom_order::top(tx, input.list_id.as_deref())?;
        let updated = tx.query_row(
            &format!(
                "UPDATE tasks SET list_id=?1,revision=revision+1,
                updated_at=max(updated_at+1,?2),position=?5 WHERE id=?3 AND revision=?4
                RETURNING {COLUMNS}"
            ),
            params![
                input.list_id,
                now,
                input.id,
                input.expected_revision,
                position
            ],
            read_task,
        )?;
        self.audit_applied(tx, AuditAction::TaskMove, &updated.id, updated.revision)?;
        let mut updated = updated;
        hydrate_tags(tx, std::slice::from_mut(&mut updated))?;
        Ok(updated)
    }

    pub fn create_list(&self, input: CreateList) -> Result<List, TaskError> {
        self.create_list_under(&input.name, None)
    }

    /// A new, empty List inside `parent_id`, at the bottom there. Refused when
    /// the parent is hidden by Removed or already sits five levels deep.
    pub fn create_sub_list(&self, input: CreateSubList) -> Result<List, TaskError> {
        self.create_list_under(&input.name, Some(&input.parent_id))
    }

    fn create_list_under(&self, name: &str, parent_id: Option<&str>) -> Result<List, TaskError> {
        let name = list_name(name)?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        if let Some(parent) = parent_id {
            require_active_list(&tx, parent)?;
            Tree::Lists.require_room_under(&tx, parent)?;
        }
        require_unique_list_name(&tx, parent_id, name, None)?;
        let now = current_millis()?;
        let position = manual_order::bottom(&tx, Siblings::lists(parent_id))?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO lists(id, parent_id, name, revision, created_at, updated_at, removed_at, position)
             VALUES (?1, ?2, ?3, 1, ?4, ?4, NULL, ?5)",
            params![id, parent_id, name, now, position],
        )?;
        let list = get_list(&tx, &id)?;
        self.audit_applied(&tx, AuditAction::ListCreate, &list.id, list.revision)?;
        tx.commit()?;
        Ok(list)
    }

    pub fn rename_list(&self, input: RenameList) -> Result<List, TaskError> {
        self.audited(AuditAction::ListRename, input, |input| {
            self.rename_list_inner(input)
        })
    }

    fn rename_list_inner(&self, input: RenameList) -> Result<List, TaskError> {
        let name = list_name(&input.name)?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let old = current_list(&tx, &input.id, input.expected_revision)?;
        if old.name == name {
            return Err(TaskError::ListUnchanged);
        }
        require_unique_list_name(&tx, old.parent_id.as_deref(), name, Some(&old.id))?;
        tx.execute(
            "UPDATE lists SET name=?1 WHERE id=?2",
            params![name, old.id],
        )?;
        let updated = bump_list(&tx, &old)?;
        self.audit_applied(&tx, AuditAction::ListRename, &updated.id, updated.revision)?;
        tx.commit()?;
        Ok(updated)
    }

    /// Puts a List after a sibling, or first, under the parent `input` names
    /// (its own when it names none), with the Lists inside it. Recorded as
    /// `list.move` when that is another parent than the List has as the write
    /// finds it, else as `list.reorder`; a refusal is recorded the same way,
    /// or as a reorder when the List is gone.
    pub fn reorder_list(&self, input: ReorderList) -> Result<List, TaskError> {
        #[cfg(test)]
        crate::privacy::between_reads();
        self.audited_as_decided(input, |input| self.place_list_inner(&input))
    }

    /// Puts a List, with the Lists inside it, exactly where the person dropped
    /// it: under `parent_id` (None: the top level), after `after_id` there.
    pub fn place_list(&self, input: PlaceList) -> Result<List, TaskError> {
        self.reorder_list(ReorderList {
            id: input.id,
            expected_revision: input.expected_revision,
            after_id: input.after_id,
            parent: ListParent::named(input.parent_id),
        })
    }

    /// Reads the List once inside the write and decides from it, before any
    /// check can refuse the change, whether it is a move or a reorder.
    fn place_list_inner(&self, input: &ReorderList) -> Result<List, (AuditAction, TaskError)> {
        let unread = |error| (AuditAction::ListReorder, error);
        let mut conn = self.connect().map_err(unread)?;
        let tx = self.begin_write(&mut conn).map_err(unread)?;
        let old = get_list(&tx, &input.id).map_err(unread)?;
        let parent_id = input.parent.resolve(old.parent_id.as_deref());
        let action = if parent_id == old.parent_id.as_deref() {
            AuditAction::ListReorder
        } else {
            AuditAction::ListMove
        };
        self.write_list_placement(tx, &old, parent_id, input, action)
            .map_err(|error| (action, error))
    }

    fn write_list_placement(
        &self,
        tx: Transaction,
        old: &List,
        parent_id: Option<&str>,
        input: &ReorderList,
        action: AuditAction,
    ) -> Result<List, TaskError> {
        require_current(&tx, old, input.expected_revision)?;
        if parent_id != old.parent_id.as_deref() {
            require_list_destination(&tx, old, parent_id)?;
        }
        let after_id = input.after_id.as_deref();
        if !manual_order::place(&tx, Siblings::lists(parent_id), &old.id, after_id)? {
            return Err(TaskError::ListUnchanged);
        }
        tx.execute(
            "UPDATE lists SET parent_id=?1 WHERE id=?2",
            params![parent_id, old.id],
        )?;
        let placed = bump_list(&tx, old)?;
        self.audit_applied(&tx, action, &placed.id, placed.revision)?;
        tx.commit()?;
        Ok(placed)
    }

    /// Moves a List, with the Lists inside it, under another List or to the
    /// top level, at the bottom there. Refused when it would land inside
    /// itself or anything would sit deeper than five levels.
    pub fn move_list(&self, input: MoveList) -> Result<List, TaskError> {
        self.audited(AuditAction::ListMove, input, |input| {
            self.move_list_inner(input)
        })
    }

    fn move_list_inner(&self, input: MoveList) -> Result<List, TaskError> {
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let old = current_list(&tx, &input.id, input.expected_revision)?;
        let parent_id = input.parent_id.as_deref();
        if old.parent_id.as_deref() == parent_id {
            return Err(TaskError::ListUnchanged);
        }
        require_list_destination(&tx, &old, parent_id)?;
        let position = manual_order::bottom(&tx, Siblings::lists(parent_id))?;
        tx.execute(
            "UPDATE lists SET parent_id=?1, position=?2 WHERE id=?3",
            params![parent_id, position, old.id],
        )?;
        let moved = bump_list(&tx, &old)?;
        self.audit_applied(&tx, AuditAction::ListMove, &moved.id, moved.revision)?;
        tx.commit()?;
        Ok(moved)
    }

    /// Removes or restores a List. The Lists and Tasks inside it are hidden
    /// with it but keep their own disposition (ADR 0002), so restoring it
    /// leaves anything Removed on its own in Removed. A List inside a Removed
    /// List cannot be removed or restored on its own.
    pub fn set_list_removed(&self, input: SetListRemoved) -> Result<List, TaskError> {
        let action = if input.removed {
            AuditAction::ListRemove
        } else {
            AuditAction::ListRestore
        };
        self.audited(action, input, |input| {
            self.set_list_removed_inner(input, action)
        })
    }

    fn set_list_removed_inner(
        &self,
        input: SetListRemoved,
        action: AuditAction,
    ) -> Result<List, TaskError> {
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        let removing = input.removed;
        let changed = self.set_list_removed_in(&tx, input, action)?;
        // Owner decision B: through an AI interface, a List that holds any
        // Task or List waits for the person's approval (`bulk`). Checked
        // after the ordinary refusals; dropping `tx` rolls the change back.
        if removing && self.needs_approval_to_empty() && list_holds_anything(&tx, &changed.id)? {
            return Err(TaskError::NeedsApproval);
        }
        tx.commit()?;
        Ok(changed)
    }

    /// The change inside the caller's write transaction (also used by an
    /// approved Bulk Change); the caller commits.
    pub(crate) fn set_list_removed_in(
        &self,
        tx: &rusqlite::Transaction,
        input: SetListRemoved,
        action: AuditAction,
    ) -> Result<List, TaskError> {
        let old = get_list(tx, &input.id)?;
        if old.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }
        if old.removed == input.removed {
            return Err(TaskError::ListUnchanged);
        }
        // Inside a Removed List this one is hidden with it; that List comes
        // first, whether restoring or removing this one on its own.
        if let Some(parent) = old.parent_id.as_deref()
            && list_hidden(tx, parent)?
        {
            return Err(TaskError::ListRemoved);
        }
        if !input.removed {
            require_unique_list_name(tx, old.parent_id.as_deref(), &old.name, Some(&old.id))?;
        }
        let now = current_millis()?;
        tx.execute(
            "UPDATE lists SET removed_at=?1 WHERE id=?2",
            params![input.removed.then_some(now), old.id],
        )?;
        let changed = bump_list(tx, &old)?;
        self.audit_applied(tx, action, &changed.id, changed.revision)?;
        Ok(changed)
    }

    /// Through an AI interface, removing a List or Note Folder that holds
    /// anything needs the person's approval. The desktop (also when it
    /// applies an approved request) removes directly.
    pub(crate) fn needs_approval_to_empty(&self) -> bool {
        self.interface() != crate::audit::Interface::Desktop
    }

    /// One bounded page of the List tree, parents before the Lists inside
    /// them and siblings in manual order, each naming its `parent_id`.
    /// Without `include_removed`, a List in Removed is left out with every
    /// List inside it.
    pub fn list_lists(
        &self,
        limit: u32,
        cursor: Option<&str>,
        include_removed: bool,
    ) -> Result<ListPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidListPage);
        }
        let scope = if include_removed { "all" } else { "active" };
        let after = match cursor {
            None => None,
            Some(raw) => {
                let bits: Vec<_> = raw.split('|').collect();
                if bits.len() != 3
                    || bits[0] != "v4"
                    || bits[2] != scope
                    || bits[1].is_empty()
                    || !bits[1].bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
                {
                    return Err(TaskError::InvalidListCursor);
                }
                Some(bits[1].to_owned())
            }
        };
        let conn = self.connect()?;
        let mut stmt = conn.prepare(&format!(
            "WITH RECURSIVE tree(id, key, level) AS (
                SELECT id, {TREE_KEY}, 1 FROM lists
                 WHERE parent_id IS NULL AND (?1 OR removed_at IS NULL)
                UNION ALL
                SELECT c.id, tree.key || printf('%020d', c.position) || c.id, tree.level + 1
                  FROM lists c JOIN tree ON c.parent_id = tree.id
                 WHERE (?1 OR c.removed_at IS NULL) AND tree.level < ?4
             )
             SELECT {LIST_COLUMNS}, tree.key FROM tree JOIN lists l ON l.id = tree.id
             WHERE ?2 IS NULL OR tree.key > ?2
             ORDER BY tree.key LIMIT ?3"
        ))?;
        let mut rows = stmt.query(params![
            include_removed,
            after,
            i64::from(limit) + 1,
            WALK_LIMIT
        ])?;
        let mut items = Vec::new();
        let mut keys = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(read_list(row)?);
            keys.push(row.get::<_, String>(10)?);
        }
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            Some(format!("v4|{}|{scope}", keys[items.len() - 1]))
        } else {
            None
        };
        Ok(ListPage { items, next_cursor })
    }
}

/// A List's name is unique, ignoring case, among the Lists not in Removed
/// that share its parent, as with Note Folders (owner decision Q33): the
/// top-level Lists are siblings of each other, and "Work › Reports" and
/// "Home › Reports" may both exist. The same rule for every interface.
fn require_unique_list_name(
    conn: &rusqlite::Connection,
    parent_id: Option<&str>,
    name: &str,
    except: Option<&str>,
) -> Result<(), TaskError> {
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM lists
           WHERE coalesce(parent_id, '') = coalesce(?1, '') AND name = ?2 COLLATE NOCASE
             AND id IS NOT coalesce(?3, '') AND removed_at IS NULL)",
        params![parent_id, name, except],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(TaskError::ListNameConflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::Interface;

    /// Review finding (PR #32): whether a placement is a move or a reorder
    /// is decided from the List as the write reads it, once, so another
    /// writer moving it in between cannot leave the history naming the
    /// wrong action.
    #[test]
    fn a_placement_is_recorded_from_the_list_as_the_write_finds_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("place-race.db");
        let desktop = TaskStore::open(&db).unwrap();
        let ai = TaskStore::open_as(&db, Interface::Cli).unwrap();
        let work = desktop
            .create_list(CreateList {
                name: "Work".into(),
            })
            .unwrap();
        let home = desktop
            .create_list(CreateList {
                name: "Home".into(),
            })
            .unwrap();
        let moving = desktop
            .create_sub_list(CreateSubList {
                name: "Moving".into(),
                parent_id: work.id.clone(),
            })
            .unwrap();
        let (id, home_id) = (moving.id.clone(), home.id.clone());
        crate::privacy::set_between_reads(move || {
            desktop
                .move_list(MoveList {
                    id,
                    expected_revision: moving.revision,
                    parent_id: Some(home_id),
                })
                .unwrap();
        });

        let stale = ai.place_list(PlaceList {
            id: moving.id.clone(),
            expected_revision: moving.revision,
            parent_id: Some(home.id.clone()),
            after_id: None,
        });

        assert!(matches!(stale, Err(TaskError::RevisionConflict)));
        let entries = ai.list_audit_entries(Some(&moving.id), 10, None).unwrap();
        let refused = entries
            .items
            .iter()
            .find(|entry| entry.interface == Interface::Cli)
            .expect("the refusal is recorded");
        assert_eq!(
            refused.action.as_str(),
            "list.reorder",
            "the List already sat inside Home when the write ran"
        );
    }
}
