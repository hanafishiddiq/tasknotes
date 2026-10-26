use rusqlite::{
    OptionalExtension, Row, Transaction, TransactionBehavior, params, params_from_iter,
    types::Value,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::search::{fingerprint, search_terms};
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
    /// None when the Note is not in a folder.
    pub folder_id: Option<String>,
    pub pinned: bool,
    /// In Removed (ADR-0002); never a deletion.
    pub removed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNote {
    pub markdown: String,
    /// The folder to create the Note in; None for no folder.
    #[serde(default)]
    pub folder_id: Option<String>,
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
    pub folder_id: Option<String>,
    pub pinned: bool,
    /// For a search hit: a few plain-text words around the match, matched
    /// words between U+0002 and U+0003. Never the whole Note.
    pub snippet: Option<String>,
}

/// Where a Note listing looks: every Note, or only those directly in one folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "folderId", rename_all = "snake_case")]
pub enum NoteScope {
    #[default]
    All,
    Folder(String),
}

/// Every Note listing goes through this one query: pinned first, then most
/// recently edited.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NoteQuery {
    pub text: String,
    pub folder: NoteScope,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveNote {
    pub id: String,
    /// None takes the Note out of every folder.
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetNotePinned {
    pub id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePage {
    pub items: Vec<NoteSummary>,
    pub next_cursor: Option<String>,
}

pub(crate) const COLUMNS: &str =
    "id, markdown, title, revision, created_at, updated_at, folder_id, pinned, removed_at";

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

const TOKENIZER: &str = "tokenize='unicode61 remove_diacritics 2'";

/// Schema v8, Note part: a Note's folder and pin, list orders that put pinned
/// Notes first, and the Note search index, kept by triggers so every writer
/// of this file (desktop, CLI, MCP) maintains it.
pub(crate) fn migrate_v8(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(&format!(
        "ALTER TABLE notes ADD COLUMN folder_id TEXT REFERENCES note_folders(id) ON DELETE RESTRICT;
         ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1));
         DROP INDEX notes_updated_order;
         CREATE INDEX notes_all_order ON notes(pinned DESC, updated_at DESC, id DESC);
         CREATE INDEX notes_folder_order ON notes(folder_id, pinned DESC, updated_at DESC, id DESC);
         CREATE VIRTUAL TABLE notes_fts USING fts5(
            markdown, content='notes', content_rowid='rowid', {TOKENIZER}
         );
         INSERT INTO notes_fts(notes_fts) VALUES('rebuild');
         CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
           INSERT INTO notes_fts(rowid, markdown) VALUES (new.rowid, new.markdown);
         END;
         CREATE TRIGGER notes_fts_update AFTER UPDATE OF markdown ON notes BEGIN
           INSERT INTO notes_fts(notes_fts, rowid, markdown) VALUES ('delete', old.rowid, old.markdown);
           INSERT INTO notes_fts(rowid, markdown) VALUES (new.rowid, new.markdown);
         END;
         CREATE TRIGGER notes_fts_delete AFTER DELETE ON notes BEGIN
           INSERT INTO notes_fts(notes_fts, rowid, markdown) VALUES ('delete', old.rowid, old.markdown);
         END;"
    ))?;
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

/// A search snippet as the editor would show the words: block markers,
/// inline marks and link targets dropped, lines run together.
fn plain_snippet(raw: &str) -> String {
    let words: Vec<String> = raw
        .lines()
        .map(|line| without_marks(&without_link_targets(without_block_marker(line.trim()))))
        .collect();
    words
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

struct After {
    pinned: bool,
    time: i64,
    id: String,
}

fn parse_cursor(value: &str, key: &str) -> Result<After, TaskError> {
    let fields: Vec<_> = value.split('|').collect();
    let [version, cursor_key, pinned, time, id] = fields[..] else {
        return Err(TaskError::InvalidNoteCursor);
    };
    if version != "n2" || cursor_key != key || Uuid::parse_str(id).is_err() {
        return Err(TaskError::InvalidNoteCursor);
    }
    let pinned = match pinned {
        "1" => true,
        "0" => false,
        _ => return Err(TaskError::InvalidNoteCursor),
    };
    let time = time
        .parse::<i64>()
        .map_err(|_| TaskError::InvalidNoteCursor)?;
    Ok(After {
        pinned,
        time,
        id: id.to_owned(),
    })
}

pub(crate) fn read_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        markdown: row.get(1)?,
        title: row.get(2)?,
        revision: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        folder_id: row.get(6)?,
        pinned: row.get(7)?,
        removed: row.get::<_, Option<i64>>(8)?.is_some(),
    })
}

impl TaskStore {
    pub fn create_note(&self, input: CreateNote) -> Result<Note, TaskError> {
        check_length(&input.markdown)?;
        let folder_id = input.folder_id.as_deref();
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(folder) = folder_id {
            crate::note_folders::require_folder(&tx, folder)?;
        }
        let note = tx.query_row(
            &format!(
                "INSERT INTO notes (id, markdown, title, revision, created_at, updated_at, folder_id)
                 SELECT ?1, ?2, ?3, 1, stamp, stamp, ?5 FROM ({NEXT_STAMP}) RETURNING {COLUMNS}"
            ),
            params![
                Uuid::new_v4().to_string(),
                input.markdown,
                title_of(&input.markdown),
                now,
                folder_id
            ],
            read_note,
        )?;
        tx.commit()?;
        Ok(note)
    }

