//! Ticket 13: JSON and Markdown export. A human export from the desktop may
//! include Notes marked Exclude from AI when the person asks; an AI-facing
//! export never does. Removed items are exported and labeled. Synthetic data.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tasknotes_core::audit::Interface;
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::note_folders::{CreateNoteFolder, NoteFolder};
use tasknotes_core::notes::{CreateNote, Note};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{CreateList, CreateTask, RecordProgress, TaskStore};

struct Fixture {
    dir: tempfile::TempDir,
    folders: Vec<NoteFolder>,
    shared: Note,
    private: Note,
    removed_note: Note,
}

impl Fixture {
    /// Work › Projects › Archive, a Removed folder, Notes in the tree, a
    /// private Note linked to a Task, a Removed Task and a List.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = TaskStore::open(&dir.path().join("tasknotes.db")).unwrap();
        let folder = |name: &str, parent: Option<&NoteFolder>| {
            store
                .create_note_folder(CreateNoteFolder {
                    name: name.into(),
                    parent_id: parent.map(|p| p.id.clone()),
                })
                .unwrap()
        };
        let work = folder("Work", None);
        let projects = folder("Projects", Some(&work));
        let archive = folder("Archive", Some(&projects));
        let old = folder("Old: drafts/misc", None);
        let removed_folder = store
            .set_note_folder_removed(SetRemoved {
                id: old.id.clone(),
                expected_revision: old.revision,
                removed: true,
            })
            .unwrap();
        let note = |markdown: &str, folder: Option<&NoteFolder>| {
            store
                .create_note(CreateNote {
                    markdown: markdown.into(),
                    folder_id: folder.map(|f| f.id.clone()),
                })
                .unwrap()
        };
        let shared = note("# Synthetic roadmap\n\nShared text", Some(&archive));
        let private = note("# Synthetic diary\n\nPrivate text", Some(&projects));
        store
            .set_note_exclude_from_ai(SetNoteExcludeFromAi {
                id: private.id.clone(),
                exclude_from_ai: true,
            })
            .unwrap();
        let removed_note = note("# Synthetic old idea", None);
        store
            .set_note_removed(SetRemoved {
                id: removed_note.id.clone(),
                expected_revision: removed_note.revision,
                removed: true,
            })
            .unwrap();

        let list = store
            .create_list(CreateList {
                name: "Synthetic Errands".into(),
            })
            .unwrap();
        let task = store
            .create_task_in_list(
                CreateTask {
                    title: "Synthetic buy stamps".into(),
                    description: "Synthetic description".into(),
                },
                Some(&list.id),
            )
            .unwrap();
        store
            .record_progress(RecordProgress {
                id: task.id.clone(),
                expected_revision: task.revision,
                status: None,
                progress: Some(40),
                next_step: Some("Synthetic next step".into()),
                note: Some("Synthetic progress note".into()),
            })
            .unwrap();
        store.link_note(&task.id, &private.id).unwrap();
        store.link_note(&task.id, &shared.id).unwrap();
        let gone = store
            .create_task(CreateTask {
                title: "Synthetic removed task".into(),
                description: String::new(),
            })
            .unwrap();
        store
            .set_task_removed(SetRemoved {
                id: gone.id.clone(),
                expected_revision: gone.revision,
                removed: true,
            })
            .unwrap();
        Self {
            dir,
            folders: vec![work, projects, archive, removed_folder],
            shared,
            private,
            removed_note,
        }
    }

    fn store(&self, interface: Interface) -> TaskStore {
        TaskStore::open_as(&self.dir.path().join("tasknotes.db"), interface).unwrap()
    }

    fn out(&self, name: &str) -> PathBuf {
        let exports = self.dir.path().join("exports");
        std::fs::create_dir_all(&exports).unwrap();
        exports.join(name)
    }
}

