//! Export of Tasks and Notes to JSON or Markdown (Ticket 13).
//!
//! Everything is read inside one read transaction, so an export is a
//! consistent picture even while other processes write. Removed items are
//! exported and labeled (they stay recoverable in Tasknotes; an export
//! deletes nothing). Note Folders keep their tree: JSON records each
//! folder's `parent_id`, Markdown nests directories.
//!
//! Exclude from AI (ADR-0004): an AI-facing interface exports exactly what
//! it may read, never an excluded Note or a link to one, and cannot ask for
//! more (`PrivateExportDesktopOnly`). The desktop is the person themself:
//! private Notes are left out unless they choose to include them.
//!
//! An export never overwrites: it is written beside the destination under a
//! hidden partial name, every file created fresh (0600, directories 0700,
//! like backups) and synced, then moved into place without replacing
//! anything (`publish`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::calendar::TimeZone;
use crate::tasks::{Priority, Task, TaskError, TaskStore, WorkStatus, current_millis};

/// The version of the export document's shape, sent as `format_version`.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// One JSON file with every record and its ids.
    Json,
    /// A directory: `Tasks/<List>.md` and `Notes/<folder>/<Note>.md`.
    Markdown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub format: ExportFormat,
    /// A file (JSON) or directory (Markdown) that must not exist yet.
    pub destination: PathBuf,
    /// Include Notes marked Exclude from AI. Only the desktop may ask.
    pub include_private_notes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub destination: PathBuf,
    pub format: ExportFormat,
    pub lists: usize,
    pub tasks: usize,
    pub note_folders: usize,
    pub notes: usize,
    pub note_links: usize,
    /// How many of the exported Notes are marked Exclude from AI.
    pub private_notes: usize,
}

#[derive(Serialize)]
struct Document {
    format: &'static str,
    format_version: u32,
    schema_version: i64,
    exported_at: i64,
    includes_private_notes: bool,
    lists: Vec<ListRecord>,
    tasks: Vec<TaskRecord>,
    note_folders: Vec<FolderRecord>,
    notes: Vec<NoteRecord>,
    note_links: Vec<LinkRecord>,
}

#[derive(Serialize)]
struct ListRecord {
    id: String,
    name: String,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    removed: bool,
}

#[derive(Serialize)]
struct TaskRecord {
    id: String,
    title: String,
    description: String,
    status: WorkStatus,
    priority: Priority,
    progress: i64,
    next_step: String,
    list_id: Option<String>,
    deadline: Option<String>,
    tags: Vec<String>,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    removed: bool,
    progress_entries: Vec<ProgressRecord>,
}

#[derive(Serialize)]
struct ProgressRecord {
    created_at: i64,
    previous_status: String,
    status: String,
    previous_progress: i64,
    progress: i64,
    note: String,
    next_step: String,
}

#[derive(Serialize)]
struct FolderRecord {
    id: String,
    parent_id: Option<String>,
    name: String,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    removed: bool,
}

#[derive(Serialize)]
struct NoteRecord {
    id: String,
    folder_id: Option<String>,
    title: String,
    markdown: String,
    pinned: bool,
    exclude_from_ai: bool,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    removed: bool,
}

#[derive(Serialize)]
struct LinkRecord {
    id: String,
    task_id: String,
    note_id: String,
    linked_at: i64,
    unlinked_at: Option<i64>,
}

