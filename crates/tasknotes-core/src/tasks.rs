use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params, types::Type};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub use crate::organization::{
    CreateList, List, ListPage, MoveTask, RenameList, SetListRemoved, TaskView, UpdateTaskDetails,
};
pub use crate::search::{TaskQuery, TaskScope};

/// One stable interface for desktop commands today and CLI/MCP adapters later.
#[derive(Debug, Clone)]
pub struct TaskStore {
    path: PathBuf,
}

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("List name cannot be empty or exceed 100 characters")]
    InvalidListName,
    #[error("A List with this name already exists")]
    ListNameConflict,
    #[error("List not found")]
    ListNotFound,
    #[error("List has been removed; restore it before assigning Tasks")]
    ListRemoved,
    #[error("This List is already in the requested state")]
    ListUnchanged,
    #[error("Invalid List cursor or page size")]
    InvalidListPage,
    #[error("Use at most 16 distinct Tags, each 1–40 characters without control characters")]
    InvalidTags,
    #[error("Deadline must be a real local calendar date in YYYY-MM-DD format")]
    InvalidDeadline,
    #[error("Task title cannot be empty")]
    EmptyTitle,
    #[error("Task title must be 200 characters or fewer")]
    TitleTooLong,
    #[error("Task description must be 20,000 characters or fewer")]
    DescriptionTooLong,
    #[error("Page size must be between 1 and 100")]
    InvalidPageSize,
    #[error("Invalid task page cursor")]
    InvalidCursor,
    #[error("Search can use at most 16 words")]
    TooManySearchWords,
    #[error("Task not found")]
    NotFound,
    #[error("Progress update must change a field or include a non-empty note")]
    EmptyProgressUpdate,
    #[error("Nothing to save: the Task already has these details")]
    TaskUnchanged,
    #[error("Next Step cannot exceed 4,000 characters")]
    NextStepTooLong,
    #[error("Progress note cannot exceed 10,000 characters")]
    ProgressNoteTooLong,
    #[error("History page size must be between 1 and 50")]
    InvalidHistoryPageSize,
    #[error("History cursor is invalid")]
    InvalidHistoryCursor,
    #[error("Task changes must specify status, priority, or progress")]
    EmptyChange,
    #[error("Progress must be between 0 and 100")]
    InvalidProgress,
    #[error("Invalid work status/progress transition: {0}")]
    InvalidTransition(&'static str),
    #[error("This Task is in Removed; restore it before changing it")]
    TaskRemoved,
    #[error("This Task's List is in Removed; restore the List first")]
    TaskListRemoved,
    #[error("This item is already in the requested state")]
    RemovedUnchanged,
    #[error("Revision conflict: the Task has changed; reload before saving")]
    RevisionConflict,
    #[error("Note not found")]
    NoteNotFound,
    #[error("Note must be 200,000 characters or fewer")]
    NoteTooLong,
    #[error("Invalid Note page cursor")]
    InvalidNoteCursor,
    #[error("This Note changed elsewhere; your text was not saved over it")]
    NoteRevisionConflict,
    #[error("Folder name cannot be empty, contain control characters or exceed 100 characters")]
    InvalidNoteFolderName,
    #[error("A folder with this name already exists here")]
    NoteFolderNameConflict,
    #[error("Folder not found")]
    NoteFolderNotFound,
    #[error("Folders can be nested at most 5 levels deep")]
    NoteFolderTooDeep,
    #[error("A folder can't be moved into itself or one of its subfolders")]
    NoteFolderCycle,
    #[error("This folder changed elsewhere; reload before saving")]
    NoteFolderRevisionConflict,
    #[error("This folder is already there with this name")]
    NoteFolderUnchanged,
    #[error("Invalid folder page cursor")]
    InvalidNoteFolderCursor,
    #[error("This link was not found or is already unlinked")]
    LinkNotFound,
    #[error("Invalid Note Link page cursor")]
    InvalidLinkCursor,
    #[error("Unsupported database schema version {0}; refusing to modify a newer database")]
    UnsupportedSchema(i64),
    #[error("Unable to open local storage: {0}")]
    Io(#[from] std::io::Error),
    #[error("Local storage failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("System clock is unavailable")]
    Clock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Ready,
    InProgress,
    Completed,
}

impl WorkStatus {
    pub(crate) fn from_sql(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(Self::Ready),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    pub(crate) fn rank(self) -> i64 {
        match self {
            Self::InProgress => 0,
            Self::Ready => 1,
            Self::Completed => 2,
        }
    }

    pub(crate) fn as_sql(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

impl Priority {
    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "very_low" => Some(Self::VeryLow),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "very_high" => Some(Self::VeryHigh),
            _ => None,
        }
    }

    pub(crate) fn rank(self) -> i64 {
        match self {
            Self::VeryLow => 1,
            Self::Low => 2,
            Self::Medium => 3,
            Self::High => 4,
            Self::VeryHigh => 5,
        }
    }

    pub(crate) fn as_sql(self) -> &'static str {
        match self {
            Self::VeryLow => "very_low",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::VeryHigh => "very_high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: WorkStatus,
    pub priority: Priority,
    pub progress: u8,
    pub next_step: String,
    pub list_id: Option<String>,
    pub due_date: Option<String>,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// In Removed (ADR-0002): a disposition beside the work status, never a deletion.
    pub removed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    #[default]
    Priority,
    Newest,
}

impl SortMode {
    pub(crate) fn as_cursor(self) -> &'static str {
        match self {
            Self::Priority => "priority",
            Self::Newest => "newest",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeTask {
    pub id: String,
    pub expected_revision: i64,
    pub status: Option<WorkStatus>,
    pub priority: Option<Priority>,
    pub progress: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordProgress {
    pub id: String,
    pub expected_revision: i64,
    pub status: Option<WorkStatus>,
    pub progress: Option<u8>,
    /// None leaves the field intact. Some("") explicitly clears the field.
    pub next_step: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEntry {
    pub sequence: i64,
    pub id: String,
    pub task_id: String,
    pub previous_status: WorkStatus,
    pub status: WorkStatus,
    pub previous_progress: u8,
    pub progress: u8,
    pub note: String,
    pub next_step: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressHistoryPage {
    pub items: Vec<ProgressEntry>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPage {
    pub items: Vec<Task>,
    pub next_cursor: Option<String>,
}

pub(crate) fn current_millis() -> Result<i64, TaskError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TaskError::Clock)?
        .as_millis();
    i64::try_from(millis).map_err(|_| TaskError::Clock)
}

fn validated_text(title: &str, description: &str) -> Result<(String, String), TaskError> {
    let clean_title = title.trim();
    if clean_title.is_empty() {
        return Err(TaskError::EmptyTitle);
    }
    if clean_title.chars().count() > 200 {
        return Err(TaskError::TitleTooLong);
    }
    if description.chars().count() > 20_000 {
        return Err(TaskError::DescriptionTooLong);
    }
    Ok((clean_title.to_owned(), description.to_owned()))
}

pub(crate) fn invalid_db_text(index: usize, value: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unknown database value {value}"),
        )),
    )
}

pub(crate) fn read_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(3)?;
    let priority: String = row.get(4)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: WorkStatus::from_sql(&status).ok_or_else(|| invalid_db_text(3, status))?,
        priority: Priority::from_sql(&priority).ok_or_else(|| invalid_db_text(4, priority))?,
        progress: row.get(5)?,
        next_step: row.get(9)?,
        list_id: row.get(10)?,
        due_date: row.get(11)?,
        tags: Vec::new(),
        revision: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        removed: row.get::<_, Option<i64>>(12)?.is_some(),
    })
}

pub(crate) const COLUMNS: &str = "id, title, description, status, priority, progress, revision, created_at, updated_at, next_step, list_id, due_date, removed_at";

/// How long a write waits for another process (the CLI, MCP) holding the database lock.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

impl TaskStore {
    /// Opens an on-disk database and applies transactional, forward-only migrations.
    pub fn open(path: &Path) -> Result<Self, TaskError> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }

        let mut connection = Connection::open(path)?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 8 {
            return Err(TaskError::UnsupportedSchema(version));
        }

        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        if version < 2 {
            let migration = connection.transaction()?;
            if version == 0 {
                migration.execute_batch(
                    "CREATE TABLE IF NOT EXISTS tasks (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
                    description TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT 'ready'
                        CHECK (status IN ('ready', 'in_progress', 'completed')),
                    priority TEXT NOT NULL DEFAULT 'medium'
                        CHECK (priority IN ('very_low', 'low', 'medium', 'high', 'very_high')),
                    progress INTEGER NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
                    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS tasks_created_order
                  ON tasks(created_at DESC, id DESC);
                PRAGMA user_version = 1;",
                )?;
            }
            migration.execute_batch(
                "ALTER TABLE tasks ADD COLUMN previous_incomplete_progress
                 INTEGER NOT NULL DEFAULT 0 CHECK (previous_incomplete_progress BETWEEN 0 AND 99);
                 UPDATE tasks SET previous_incomplete_progress = progress
                   WHERE status = 'in_progress' AND progress BETWEEN 0 AND 99;
                 CREATE INDEX IF NOT EXISTS tasks_status_priority_order ON tasks (
                     CASE status WHEN 'in_progress' THEN 0 WHEN 'ready' THEN 1 ELSE 2 END ASC,
                     CASE priority WHEN 'very_high' THEN 5 WHEN 'high' THEN 4
                       WHEN 'medium' THEN 3 WHEN 'low' THEN 2 ELSE 1 END DESC,
                     created_at DESC, id DESC
                 );
                 CREATE INDEX IF NOT EXISTS tasks_status_newest_order ON tasks (
                     CASE status WHEN 'in_progress' THEN 0 WHEN 'ready' THEN 1 ELSE 2 END ASC,
                     created_at DESC, id DESC
                 );
                 PRAGMA user_version = 2;",
            )?;
            migration.commit()?;
        }
        if version < 3 {
            let migration = connection.transaction()?;
            migration.execute_batch(
                "ALTER TABLE tasks ADD COLUMN next_step TEXT NOT NULL DEFAULT '';
                 CREATE TABLE progress_entries (
                    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    id TEXT NOT NULL UNIQUE,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
                    previous_status TEXT NOT NULL CHECK(previous_status IN ('ready', 'in_progress', 'completed')),
                    status TEXT NOT NULL CHECK(status IN ('ready', 'in_progress', 'completed')),
                    previous_progress INTEGER NOT NULL CHECK(previous_progress BETWEEN 0 AND 100),
                    progress INTEGER NOT NULL CHECK(progress BETWEEN 0 AND 100),
                    note TEXT NOT NULL DEFAULT '',
                    next_step TEXT NOT NULL DEFAULT '',
                    created_at INTEGER NOT NULL
                 );
                 CREATE INDEX progress_entries_task_sequence ON progress_entries(task_id, sequence DESC);
                 PRAGMA user_version = 3;",
            )?;
            migration.commit()?;
        }
        if version < 4 {
            let migration = connection.transaction()?;
            migration.execute_batch(
                "CREATE TABLE lists (
                    id TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
                    revision INTEGER NOT NULL CHECK(revision >= 1),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    removed_at INTEGER
                 );
                 CREATE UNIQUE INDEX lists_active_name ON lists(name COLLATE NOCASE)
                   WHERE removed_at IS NULL;
                 CREATE INDEX lists_created_order ON lists(created_at DESC, id DESC);
                 ALTER TABLE tasks ADD COLUMN list_id TEXT REFERENCES lists(id) ON DELETE RESTRICT;
                 ALTER TABLE tasks ADD COLUMN due_date TEXT
                   CHECK(due_date IS NULL OR length(due_date) = 10);
                 CREATE INDEX tasks_list_created ON tasks(list_id, created_at DESC, id DESC);
                 CREATE TABLE tags (
                    name TEXT PRIMARY KEY COLLATE NOCASE NOT NULL
                      CHECK(length(trim(name)) BETWEEN 1 AND 40),
                    fold TEXT NOT NULL UNIQUE
                 );
                 CREATE TABLE task_tags (
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
                    tag_name TEXT NOT NULL REFERENCES tags(name) ON DELETE RESTRICT,
                    PRIMARY KEY(task_id, tag_name)
                 );
                 CREATE INDEX task_tags_tag_name ON task_tags(tag_name, task_id);
                 PRAGMA user_version = 4;",
            )?;
            migration.commit()?;
        }
        if version < 5 {
            let migration = connection.transaction()?;
            crate::search::migrate_v5(&migration)?;
            migration.commit()?;
        }
        if version < 6 {
            let migration = connection.transaction()?;
            crate::notes::migrate_v6(&migration)?;
            migration.commit()?;
        }
        if version < 7 {
            let migration = connection.transaction()?;
            crate::links::migrate_v7(&migration)?;
            migration.commit()?;
        }
        if version < 8 {
            let migration = connection.transaction()?;
            crate::note_folders::migrate_v8(&migration)?;
            crate::notes::migrate_v8(&migration)?;
            crate::organization::refold_tags(&migration)?;
            migration.execute_batch("PRAGMA user_version = 8;")?;
            migration.commit()?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn connect(&self) -> Result<Connection, TaskError> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(connection)
    }

    pub fn create_task(&self, input: CreateTask) -> Result<Task, TaskError> {
        self.create_task_in_list(input, None)
    }

    /// A Task goes to Inbox when list_id is absent, or directly to an active List.
    pub fn create_task_in_list(
        &self,
        input: CreateTask,
        list_id: Option<&str>,
    ) -> Result<Task, TaskError> {
        let (title, description) = validated_text(&input.title, &input.description)?;
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(id) = list_id {
            crate::organization::require_active_list(&tx, id)?;
        }
        let task = Task {
            id: Uuid::new_v4().to_string(),
            title,
            description,
            status: WorkStatus::Ready,
            priority: Priority::Medium,
            progress: 0,
            next_step: String::new(),
            list_id: list_id.map(str::to_owned),
            due_date: None,
            tags: Vec::new(),
            revision: 1,
            created_at: now,
            updated_at: now,
            removed: false,
        };
        tx.execute(
            "INSERT INTO tasks
             (id, title, description, status, priority, progress, revision, created_at,
              updated_at, next_step, list_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                task.id,
                task.title,
                task.description,
                task.status.as_sql(),
                task.priority.as_sql(),
                task.progress,
                task.revision,
                task.created_at,
                task.updated_at,
                task.next_step,
                task.list_id,
            ],
        )?;
        tx.commit()?;
        Ok(task)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>, TaskError> {
        let conn = self.connect()?;
        let sql = format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1");
        let mut task = conn.query_row(&sql, params![id], read_task).optional()?;
        if let Some(ref mut item) = task {
            crate::organization::hydrate_tags(&conn, std::slice::from_mut(item))?;
        }
        Ok(task)
    }

    pub fn update_task(&self, input: UpdateTask) -> Result<Task, TaskError> {
        let (title, description) = validated_text(&input.title, &input.description)?;
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::removed::require_writable_task(&tx, &input.id)?;
        let sql = format!(
            "UPDATE tasks SET title = ?1, description = ?2,
              revision = revision + 1, updated_at = max(updated_at + 1, ?5)
             WHERE id = ?3 AND revision = ?4 RETURNING {COLUMNS}"
        );
        let mut task = tx
            .query_row(
                &sql,
                params![title, description, input.id, input.expected_revision, now],
                read_task,
            )
            .optional()?
            .ok_or(TaskError::RevisionConflict)?;
        crate::organization::hydrate_tags(&tx, std::slice::from_mut(&mut task))?;
        tx.commit()?;
        Ok(task)
    }

    /// Applies a status, priority, or percentage change atomically under the same
    /// optimistic revision rules used by ordinary Task editing.
    ///
    /// Ready means not yet started. An in-progress Task does not become Ready when
    /// its percentage drops to zero; resetting previously started work is not implicit.
    pub fn change_task(&self, input: ChangeTask) -> Result<Task, TaskError> {
        self.apply_task_change(input, None, None)
    }

    /// Record a dated update to a Task's progress and/or Next Step; a note can
    /// stand alone so the user can log work without changing the percentage.
    pub fn record_progress(&self, input: RecordProgress) -> Result<Task, TaskError> {
        self.apply_task_change(
            ChangeTask {
                id: input.id,
                expected_revision: input.expected_revision,
                status: input.status,
                progress: input.progress,
                priority: None,
            },
            input.next_step,
            input.note,
        )
    }

    fn apply_task_change(
        &self,
        input: ChangeTask,
        next_step: Option<String>,
        note: Option<String>,
    ) -> Result<Task, TaskError> {
        let note = note.unwrap_or_default();
        if note.chars().count() > 10_000 {
            return Err(TaskError::ProgressNoteTooLong);
        }
        let note = note.trim().to_owned();
        if next_step
            .as_ref()
            .is_some_and(|value| value.chars().count() > 4_000)
        {
            return Err(TaskError::NextStepTooLong);
        }
        if input.status.is_none()
            && input.priority.is_none()
            && input.progress.is_none()
            && next_step.is_none()
            && note.is_empty()
        {
            return Err(TaskError::EmptyProgressUpdate);
        }
        if input.progress.is_some_and(|progress| progress > 100) {
            return Err(TaskError::InvalidProgress);
        }

        let mut conn = self.connect()?;
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sql =
            format!("SELECT {COLUMNS}, previous_incomplete_progress FROM tasks WHERE id = ?1");
        let current = transaction
            .query_row(&sql, params![input.id], |row| {
                Ok((read_task(row)?, row.get::<_, u8>(13)?))
            })
            .optional()?;
        let (before, mut previous_incomplete) = current.ok_or(TaskError::NotFound)?;
        crate::removed::require_writable_task(&transaction, &before.id)?;
        if before.revision != input.expected_revision {
            return Err(TaskError::RevisionConflict);
        }

        let mut next_status = before.status;
        let mut next_progress = before.progress;
        if let Some(progress) = input.progress {
            next_progress = progress;
            next_status = if progress == 100 {
                WorkStatus::Completed
            } else if progress == 0 && before.status == WorkStatus::Ready {
                WorkStatus::Ready
            } else {
                WorkStatus::InProgress
            };
        }
        if let Some(status) = input.status {
            match status {
                WorkStatus::Ready => {
                    if before.status != WorkStatus::Ready || next_progress != 0 {
                        return Err(TaskError::InvalidTransition(
                            "Ready is only available for work that has not started",
                        ));
                    }
                    next_status = WorkStatus::Ready;
                }
                WorkStatus::InProgress => {
                    if next_progress == 100 && input.progress.is_some() {
                        return Err(TaskError::InvalidTransition(
                            "In Progress cannot have 100% completion",
                        ));
                    }
                    if before.status == WorkStatus::Completed && input.progress.is_none() {
                        next_progress = previous_incomplete;
                    }
                    next_status = WorkStatus::InProgress;
                }
                WorkStatus::Completed => {
                    if input.progress.is_some_and(|progress| progress != 100) {
                        return Err(TaskError::InvalidTransition(
                            "Completed requires progress of 100%",
                        ));
                    }
                    next_status = WorkStatus::Completed;
                    next_progress = 100;
                }
            }
        }
        if next_progress < 100 {
            previous_incomplete = next_progress;
        }
        let next_priority = input.priority.unwrap_or(before.priority);
        let new_next_step = next_step
            .map(|value| value.trim().to_owned())
            .unwrap_or_else(|| before.next_step.clone());
        let substantive = next_status != before.status
            || next_progress != before.progress
            || new_next_step != before.next_step
            || !note.is_empty();
        if !substantive && next_priority == before.priority {
            return Err(TaskError::EmptyProgressUpdate);
        }
        let now = current_millis()?;
        let sql = format!(
            "UPDATE tasks SET
                status = ?1, priority = ?2, progress = ?3,
                previous_incomplete_progress = ?4,
                next_step = ?5,
                revision = revision + 1,
                updated_at = max(updated_at + 1, ?6)
             WHERE id = ?7 AND revision = ?8
             RETURNING {COLUMNS}"
        );
        let updated = transaction.query_row(
            &sql,
            params![
                next_status.as_sql(),
                next_priority.as_sql(),
                next_progress,
                previous_incomplete,
                new_next_step,
                now,
                input.id,
                input.expected_revision
            ],
            read_task,
        )?;
        if substantive {
            transaction.execute(
                "INSERT INTO progress_entries
                  (id, task_id, previous_status, status, previous_progress, progress,
                   note, next_step, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    Uuid::new_v4().to_string(),
                    updated.id,
                    before.status.as_sql(),
                    updated.status.as_sql(),
                    before.progress,
                    updated.progress,
                    note,
                    updated.next_step,
                    updated.updated_at
                ],
            )?;
        }
        transaction.commit()?;
        let mut updated = updated;
        crate::organization::hydrate_tags(&conn, std::slice::from_mut(&mut updated))?;
        Ok(updated)
    }

    /// Recent Progress Entries in reverse write order. The sequence is an
    /// append-only cursor, so entries written in the same millisecond do not
    /// reorder or disappear when moving between pages.
    pub fn list_progress_entries(
        &self,
        task_id: &str,
        limit: u32,
        before: Option<i64>,
    ) -> Result<ProgressHistoryPage, TaskError> {
        if !(1..=50).contains(&limit) {
            return Err(TaskError::InvalidHistoryPageSize);
        }
        if before.is_some_and(|cursor| cursor <= 0) {
            return Err(TaskError::InvalidHistoryCursor);
        }
        let conn = self.connect()?;
        if !conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
            params![task_id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(TaskError::NotFound);
        }
        let mut statement = conn.prepare(
            "SELECT sequence, id, task_id, previous_status, status,
                    previous_progress, progress, note, next_step, created_at
             FROM progress_entries
             WHERE task_id = ?1 AND (?2 IS NULL OR sequence < ?2)
             ORDER BY sequence DESC LIMIT ?3",
        )?;
        let mut rows = statement.query(params![task_id, before, i64::from(limit) + 1])?;
        let mut items = Vec::with_capacity(limit as usize + 1);
        while let Some(row) = rows.next()? {
            let prev: String = row.get(3)?;
            let next: String = row.get(4)?;
            items.push(ProgressEntry {
                sequence: row.get(0)?,
                id: row.get(1)?,
                task_id: row.get(2)?,
                previous_status: WorkStatus::from_sql(&prev)
                    .ok_or_else(|| invalid_db_text(3, prev))?,
                status: WorkStatus::from_sql(&next).ok_or_else(|| invalid_db_text(4, next))?,
                previous_progress: row.get(5)?,
                progress: row.get(6)?,
                note: row.get(7)?,
                next_step: row.get(8)?,
                created_at: row.get(9)?,
            });
        }
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|entry| entry.sequence)
        } else {
            None
        };
        Ok(ProgressHistoryPage { items, next_cursor })
    }

    /// Backwards-compatible default: work status is always the outer sort key,
    /// then highest priority, newest creation time, and stable UUID tie breaker.
    pub fn list_tasks(&self, limit: u32, cursor: Option<&str>) -> Result<TaskPage, TaskError> {
        self.list_tasks_sorted(limit, cursor, SortMode::Priority)
    }

    /// Keyset pages retain the fixed work-status grouping under either secondary
    /// sort. Cursors are tied to their sort mode and validated before querying.
    pub fn list_tasks_sorted(
        &self,
        limit: u32,
        cursor: Option<&str>,
        sort: SortMode,
    ) -> Result<TaskPage, TaskError> {
        self.list_tasks_in_view(limit, cursor, sort, TaskView::All)
    }

    /// Pagination filters each Inbox/List before taking a bounded page.
    pub fn list_tasks_in_view(
        &self,
        limit: u32,
        cursor: Option<&str>,
        sort: SortMode,
        view: TaskView,
    ) -> Result<TaskPage, TaskError> {
        self.query_tasks(
            &TaskQuery {
                view,
                sort,
                ..TaskQuery::default()
            },
            limit,
            cursor,
        )
    }
}