fn request(format: ExportFormat, destination: &Path, include_private: bool) -> ExportRequest {
    ExportRequest {
        format,
        destination: destination.to_path_buf(),
        include_private_notes: include_private,
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn ids(items: &Value) -> Vec<String> {
    let mut ids: Vec<String> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_json_export_round_trips_the_folder_tree_and_keeps_removed_items() {
    let fx = Fixture::new();
    let desktop = fx.store(Interface::Desktop);
    let path = fx.out("tasknotes.json");
    let summary = desktop
        .export_data(request(ExportFormat::Json, &path, false))
        .unwrap();
    assert_eq!(summary.private_notes, 0);

    let doc = read_json(&path);
    assert_eq!(doc["format"], "tasknotes-export");
    assert_eq!(doc["format_version"], 1);
    assert_eq!(doc["includes_private_notes"], false);

    // Each folder keeps its parent, so the tree reads back as it was.
    let folders = doc["note_folders"].as_array().unwrap();
    let parent_of = |id: &str| {
        folders.iter().find(|f| f["id"] == id).unwrap()["parent_id"]
            .as_str()
            .map(str::to_owned)
    };
    let [work, projects, archive, old] = &fx.folders[..] else {
        unreachable!()
    };
    assert_eq!(parent_of(&work.id), None);
    assert_eq!(parent_of(&projects.id), Some(work.id.clone()));
    assert_eq!(parent_of(&archive.id), Some(projects.id.clone()));
    let removed_folder = folders.iter().find(|f| f["id"] == old.id.as_str()).unwrap();
    assert_eq!(removed_folder["removed"], true);

    // Removed items are exported and say so.
    let tasks = doc["tasks"].as_array().unwrap();
    let removed_task = tasks
        .iter()
        .find(|t| t["title"] == "Synthetic removed task")
        .unwrap();
    assert_eq!(removed_task["removed"], true);
    let notes = doc["notes"].as_array().unwrap();
    let old_idea = notes
        .iter()
        .find(|n| n["id"] == fx.removed_note.id.as_str())
        .unwrap();
    assert_eq!(old_idea["removed"], true);

    // Private Notes, and links to them, stay out unless asked for.
    assert_eq!(ids(&doc["notes"]), {
        let mut v = vec![fx.shared.id.clone(), fx.removed_note.id.clone()];
        v.sort();
        v
    });
    let links = doc["note_links"].as_array().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["note_id"], fx.shared.id.as_str());

    // Tasks carry their List, Tags, progress and history.
    let stamps = tasks
        .iter()
        .find(|t| t["title"] == "Synthetic buy stamps")
        .unwrap();
    assert_eq!(stamps["progress"], 40);
    assert_eq!(stamps["next_step"], "Synthetic next step");
    let history = stamps["progress_entries"].as_array().unwrap();
    assert_eq!(history[0]["note"], "Synthetic progress note");
    let list = &doc["lists"][0];
    assert_eq!(stamps["list_id"], list["id"]);
    assert_eq!(summary.tasks, 2);
    assert_eq!(summary.note_folders, 4);
}

#[test]
fn the_person_can_include_private_notes_in_a_desktop_export() {
    let fx = Fixture::new();
    let desktop = fx.store(Interface::Desktop);
    let path = fx.out("with-private.json");
    let summary = desktop
        .export_data(request(ExportFormat::Json, &path, true))
        .unwrap();
    assert_eq!(summary.private_notes, 1);
    let doc = read_json(&path);
    assert_eq!(doc["includes_private_notes"], true);
    let private = doc["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == fx.private.id.as_str())
        .unwrap();
    assert_eq!(private["exclude_from_ai"], true);
    assert_eq!(private["markdown"], fx.private.markdown.as_str());
    assert_eq!(doc["note_links"].as_array().unwrap().len(), 2);
}

#[test]
fn an_ai_facing_export_never_contains_a_private_note() {
    let fx = Fixture::new();
    for interface in [Interface::Cli, Interface::Mcp] {
        let ai = fx.store(interface);
        let refused = ai
            .export_data(request(
                ExportFormat::Json,
                &fx.out(&format!("{interface:?}-private.json")),
                true,
            ))
            .unwrap_err();
        assert_eq!(refused.code(), "desktop_only");

        for format in [ExportFormat::Json, ExportFormat::Markdown] {
            let path = fx.out(&format!("{interface:?}-{format:?}"));
            let summary = ai.export_data(request(format, &path, false)).unwrap();
            assert_eq!(summary.private_notes, 0);
            let everything = all_text(&path);
            assert!(!everything.contains("Synthetic diary"));
            assert!(!everything.contains("Private text"));
            assert!(!everything.contains(&fx.private.id));
            assert!(everything.contains("Synthetic roadmap"));
        }
    }
}

/// Every byte of a file, or of every file under a directory.
fn all_text(path: &Path) -> String {
    if path.is_file() {
        return std::fs::read_to_string(path).unwrap();
    }
    let mut text = String::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap().path();
        text.push_str(&entry.to_string_lossy());
        text.push('\n');
        text.push_str(&all_text(&entry));
    }
    text
}

#[test]
fn a_markdown_export_keeps_the_folder_tree_as_directories() {
    let fx = Fixture::new();
    let desktop = fx.store(Interface::Desktop);
    let root = fx.out("Tasknotes export");
    desktop
        .export_data(request(ExportFormat::Markdown, &root, true))
        .unwrap();

    let roadmap = root.join("Notes/Work/Projects/Archive/Synthetic roadmap.md");
    let text = std::fs::read_to_string(&roadmap).unwrap();
    assert!(text.starts_with("---\n"));
    assert!(text.contains(&format!("id: \"{}\"", fx.shared.id)));
    assert!(text.ends_with("# Synthetic roadmap\n\nShared text\n"));

    let diary =
        std::fs::read_to_string(root.join("Notes/Work/Projects/Synthetic diary.md")).unwrap();
    assert!(diary.contains("exclude_from_ai: true"));

    // Removed items are there and labeled; unsafe characters are replaced.
    assert!(root.join("Notes/Old- drafts-misc (Removed)").is_dir());
    let old_idea = std::fs::read_to_string(root.join("Notes/Synthetic old idea.md")).unwrap();
    assert!(old_idea.contains("removed: true"));

    let errands = std::fs::read_to_string(root.join("Tasks/Synthetic Errands.md")).unwrap();
    assert!(errands.contains("## Synthetic buy stamps"));
    assert!(errands.contains("Progress: 40%"));
    assert!(errands.contains("Synthetic progress note"));
    let inbox = std::fs::read_to_string(root.join("Tasks/Inbox.md")).unwrap();
    assert!(inbox.contains("## Synthetic removed task (Removed)"));
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.contains("Exclude from AI: included"));
}

#[test]
fn an_export_never_overwrites_and_leaves_nothing_half_written() {
    let fx = Fixture::new();
    let desktop = fx.store(Interface::Desktop);
    let path = fx.out("taken.json");
    std::fs::write(&path, "synthetic existing file").unwrap();
    for format in [ExportFormat::Json, ExportFormat::Markdown] {
        let refused = desktop
            .export_data(request(format, &path, false))
            .unwrap_err();
        assert_eq!(refused.code(), "invalid_input");
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "synthetic existing file"
    );
    let left: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, ["taken.json"]);
}

