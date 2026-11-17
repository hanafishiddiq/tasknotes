//! Images in a Task's notes (owner, batch 4, 2026-11-18). An image is kept
//! in the database, so every backup and restore carries it, and the Task's
//! notes hold a short reference, `![image](tasknotes-image:ID)`, that the
//! window draws as the image. Only PNG, JPEG, GIF and WebP, checked by their
//! own first bytes (never SVG, which can carry scripts), at most 10 MB each.
//! An image is never deleted: removing its reference from the notes leaves
//! it in the file, as Removed leaves everything (ADR 0002).

use rusqlite::{OptionalExtension, Row, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditAction;
use crate::tasks::{TaskError, TaskStore, current_millis};

/// The largest image kept: a screenshot or a phone photo fits.
pub const TASK_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// How a reference to an image is written in a Task's notes.
pub const IMAGE_SCHEME: &str = "tasknotes-image:";

/// An image's description, without its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskImage {
    pub id: String,
    pub task_id: String,
    pub media_type: String,
    /// Bytes.
    pub size: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskImage {
    pub task_id: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// The Markdown that shows image `id` in a Task's notes.
pub fn image_reference(id: &str) -> String {
    format!("![image]({IMAGE_SCHEME}{id})")
}

/// The file extension an export gives an image of `media_type`.
pub fn extension_of(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    }
}

/// Whether `bytes` really are an image of `media_type`, by its signature.
fn is_image(media_type: &str, bytes: &[u8]) -> bool {
    match media_type {
        "image/png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

/// Schema v26: images in Tasks' notes. The CHECKs hold whoever writes the
/// file to the core's kinds and size. `IF NOT EXISTS`: a file set back to an
/// older version keeps its images.
pub(crate) fn migrate_v26(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE IF NOT EXISTS task_images (
            id TEXT PRIMARY KEY NOT NULL,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
            media_type TEXT NOT NULL
                CHECK(media_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp')),
            bytes BLOB NOT NULL CHECK(length(bytes) BETWEEN 1 AND 10485760),
            created_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS task_images_task ON task_images(task_id, created_at);
         PRAGMA user_version = 26;",
    )?;
    Ok(())
}

const COLUMNS: &str = "id, task_id, media_type, length(bytes), created_at";

fn read_image(row: &Row<'_>) -> rusqlite::Result<TaskImage> {
    Ok(TaskImage {
        id: row.get(0)?,
        task_id: row.get(1)?,
        media_type: row.get(2)?,
        size: row.get(3)?,
        created_at: row.get(4)?,
    })
}

impl TaskStore {
    /// Keeps an image for a Task's notes; the caller writes its reference
    /// (`image_reference`) into the notes. The Task itself is not written.
    pub fn add_task_image(&self, input: AddTaskImage) -> Result<TaskImage, TaskError> {
        if input.bytes.is_empty()
            || input.bytes.len() > TASK_IMAGE_BYTES
            || !is_image(&input.media_type, &input.bytes)
        {
            return Err(TaskError::InvalidTaskImage);
        }
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = self.begin_write(&mut conn)?;
        crate::removed::require_writable_task(&tx, &input.task_id)?;
        let image = tx.query_row(
            &format!(
                "INSERT INTO task_images (id, task_id, media_type, bytes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING {COLUMNS}"
            ),
            params![
                Uuid::new_v4().to_string(),
                input.task_id,
                input.media_type,
                input.bytes,
                now
            ],
            read_image,
        )?;
        self.audit_applied(&tx, AuditAction::TaskImageAdd, &image.id, None)?;
        tx.commit()?;
        Ok(image)
    }

    /// An image and its bytes, whatever its Task's disposition: a Removed
    /// Task still shows its notes in Removed.
    pub fn read_task_image(&self, id: &str) -> Result<(TaskImage, Vec<u8>), TaskError> {
        let conn = self.connect()?;
        conn.query_row(
            &format!("SELECT {COLUMNS}, bytes FROM task_images WHERE id = ?1"),
            params![id],
            |row| Ok((read_image(row)?, row.get::<_, Vec<u8>>(5)?)),
        )
        .optional()?
        .ok_or(TaskError::TaskImageNotFound)
    }
}
