use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// One stable interface for desktop commands today and CLI/MCP adapters later.
#[derive(Debug, Clone)]
pub struct TaskStore {
    path: PathBuf,
}

#[derive(Debug, Error)]
pub enum TaskError {
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
    #[error("Task not found")]
    NotFound,
    #[error("Revision conflict: the Task has changed; reload before saving")]
    RevisionConflict,
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
    fn from_sql(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(Self::Ready),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    fn as_sql(self) -> &'static str {
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

    fn as_sql(self) -> &'static str {
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
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPage {
    pub items: Vec<Task>,
    pub next_cursor: Option<String>,
}

fn current_millis() -> Result<i64, TaskError> {
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

fn invalid_db_text(index: usize, value: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unknown database value {value}"),
        )),
    )
}

fn read_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(3)?;
    let priority: String = row.get(4)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: WorkStatus::from_sql(&status).ok_or_else(|| invalid_db_text(3, status))?,
        priority: Priority::from_sql(&priority).ok_or_else(|| invalid_db_text(4, priority))?,
        progress: row.get(5)?,
        revision: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const COLUMNS: &str =
    "id, title, description, status, priority, progress, revision, created_at, updated_at";

impl TaskStore {
    /// Opens an on-disk database and applies transactional, forward-only migrations.
    pub fn open(path: &Path) -> Result<Self, TaskError> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }

        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            return Err(TaskError::UnsupportedSchema(version));
        }

        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        if version == 0 {
            let migration = connection.transaction()?;
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
            migration.commit()?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    fn connect(&self) -> Result<Connection, TaskError> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(connection)
    }

    pub fn create_task(&self, input: CreateTask) -> Result<Task, TaskError> {
        let (title, description) = validated_text(&input.title, &input.description)?;
        let now = current_millis()?;
        let task = Task {
            id: Uuid::new_v4().to_string(),
            title,
            description,
            status: WorkStatus::Ready,
            priority: Priority::Medium,
            progress: 0,
            revision: 1,
            created_at: now,
            updated_at: now,
        };
        self.connect()?.execute(
            "INSERT INTO tasks
             (id, title, description, status, priority, progress, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                task.id,
                task.title,
                task.description,
                task.status.as_sql(),
                task.priority.as_sql(),
                task.progress,
                task.revision,
                task.created_at,
                task.updated_at
            ],
        )?;
        Ok(task)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>, TaskError> {
        let conn = self.connect()?;
        let sql = format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1");
        Ok(conn.query_row(&sql, params![id], read_task).optional()?)
    }

    pub fn update_task(&self, input: UpdateTask) -> Result<Task, TaskError> {
        let (title, description) = validated_text(&input.title, &input.description)?;
        let now = current_millis()?;
        let conn = self.connect()?;
        let sql = format!(
            "UPDATE tasks SET title = ?1, description = ?2,
              revision = revision + 1, updated_at = max(updated_at + 1, ?5)
             WHERE id = ?3 AND revision = ?4 RETURNING {COLUMNS}"
        );
        let updated = conn
            .query_row(
                &sql,
                params![title, description, input.id, input.expected_revision, now],
                read_task,
            )
            .optional()?;

        match updated {
            Some(task) => Ok(task),
            None => {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
                    params![input.id],
                    |row| row.get(0),
                )?;
                Err(if exists {
                    TaskError::RevisionConflict
                } else {
                    TaskError::NotFound
                })
            }
        }
    }

    /// Cursor pagination is bounded to 100 records to protect desktop and future AI callers.
    pub fn list_tasks(&self, limit: u32, cursor: Option<&str>) -> Result<TaskPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }

        let (after_time, after_id): (Option<i64>, Option<String>) = match cursor {
            None => (None, None),
            Some(cursor) => {
                let (time, id) = cursor.split_once(':').ok_or(TaskError::InvalidCursor)?;
                let time: i64 = time.parse().map_err(|_| TaskError::InvalidCursor)?;
                Uuid::parse_str(id).map_err(|_| TaskError::InvalidCursor)?;
                (Some(time), Some(id.to_owned()))
            }
        };

        let conn = self.connect()?;
        let sql = format!(
            "SELECT {COLUMNS} FROM tasks
             WHERE (?1 IS NULL OR created_at < ?1 OR (created_at = ?1 AND id < ?2))
             ORDER BY created_at DESC, id DESC LIMIT ?3"
        );
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query(params![after_time, after_id, i64::from(limit) + 1])?;
        let mut items = Vec::with_capacity(limit as usize + 1);
        while let Some(row) = rows.next()? {
            items.push(read_task(row)?);
        }

        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|task| format!("{}:{}", task.created_at, task.id))
        } else {
            None
        };

        Ok(TaskPage { items, next_cursor })
    }
}
