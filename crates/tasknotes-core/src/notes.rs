use rusqlite::{OptionalExtension, Row, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tasks::{TaskError, TaskStore, current_millis};

/// A standalone document stored as canonical Markdown (ADR-0003).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub markdown: String,
    /// Derived from the first line on every write; never edited directly.
    pub title: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNote {
    pub markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNote {
    pub id: String,
    pub markdown: String,
    pub expected_revision: i64,
}

/// What a Note list shows, without loading whole documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub revision: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePage {
    pub items: Vec<NoteSummary>,
    pub next_cursor: Option<String>,
}

const COLUMNS: &str = "id, markdown, title, revision, created_at, updated_at";

/// Edit stamps never repeat, so the list follows the order of writes even when
/// several land within one millisecond. `?4` is the current time.
const NEXT_STAMP: &str = "SELECT max(?4, coalesce(max(updated_at) + 1, 0)) AS stamp FROM notes";

/// Schema v6: standalone Notes.
pub(crate) fn migrate_v6(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE notes (
            id TEXT PRIMARY KEY NOT NULL,
            markdown TEXT NOT NULL,
            title TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision >= 1),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
         );
         CREATE INDEX notes_updated_order ON notes(updated_at DESC, id DESC);
         PRAGMA user_version = 6;",
    )?;
    Ok(())
}

const MAX_NOTE_CHARS: usize = 200_000;
const TITLE_CHARS: usize = 100;
const UNTITLED: &str = "New Note";

fn check_length(markdown: &str) -> Result<(), TaskError> {
    if markdown.chars().count() > MAX_NOTE_CHARS {
        return Err(TaskError::NoteTooLong);
    }
    Ok(())
}

/// The first line as the editor shows it: block markers, inline marks, escapes
/// and link targets removed, at most 100 characters. A Note that opens with a
/// code block is titled by its first line of code, as written.
fn title_of(markdown: &str) -> String {
    let mut lines = markdown
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let plain = match lines.next() {
        Some(fence) if fence.starts_with("```") => lines
            .next()
            .filter(|line| !line.starts_with("```"))
            .unwrap_or("")
            .to_owned(),
        Some(line) => without_marks(&without_link_targets(without_block_marker(line))),
        None => String::new(),
    };
    let title: String = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = title.chars().take(TITLE_CHARS).collect();
    match title.trim_end() {
        "" => UNTITLED.to_owned(),
        trimmed => trimmed.to_owned(),
    }
}