impl TaskStore {
    /// Write every Task, List, Note Folder, Note and Note Link this
    /// interface may read to a new file or directory.
    pub fn export_data(&self, request: ExportRequest) -> Result<ExportSummary, TaskError> {
        if request.include_private_notes && self.hides_private() {
            return Err(TaskError::PrivateExportDesktopOnly);
        }
        let destination = request.destination.clone();
        let name = destination
            .file_name()
            .ok_or(TaskError::InvalidExportDestination)?
            .to_string_lossy()
            .into_owned();
        if destination.symlink_metadata().is_ok() {
            return Err(TaskError::ExportDestinationExists);
        }
        let mut conn = self.connect()?;
        let document = {
            // One read transaction: the whole export is one consistent state.
            let tx = conn.transaction()?;
            let document = self.read_document(&tx, request.include_private_notes)?;
            tx.commit()?;
            document
        };
        let summary = ExportSummary {
            destination: destination.clone(),
            format: request.format,
            lists: document.lists.len(),
            tasks: document.tasks.len(),
            note_folders: document.note_folders.len(),
            notes: document.notes.len(),
            note_links: document.note_links.len(),
            private_notes: document.notes.iter().filter(|n| n.exclude_from_ai).count(),
        };
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let partial = parent.join(format!(".{name}.partial-{}", Uuid::new_v4().simple()));
        let written = match request.format {
            ExportFormat::Json => serde_json::to_vec_pretty(&document)
                .map_err(std::io::Error::other)
                .and_then(|bytes| write_private(&partial, &bytes)),
            ExportFormat::Markdown => write_markdown(&document, &partial),
        };
        let finished = written
            .and_then(|()| publish(&partial, &destination, request.format))
            .and_then(|()| crate::backup::sync_dir(parent));
        if let Err(error) = finished {
            let _ = fs::remove_dir_all(&partial).or_else(|_| fs::remove_file(&partial));
            return Err(if error.kind() == std::io::ErrorKind::AlreadyExists {
                TaskError::ExportDestinationExists
            } else {
                error.into()
            });
        }
        Ok(summary)
    }

    fn read_document(
        &self,
        conn: &Connection,
        include_private: bool,
    ) -> Result<Document, TaskError> {
        // An AI interface never gets here with `include_private` (refused
        // above), so it always reads only what is shared with AI.
        let note_shown = if include_private {
            self.note_shown_sql("notes")
        } else {
            crate::privacy::shared_with_ai_sql("notes")
        };
        let lists = collect(
            conn,
            "SELECT id, name, revision, created_at, updated_at, removed_at FROM lists
             ORDER BY created_at, id",
            |row| {
                Ok(ListRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    revision: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    removed: row.get::<_, Option<i64>>(5)?.is_some(),
                })
            },
        )?;
        let mut tasks: Vec<Task> = collect(
            conn,
            &format!(
                "SELECT {} FROM tasks ORDER BY created_at, id",
                crate::tasks::COLUMNS
            ),
            crate::tasks::read_task,
        )?;
        // Bounded statements, well below SQLite's variable limit.
        for chunk in tasks.chunks_mut(500) {
            crate::organization::hydrate_tags(conn, chunk)?;
        }
        let mut history: HashMap<String, Vec<ProgressRecord>> = HashMap::new();
        for (task_id, entry) in collect(
            conn,
            "SELECT task_id, created_at, previous_status, status, previous_progress, progress,
                    note, next_step
             FROM progress_entries ORDER BY sequence",
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ProgressRecord {
                        created_at: row.get(1)?,
                        previous_status: row.get(2)?,
                        status: row.get(3)?,
                        previous_progress: row.get(4)?,
                        progress: row.get(5)?,
                        note: row.get(6)?,
                        next_step: row.get(7)?,
                    },
                ))
            },
        )? {
            history.entry(task_id).or_default().push(entry);
        }
        let tasks = tasks
            .into_iter()
            .map(|task| TaskRecord {
                progress_entries: history.remove(&task.id).unwrap_or_default(),
                id: task.id,
                title: task.title,
                description: task.description,
                status: task.status,
                priority: task.priority,
                progress: i64::from(task.progress),
                next_step: task.next_step,
                list_id: task.list_id,
                deadline: task.due_date,
                tags: task.tags,
                revision: task.revision,
                created_at: task.created_at,
                updated_at: task.updated_at,
                removed: task.removed,
            })
            .collect();
        let note_folders = collect(
            conn,
            "SELECT id, parent_id, name, revision, created_at, updated_at, removed_at
             FROM note_folders ORDER BY created_at, id",
            |row| {
                Ok(FolderRecord {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    removed: row.get::<_, Option<i64>>(6)?.is_some(),
                })
            },
        )?;
        let notes = collect(
            conn,
            &format!(
                "SELECT {} FROM notes WHERE {note_shown} ORDER BY created_at, id",
                crate::notes::COLUMNS
            ),
            crate::notes::read_note,
        )?
        .into_iter()
        .map(|note| NoteRecord {
            id: note.id,
            folder_id: note.folder_id,
            title: note.title,
            markdown: note.markdown,
            pinned: note.pinned,
            exclude_from_ai: note.exclude_from_ai,
            revision: note.revision,
            created_at: note.created_at,
            updated_at: note.updated_at,
            removed: note.removed,
        })
        .collect();
        let note_links = collect(
            conn,
            &format!(
                "SELECT l.id, l.task_id, l.note_id, l.linked_at, l.unlinked_at
                 FROM task_note_links l
                 WHERE EXISTS (SELECT 1 FROM notes WHERE notes.id = l.note_id AND {note_shown})
                 ORDER BY l.linked_at, l.id"
            ),
            |row| {
                Ok(LinkRecord {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    note_id: row.get(2)?,
                    linked_at: row.get(3)?,
                    unlinked_at: row.get(4)?,
                })
            },
        )?;
        Ok(Document {
            format: "tasknotes-export",
            format_version: EXPORT_FORMAT_VERSION,
            schema_version: conn.pragma_query_value(None, "user_version", |r| r.get(0))?,
            exported_at: current_millis()?,
            includes_private_notes: include_private,
            lists,
            tasks,
            note_folders,
            notes,
            note_links,
        })
    }
}

