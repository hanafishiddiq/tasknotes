//! Exclude from AI negative controls (Ticket 11) through the real
//! `tasknotes` process: an excluded Note must be indistinguishable from one
//! that never existed, on success and failure output alike. The desktop side
//! is the core store opened directly, as the Tauri window opens it.
//! Synthetic data only.

mod common;

use common::{Cli, Run};
use serde_json::Value;
use tasknotes_core::links::NoteLink;
use tasknotes_core::note_folders::{CreateNoteFolder, NoteFolder};
use tasknotes_core::notes::{CreateNote, Note, UpdateNote};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{CreateTask, Task, TaskStore};

const SECRET: &str = "secret-marker-7731";
const PRIVATE_TITLE: &str = "Private synthetic diary";
const NEVER: &str = "00000000-0000-4000-8000-00000000beef";
const NEVER_LINK: &str = "00000000-0000-4000-8000-00000000cafe";

struct World {
    cli: Cli,
    task: Task,
    vault: NoteFolder,
    shared: Note,
    private: Note,
    private_link: NoteLink,
}

impl World {
    fn desktop(&self) -> TaskStore {
        TaskStore::open(&self.cli.db()).unwrap()
    }

    /// Run, and prove nothing about the private Note reached stdout or stderr.
    fn run(&self, args: &[&str]) -> Run {
        let run = self.cli.run(args);
        let out = run.json.to_string();
        for leak in [
            SECRET,
            PRIVATE_TITLE,
            self.private.id.as_str(),
            self.private_link.id.as_str(),
        ] {
            assert!(!out.contains(leak), "{args:?} leaked {leak:?}: {out}");
            assert!(!run.stderr.contains(leak), "{args:?} leaked on stderr");
        }
        assert!(
            run.stderr.is_empty(),
            "{args:?} wrote to stderr: {}",
            run.stderr
        );
        run
    }

    fn ok(&self, args: &[&str]) -> Value {
        let run = self.run(args);
        assert_eq!(run.code, 0, "{args:?}: {}", run.json);
        run.json
    }
}