/// Drops what the editor reads as a heading, quote or list marker, and a
/// checkbox at the start of a paragraph or after `- `. Anything else, such as
/// `#tag` or `1) step`, is text the editor shows as typed.
fn without_block_marker(line: &str) -> &str {
    let spaced = |rest: &str| rest.starts_with(char::is_whitespace);
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    if (1..=6).contains(&hashes) && spaced(&line[hashes..]) {
        return line[hashes..].trim_start();
    }
    if let Some(rest) = line.strip_prefix('>').filter(|rest| spaced(rest)) {
        return rest.trim_start();
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if let Some(rest) = line[digits..]
        .strip_prefix('.')
        .filter(|rest| digits > 0 && spaced(rest))
    {
        return rest.trim_start();
    }
    let (rest, dashed) = match line.strip_prefix("- ") {
        Some(rest) => (rest.trim_start(), true),
        None => (line, false),
    };
    for checkbox in ["[ ]", "[x]", "[X]", "[]"] {
        if let Some(text) = rest.strip_prefix(checkbox).filter(|text| spaced(text)) {
            return text.trim_start();
        }
    }
    if dashed {
        return rest;
    }
    match line.strip_prefix("* ").or_else(|| line.strip_prefix("+ ")) {
        Some(rest) => rest.trim_start(),
        None => line,
    }
}

/// `[text](url)` becomes `text`.
fn without_link_targets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        if rest[..open].ends_with('\\') {
            out.push_str(&rest[..=open]);
            rest = after;
            continue;
        }
        let link = after.find("](").and_then(|close| {
            after[close + 2..]
                .find(')')
                .map(|end| (&after[..close], close + 2 + end + 1))
        });
        match link {
            Some((label, consumed)) => {
                out.push_str(&rest[..open]);
                out.push_str(label);
                rest = &after[consumed..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Plain text from inline Markdown: a backslash escape gives its literal
/// character, and `*`, `~` and backticks used as marks are dropped. A lone `*`
/// like `5 * 3` and an underscore inside a word like `snake_case` are text.
fn without_marks(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let spaced = |index: Option<usize>| {
        index
            .and_then(|i| chars.get(i))
            .is_none_or(|c| c.is_whitespace())
    };
    let wordy = |index: Option<usize>| {
        index
            .and_then(|i| chars.get(i))
            .is_some_and(|c| c.is_alphanumeric())
    };
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        let (before, after) = (index.checked_sub(1), Some(index + 1));
        match c {
            '\\' if chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_punctuation()) =>
            {
                out.push(chars[index + 1]);
                index += 1;
            }
            '*' | '~' | '`' if !(spaced(before) && spaced(after)) => {}
            '_' if !(wordy(before) && wordy(after)) => {}
            _ => out.push(c),
        }
        index += 1;
    }
    out
}

fn parse_cursor(value: &str) -> Result<(i64, String), TaskError> {
    let mut fields = value.split('|');
    match (fields.next(), fields.next(), fields.next(), fields.next()) {
        (Some("n1"), Some(time), Some(id), None) => {
            let time = time
                .parse::<i64>()
                .map_err(|_| TaskError::InvalidNoteCursor)?;
            Uuid::parse_str(id).map_err(|_| TaskError::InvalidNoteCursor)?;
            Ok((time, id.to_owned()))
        }
        _ => Err(TaskError::InvalidNoteCursor),
    }
}

fn read_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        markdown: row.get(1)?,
        title: row.get(2)?,
        revision: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

impl TaskStore {
    pub fn create_note(&self, input: CreateNote) -> Result<Note, TaskError> {
        check_length(&input.markdown)?;
        let now = current_millis()?;
        Ok(self.connect()?.query_row(
            &format!(
                "INSERT INTO notes ({COLUMNS})
                 SELECT ?1, ?2, ?3, 1, stamp, stamp FROM ({NEXT_STAMP}) RETURNING {COLUMNS}"
            ),
            params![
                Uuid::new_v4().to_string(),
                input.markdown,
                title_of(&input.markdown),
                now
            ],
            read_note,
        )?)
    }

    /// Saves only over the revision the editor started from; a newer edit wins
    /// and the caller decides what to do with its text.
    pub fn update_note(&self, input: UpdateNote) -> Result<Note, TaskError> {
        check_length(&input.markdown)?;
        let now = current_millis()?;
        let conn = self.connect()?;
        let updated = conn
            .query_row(
                &format!(
                    "UPDATE notes SET markdown = ?1, title = ?2, revision = revision + 1,
                       updated_at = ({NEXT_STAMP})
                     WHERE id = ?3 AND revision = ?5 RETURNING {COLUMNS}"
                ),
                params![
                    input.markdown,
                    title_of(&input.markdown),
                    input.id,
                    now,
                    input.expected_revision
                ],
                read_note,
            )
            .optional()?;
        match updated {
            Some(note) => Ok(note),
            None => {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1)",
                    params![input.id],
                    |row| row.get(0),
                )?;
                Err(if exists {
                    TaskError::NoteRevisionConflict
                } else {
                    TaskError::NoteNotFound
                })
            }
        }
    }

    /// Most recently edited first, as a bounded keyset page.
    pub fn list_notes(&self, limit: u32, cursor: Option<&str>) -> Result<NotePage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let (time, id) = cursor.map(parse_cursor).transpose()?.unzip();
        let conn = self.connect()?;
        // A row-value bound lets SQLite seek into the index instead of scanning
        // every newer Note on later pages. The first page binds both as NULL.
        let mut stmt = conn.prepare(&format!(
            "SELECT id, title, revision, updated_at FROM notes {}
             ORDER BY updated_at DESC, id DESC LIMIT ?3",
            if time.is_some() {
                "WHERE (updated_at, id) < (?1, ?2)"
            } else {
                "WHERE ?1 IS NULL AND ?2 IS NULL"
            }
        ))?;
        let mut items = stmt
            .query_map(params![time, id, i64::from(limit) + 1], |row| {
                Ok(NoteSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    revision: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|last| format!("n1|{}|{}", last.updated_at, last.id))
        } else {
            None
        };
        Ok(NotePage { items, next_cursor })
    }

    pub fn get_note(&self, id: &str) -> Result<Option<Note>, TaskError> {
        Ok(self
            .connect()?
            .query_row(
                &format!("SELECT {COLUMNS} FROM notes WHERE id = ?1"),
                params![id],
                read_note,
            )
            .optional()?)
    }
}