fn collect<T>(
    conn: &Connection,
    sql: &str,
    read: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, TaskError> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map(params_from_iter(std::iter::empty::<i64>()), read)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// `Tasks/<List>.md` (Inbox for Tasks in no List), `Notes/<folder tree>/
/// <Note title>.md` with a front-matter header, and a README.
fn write_markdown(doc: &Document, root: &Path) -> std::io::Result<()> {
    private_dir(root)?;
    write_private(&root.join("README.md"), readme(doc).as_bytes())?;

    let tasks_dir = root.join("Tasks");
    private_dir(&tasks_dir)?;
    let mut names = Names::default();
    let mut groups: Vec<(Option<&str>, String)> = vec![(None, "Inbox".to_owned())];
    groups.extend(
        doc.lists
            .iter()
            .map(|l| (Some(l.id.as_str()), labeled(&l.name, l.removed))),
    );
    for (list_id, heading) in groups {
        let tasks: Vec<&TaskRecord> = doc
            .tasks
            .iter()
            .filter(|t| t.list_id.as_deref() == list_id)
            .collect();
        let mut text = format!("# {heading}\n");
        for task in tasks {
            text.push_str(&task_markdown(task));
        }
        write_private(&names.file(&tasks_dir, &heading), text.as_bytes())?;
    }

    let notes_dir = root.join("Notes");
    private_dir(&notes_dir)?;
    let mut folder_dirs: HashMap<&str, PathBuf> = HashMap::new();
    // Parents before children; a damaged (cyclic) tree is cut at the root.
    let mut pending: Vec<&FolderRecord> = doc.note_folders.iter().collect();
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|folder| {
            let parent = match folder.parent_id.as_deref() {
                None => Some(notes_dir.clone()),
                Some(id) => folder_dirs.get(id).cloned(),
            };
            let Some(parent) = parent else { return true };
            let dir = names.file(&parent, &labeled(&folder.name, folder.removed));
            let dir = dir.with_extension("");
            folder_dirs.insert(&folder.id, dir);
            false
        });
        if pending.len() == before {
            for folder in pending.drain(..) {
                let dir = names.file(&notes_dir, &labeled(&folder.name, folder.removed));
                folder_dirs.insert(&folder.id, dir.with_extension(""));
            }
        }
    }
    // Parents sort before their children.
    let mut dirs: Vec<&PathBuf> = folder_dirs.values().collect();
    dirs.sort();
    for dir in dirs {
        private_dir(dir)?;
    }
    let mut linked: HashMap<&str, Vec<&str>> = HashMap::new();
    for link in doc.note_links.iter().filter(|l| l.unlinked_at.is_none()) {
        linked.entry(&link.note_id).or_default().push(&link.task_id);
    }
    for note in &doc.notes {
        let dir = note
            .folder_id
            .as_deref()
            .and_then(|id| folder_dirs.get(id))
            .unwrap_or(&notes_dir);
        let tasks = linked.get(note.id.as_str()).cloned().unwrap_or_default();
        write_private(
            &names.file(dir, &note.title),
            note_markdown(note, &tasks).as_bytes(),
        )?;
    }
    Ok(())
}