fn world() -> World {
    let cli = Cli::trusted();
    let desktop = TaskStore::open(&cli.db()).unwrap();
    let task = desktop
        .create_task(CreateTask {
            title: "Synthetic weekly review".into(),
            description: String::new(),
        })
        .unwrap();
    let vault = desktop
        .create_note_folder(CreateNoteFolder {
            name: "Vault".into(),
            parent_id: None,
        })
        .unwrap();
    let put = |markdown: &str| {
        desktop
            .create_note(CreateNote {
                markdown: markdown.into(),
                folder_id: Some(vault.id.clone()),
            })
            .unwrap()
    };
    let shared = put("# Shared synthetic diary\n\nordinary words");
    let private = put(&format!("# {PRIVATE_TITLE}\n\n{SECRET} ordinary words"));
    desktop.link_note(&task.id, &shared.id).unwrap();
    let private_link = desktop.link_note(&task.id, &private.id).unwrap();
    let private = desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    // The person's own stale save leaves a rejected Audit Entry about it.
    let stale = desktop.update_note(UpdateNote {
        id: private.id.clone(),
        markdown: "# stale".into(),
        expected_revision: private.revision + 5,
    });
    assert_eq!(stale.unwrap_err().code(), "revision_conflict");
    World {
        cli,
        task,
        vault,
        shared,
        private,
        private_link,
    }
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            item["id"]
                .as_str()
                .or(item["note_id"].as_str())
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn direct_get_of_an_excluded_note_reads_exactly_like_a_missing_one() {
    let w = world();
    let hidden = w.run(&["note", "get", &w.private.id]);
    let missing = w.run(&["note", "get", NEVER]);
    assert_eq!(hidden.code, 3);
    assert_eq!(hidden.json, missing.json);
    assert_eq!(hidden.json["error"]["code"], "not_found");
    assert_eq!(
        w.ok(&["note", "get", &w.shared.id])["item"]["id"],
        *w.shared.id
    );
}

#[test]
fn listings_searches_and_folder_counts_leave_an_excluded_note_out() {
    let w = world();
    let shared = vec![w.shared.id.clone()];
    assert_eq!(ids(&w.ok(&["note", "list"])), shared);
    assert_eq!(
        ids(&w.ok(&["note", "list", "--folder", &w.vault.id])),
        shared
    );
    assert_eq!(ids(&w.ok(&["note", "list", "--text", "ordinary"])), shared);
    assert_eq!(ids(&w.ok(&["note", "list", "--text", "diary"])), shared);
    for text in [SECRET, "private"] {
        assert!(ids(&w.ok(&["note", "list", "--text", text])).is_empty());
    }
    let folders = w.ok(&["folder", "list"]);
    assert_eq!(
        folders["items"][0]["note_count"], 1,
        "AI-visible Notes only"
    );
    assert_eq!(
        w.ok(&["folder", "get", &w.vault.id])["item"]["note_count"],
        1
    );
    // The person still sees both.
    assert_eq!(
        w.desktop().count_folder_notes(Some(&w.vault.id)).unwrap(),
        2
    );
}

#[test]
fn link_metadata_never_mentions_an_excluded_note() {
    let w = world();
    let linked = w.ok(&["task", "notes", &w.task.id]);
    assert_eq!(ids(&linked), vec![w.shared.id.clone()]);
    for (hidden, missing) in [
        (
            vec!["note", "tasks", w.private.id.as_str()],
            vec!["note", "tasks", NEVER],
        ),
        (
            vec!["link", "get", w.private_link.id.as_str()],
            vec!["link", "get", NEVER_LINK],
        ),
    ] {
        let hidden = w.run(&hidden);
        assert_eq!(hidden.code, 3);
        assert_eq!(hidden.json, w.run(&missing).json);
    }
    // The Task itself is unchanged and still readable.
    assert_eq!(w.ok(&["task", "get", &w.task.id])["item"]["revision"], 1);
}

#[test]
fn writes_restores_and_link_changes_on_an_excluded_note_fail_like_a_missing_note() {
    let w = world();
    let before = w.desktop().get_note(&w.private.id).unwrap().unwrap();
    let rev = before.revision.to_string();
    let attempts = |note: &str, link: &str| -> Vec<Vec<String>> {
        [
            vec![
                "note",
                "update",
                note,
                "--revision",
                &rev,
                "--markdown",
                "# AI text",
            ],
            vec![
                "note",
                "update",
                note,
                "--revision",
                "99",
                "--markdown",
                "# AI text",
            ],
            vec!["note", "move", note, "--revision", &rev, "--no-folder"],
            vec!["note", "remove", note, "--revision", &rev],
            vec!["note", "restore", note, "--revision", &rev],
            vec!["link", "create", "--task", &w.task.id, "--note", note],
            vec!["link", "remove", link],
            vec!["link", "restore", link],
        ]
        .iter()
        .map(|a| a.iter().map(|s| s.to_string()).collect())
        .collect()
    };
    let hidden = attempts(&w.private.id, &w.private_link.id);
    let missing = attempts(NEVER, NEVER_LINK);
    for (hidden, missing) in hidden.iter().zip(&missing) {
        let hidden_args: Vec<&str> = hidden.iter().map(String::as_str).collect();
        let missing_args: Vec<&str> = missing.iter().map(String::as_str).collect();
        let refused = w.run(&hidden_args);
        assert_eq!(refused.code, 3, "{hidden_args:?}: {}", refused.json);
        assert_eq!(refused.json, w.run(&missing_args).json, "{hidden_args:?}");
    }

    // Nothing changed for the person, and their link is still there.
    let desktop = w.desktop();
    assert_eq!(desktop.get_note(&w.private.id).unwrap().unwrap(), before);
    let link = desktop.get_link(&w.private_link.id).unwrap().unwrap();
    assert!(link.unlinked_at.is_none());

    // Removed by the person: restoring through the CLI is still "not found",
    // and the Removed view does not list it.
    let removed = desktop
        .set_note_removed(SetRemoved {
            id: w.private.id.clone(),
            expected_revision: before.revision,
            removed: true,
        })
        .unwrap();
    let restore = w.run(&[
        "note",
        "restore",
        &w.private.id,
        "--revision",
        &removed.revision.to_string(),
    ]);
    assert_eq!(restore.code, 3);
    for text in ["", "diary", SECRET] {
        assert!(ids(&w.ok(&["removed", "list", "--text", text])).is_empty());
    }
    assert!(desktop.get_note(&w.private.id).unwrap().unwrap().removed);
}

#[test]
fn audit_history_shown_to_ai_has_no_entry_about_an_excluded_note() {
    let w = world();
    // Failure-side activity first: refused AI attempts must not add entries.
    w.run(&[
        "note",
        "update",
        &w.private.id,
        "--revision",
        "1",
        "--markdown",
        "# x",
    ]);
    w.run(&["link", "remove", &w.private_link.id]);
    let all = w.ok(&["audit", "list", "--limit", "100"]);
    let seen: Vec<String> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["entity_id"].as_str().unwrap().to_owned())
        .collect();
    assert!(seen.contains(&w.shared.id) && seen.contains(&w.task.id));
    for entity in [w.private.id.as_str(), w.private_link.id.as_str()] {
        let only = w.ok(&["audit", "list", "--entity", entity]);
        assert!(only["items"].as_array().unwrap().is_empty());
        assert_eq!(only, w.ok(&["audit", "list", "--entity", NEVER]));
    }
    // The person's history still has them, and no AI attempt was recorded.
    let desktop = w.desktop();
    let mine = desktop
        .list_audit_entries(Some(&w.private.id), 100, None)
        .unwrap()
        .items;
    let actions: Vec<_> = mine.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(actions, ["note.update", "note.privacy", "note.create"]);
    assert!(mine.iter().all(|e| e.interface.as_str() == "desktop"));
}

#[test]
fn sharing_the_note_again_on_the_desktop_makes_it_visible_to_ai_at_once() {
    let w = world();
    w.desktop()
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: w.private.id.clone(),
            exclude_from_ai: false,
        })
        .unwrap();
    let got = w.cli.ok(&["note", "get", &w.private.id]);
    assert_eq!(got["item"]["title"], PRIVATE_TITLE);
    assert_eq!(got["item"]["folder_path"], "Vault");
    let folders = w.cli.ok(&["folder", "get", &w.vault.id]);
    assert_eq!(folders["item"]["note_count"], 2);
}