    /// Placement only: the Markdown, its revision and the edit time stay, so
    /// an editor open on the Note keeps saving without a conflict.
    pub fn move_note(&self, input: MoveNote) -> Result<Note, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::removed::require_writable_note(&tx, &input.id)?;
        if let Some(folder) = input.folder_id.as_deref() {
            crate::note_folders::require_folder(&tx, folder)?;
        }
        let note = tx
            .query_row(
                &format!("UPDATE notes SET folder_id = ?1 WHERE id = ?2 RETURNING {COLUMNS}"),
                params![input.folder_id, input.id],
                read_note,
            )
            .optional()?
            .ok_or(TaskError::NoteNotFound)?;
        tx.commit()?;
        Ok(note)
    }

    /// Placement only, like a move; pinning twice is harmless.
    pub fn set_note_pinned(&self, input: SetNotePinned) -> Result<Note, TaskError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::removed::require_writable_note(&tx, &input.id)?;
        let note = tx.query_row(
            &format!("UPDATE notes SET pinned = ?1 WHERE id = ?2 RETURNING {COLUMNS}"),
            params![input.pinned, input.id],
            read_note,
        )?;
        tx.commit()?;
        Ok(note)
    }

    /// Saves only over the revision the editor started from; a newer edit wins
    /// and the caller decides what to do with its text.
    pub fn update_note(&self, input: UpdateNote) -> Result<Note, TaskError> {
        check_length(&input.markdown)?;
        let now = current_millis()?;
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::removed::require_writable_note(&tx, &input.id)?;
        let updated = tx
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
            .optional()?
            .ok_or(TaskError::NoteRevisionConflict)?;
        tx.commit()?;
        Ok(updated)
    }

    /// Every Note, pinned first, then most recently edited, as a bounded keyset page.
    pub fn list_notes(&self, limit: u32, cursor: Option<&str>) -> Result<NotePage, TaskError> {
        self.query_notes(&NoteQuery::default(), limit, cursor)
    }

    /// Bounded keyset page over one query. A cursor is valid only for the
    /// query that produced it, so pages never repeat or skip across queries.
    pub fn query_notes(
        &self,
        query: &NoteQuery,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<NotePage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let folder_key = match &query.folder {
            NoteScope::All => "all".to_owned(),
            NoteScope::Folder(id) => format!("folder:{id}"),
        };
        let terms = search_terms(&query.text)?;
        let key = fingerprint(&[&folder_key, &terms.join(" ")]);
        let after = cursor.map(|value| parse_cursor(value, &key)).transpose()?;
        // Text with no words in it ("!!!") is a search that matches nothing,
        // not a request to list every Note.
        if terms.is_empty() && !query.text.trim().is_empty() {
            return Ok(NotePage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let mut values: Vec<Value> = Vec::new();
        let mut bind = |value: Value| {
            values.push(value);
            format!("?{}", values.len())
        };
        // Removed Notes, and Notes in a Removed folder or below one, are hidden.
        let mut filters = vec![note_visible!("n").to_owned()];
        // All words must match; each is a quoted prefix term, so user text
        // never reaches FTS5 query syntax.
        let match_terms = terms.join(" ");
        let source = if terms.is_empty() {
            "notes n"
        } else {
            filters.push(format!(
                "notes_fts MATCH {}",
                bind(Value::Text(match_terms.clone()))
            ));
            "notes_fts JOIN notes n ON n.rowid = notes_fts.rowid"
        };
        if let NoteScope::Folder(id) = &query.folder {
            filters.push(format!("n.folder_id = {}", bind(Value::Text(id.clone()))));
        }
        if let Some(after) = &after {
            // A row-value bound lets SQLite seek into the index on later pages.
            filters.push(format!(
                "(n.pinned, n.updated_at, n.id) < ({}, {}, {})",
                bind(Value::Integer(i64::from(after.pinned))),
                bind(Value::Integer(after.time)),
                bind(Value::Text(after.id.clone()))
            ));
        }
        let page_size = bind(Value::Integer(i64::from(limit) + 1));
        let sql = format!(
            "SELECT n.id, n.title, n.revision, n.updated_at, n.folder_id, n.pinned, n.rowid
             FROM {source} WHERE {}
             ORDER BY n.pinned DESC, n.updated_at DESC, n.id DESC LIMIT {page_size}",
            filters.join(" AND ")
        );

        let conn = self.connect()?;
        if let NoteScope::Folder(id) = &query.folder {
            crate::note_folders::require_folder(&conn, id)?;
        }
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt
            .query_map(params_from_iter(values), |row| {
                let summary = NoteSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    revision: row.get(2)?,
                    updated_at: row.get(3)?,
                    folder_id: row.get(4)?,
                    pinned: row.get(5)?,
                    snippet: None,
                };
                Ok((summary, row.get::<_, i64>(6)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        // Snippets only for the page: cutting one reads the whole Note, so
        // doing it for every match before LIMIT cost seconds on long Notes.
        if !terms.is_empty() {
            let mut cut = conn.prepare(
                "SELECT snippet(notes_fts, 0, char(2), char(3), '…', 12)
                 FROM notes_fts WHERE notes_fts MATCH ?1 AND rowid = ?2",
            )?;
            for (summary, rowid) in &mut rows {
                summary.snippet = cut
                    .query_row(params![match_terms, *rowid], |row| row.get::<_, String>(0))
                    .optional()?
                    .map(|raw| plain_snippet(&raw));
            }
        }
        let items: Vec<NoteSummary> = rows.into_iter().map(|(summary, _)| summary).collect();
        let next_cursor = if more {
            items.last().map(|last| {
                format!(
                    "n2|{key}|{}|{}|{}",
                    u8::from(last.pinned),
                    last.updated_at,
                    last.id
                )
            })
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