fn readme(doc: &Document) -> String {
    format!(
        "# Tasknotes export\n\n\
         Exported {} (UTC), Tasknotes data schema {}.\n\n\
         - `Tasks/`: one file per List; `Inbox.md` holds Tasks in no List.\n\
         - `Notes/`: Notes as Markdown files in their Note Folder tree.\n\
         - Items marked (Removed) are in Removed in Tasknotes and can still be restored there.\n\
         - Notes marked Exclude from AI: {}.\n",
        iso_time(doc.exported_at),
        doc.schema_version,
        if doc.includes_private_notes {
            "included"
        } else {
            "not included"
        },
    )
}

fn task_markdown(task: &TaskRecord) -> String {
    let mut text = format!("\n## {}\n\n", labeled(&task.title, task.removed));
    text.push_str(&format!(
        "- Status: {} · Priority: {} · Progress: {}%\n",
        status_label(task.status),
        priority_label(task.priority),
        task.progress
    ));
    if let Some(deadline) = &task.deadline {
        text.push_str(&format!("- Deadline: {deadline}\n"));
    }
    if !task.tags.is_empty() {
        text.push_str(&format!("- Tags: {}\n", task.tags.join(", ")));
    }
    if !task.next_step.is_empty() {
        text.push_str(&format!("- Next Step: {}\n", task.next_step));
    }
    text.push_str(&format!(
        "- Created: {} · Updated: {}\n- Id: {}\n",
        iso_time(task.created_at),
        iso_time(task.updated_at),
        task.id
    ));
    if !task.description.is_empty() {
        text.push_str(&format!("\n{}\n", task.description.trim_end()));
    }
    if !task.progress_entries.is_empty() {
        text.push_str("\n### Progress history\n\n");
        for entry in &task.progress_entries {
            text.push_str(&format!(
                "- {}: {} at {}%",
                iso_time(entry.created_at),
                entry.status.replace('_', " "),
                entry.progress
            ));
            if !entry.note.is_empty() {
                text.push_str(&format!(" — {}", entry.note));
            }
            text.push('\n');
        }
    }
    text
}

fn note_markdown(note: &NoteRecord, linked_tasks: &[&str]) -> String {
    // JSON strings are valid YAML scalars, so the header parses either way.
    let quote = |s: &str| serde_json::Value::from(s).to_string();
    let mut text = String::from("---\n");
    text.push_str(&format!("id: {}\n", quote(&note.id)));
    text.push_str(&format!("created: {}\n", quote(&iso_time(note.created_at))));
    text.push_str(&format!("updated: {}\n", quote(&iso_time(note.updated_at))));
    text.push_str(&format!("pinned: {}\n", note.pinned));
    text.push_str(&format!("removed: {}\n", note.removed));
    text.push_str(&format!("exclude_from_ai: {}\n", note.exclude_from_ai));
    let tasks: Vec<String> = linked_tasks.iter().map(|id| quote(id)).collect();
    text.push_str(&format!("linked_tasks: [{}]\n", tasks.join(", ")));
    text.push_str("---\n\n");
    text.push_str(&note.markdown);
    if !note.markdown.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn labeled(name: &str, removed: bool) -> String {
    if removed {
        format!("{name} (Removed)")
    } else {
        name.to_owned()
    }
}

fn status_label(status: WorkStatus) -> &'static str {
    match status {
        WorkStatus::InProgress => "In Progress",
        WorkStatus::Ready => "Ready",
        WorkStatus::Completed => "Completed",
    }
}

fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::VeryLow => "Very Low",
        Priority::Low => "Low",
        Priority::Medium => "Medium",
        Priority::High => "High",
        Priority::VeryHigh => "Very High",
    }
}

/// A file or directory name for an export made at `now_ms`, such as
/// `Tasknotes export 2026-10-26 09.15.json` on the clock in `zone`, so the
/// name matches the time the person sees (no `:` for Finder).
pub fn suggested_name(format: ExportFormat, now_ms: i64, zone: TimeZone) -> String {
    let stamp = iso_time(now_ms + zone.offset_ms(now_ms)).replace(':', ".");
    match format {
        ExportFormat::Json => format!("Tasknotes export {stamp}.json"),
        ExportFormat::Markdown => format!("Tasknotes export {stamp}"),
    }
}