#[test]
fn an_empty_database_exports_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasknotes.db")).unwrap();
    let json = dir.path().join("empty.json");
    let summary = store
        .export_data(request(ExportFormat::Json, &json, false))
        .unwrap();
    assert_eq!((summary.tasks, summary.notes), (0, 0));
    assert_eq!(read_json(&json)["tasks"], serde_json::json!([]));
    let md = dir.path().join("empty-md");
    store
        .export_data(request(ExportFormat::Markdown, &md, false))
        .unwrap();
    assert!(md.join("README.md").is_file());
}

/// Review finding: "Café" typed precomposed (NFC) and decomposed (NFD) are
/// one name on macOS file systems, so one Note's file silently replaced
/// the other's. Names are compared after normalisation and case folding,
/// and every file is created fresh (never overwritten).
#[test]
fn notes_whose_titles_differ_only_in_unicode_form_get_two_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasknotes.db")).unwrap();
    for markdown in [
        "# Caf\u{e9}\n\nSynthetic A",
        "# Cafe\u{301}\n\nSynthetic B",
        "# CAF\u{c9}\n\nSynthetic C",
    ] {
        store
            .create_note(CreateNote {
                markdown: markdown.into(),
                folder_id: None,
            })
            .unwrap();
    }
    let out = dir.path().join("out");
    let summary = store
        .export_data(request(ExportFormat::Markdown, &out, false))
        .unwrap();
    assert_eq!(summary.notes, 3);
    let bodies: Vec<String> = std::fs::read_dir(out.join("Notes"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert_eq!(bodies.len(), 3);
    for letter in ["A", "B", "C"] {
        let text = format!("Synthetic {letter}");
        assert_eq!(
            bodies.iter().filter(|b| b.contains(&text)).count(),
            1,
            "{letter}"
        );
    }
}

/// Exports hold the person's data: private to their account, like backups.
#[cfg(unix)]
#[test]
fn exports_are_private_to_the_persons_account() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    let desktop = fx.store(Interface::Desktop);
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    let json = fx.out("private.json");
    desktop
        .export_data(request(ExportFormat::Json, &json, false))
        .unwrap();
    assert_eq!(mode(&json), 0o600);
    let md = fx.out("private-md");
    desktop
        .export_data(request(ExportFormat::Markdown, &md, false))
        .unwrap();
    let mut checked = 0;
    let mut pending = vec![md.clone()];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            assert_eq!(mode(&path), 0o700, "{}", path.display());
            pending.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
        } else {
            assert_eq!(mode(&path), 0o600, "{}", path.display());
            checked += 1;
        }
    }
    assert!(checked >= 3);
}

/// Review finding: a destination with no name was reported as "already
/// exists".
#[test]
fn a_destination_without_a_name_is_its_own_error() {
    let fx = Fixture::new();
    let desktop = fx.store(Interface::Desktop);
    let refused = desktop
        .export_data(request(ExportFormat::Json, Path::new("/"), false))
        .unwrap_err();
    assert_eq!(refused.code(), "invalid_input");
    assert!(
        refused.to_string().contains("needs a file or folder name"),
        "{refused}"
    );
}