/// `2026-10-26 09:15` for Unix milliseconds, UTC.
fn iso_time(ms: i64) -> String {
    let (year, month, day) =
        crate::calendar::civil_from_days(ms.div_euclid(crate::calendar::DAY_MS));
    let minutes = ms.rem_euclid(86_400_000) / 60_000;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minutes / 60,
        minutes % 60
    )
}

/// File names unique per directory as macOS file systems compare them:
/// ignoring case and Unicode form, so a precomposed "Café" and one with a
/// combining accent are one name.
#[derive(Default)]
struct Names {
    taken: HashMap<PathBuf, std::collections::HashSet<String>>,
}

impl Names {
    /// A safe, unused `<dir>/<name>.md` for a title.
    fn file(&mut self, dir: &Path, title: &str) -> PathBuf {
        let base = safe_name(title);
        let taken = self.taken.entry(dir.to_path_buf()).or_default();
        let mut candidate = base.clone();
        let mut n = 2;
        while !taken.insert(crate::text_fold::fold(&candidate)) {
            candidate = format!("{base} ({n})");
            n += 1;
        }
        dir.join(format!("{candidate}.md"))
    }
}

/// A new file only the person's account can read, written and synced.
/// `create_new`: an existing file is an error, never overwritten.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = crate::backup::private_file(path, true)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// A new directory only the person's account can open.
fn private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

/// Move a finished export into place without replacing anything. A file
/// is hard-linked to its name, which fails if the name is taken, then the
/// partial name is removed; a file system without hard links falls back
/// to a checked rename. A directory is renamed: `rename` refuses a
/// non-empty directory or a file at the destination (an empty directory
/// created there in the meantime is replaced, which loses nothing).
fn publish(partial: &Path, destination: &Path, format: ExportFormat) -> std::io::Result<()> {
    if format == ExportFormat::Json {
        match fs::hard_link(partial, destination) {
            Ok(()) => return fs::remove_file(partial),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Err(error),
            Err(_) => {}
        }
    }
    if destination.symlink_metadata().is_ok() {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    }
    fs::rename(partial, destination)
}

/// A title as a file name: no path separators, reserved or control
/// characters, no leading dot, at most 80 characters.
fn safe_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "Untitled".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_become_safe_unique_file_names() {
        assert_eq!(safe_name("a/b: c?"), "a-b- c-");
        assert_eq!(safe_name("../secret"), "-secret");
        assert_eq!(safe_name("   "), "Untitled");
        assert_eq!(safe_name(".hidden"), "hidden");
        let mut names = Names::default();
        let dir = Path::new("/x");
        assert_eq!(names.file(dir, "Plan"), dir.join("Plan.md"));
        assert_eq!(names.file(dir, "plan"), dir.join("plan (2).md"));
        assert_eq!(names.file(dir, "Plan"), dir.join("Plan (3).md"));
    }

    #[test]
    fn exports_get_a_dated_name() {
        let now = 1_793_006_795_568;
        assert_eq!(
            suggested_name(ExportFormat::Json, now, TimeZone::UTC),
            "Tasknotes export 2026-10-26 09.26.json"
        );
        assert_eq!(
            suggested_name(ExportFormat::Markdown, now, TimeZone::UTC),
            "Tasknotes export 2026-10-26 09.26"
        );
    }

    #[test]
    fn an_export_name_shows_the_time_on_the_persons_clock() {
        let late_evening = 1_793_006_795_568 + 10 * 3_600_000;
        assert_eq!(
            suggested_name(
                ExportFormat::Json,
                late_evening,
                TimeZone::Fixed { minutes: 7 * 60 }
            ),
            "Tasknotes export 2026-10-27 02.26.json"
        );
    }

    #[test]
    fn times_are_written_in_utc() {
        assert_eq!(iso_time(0), "1970-01-01 00:00");
        assert_eq!(iso_time(1_793_006_795_568), "2026-10-26 09:26");
    }
}
