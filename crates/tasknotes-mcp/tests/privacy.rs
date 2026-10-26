//! Exclude from AI negative controls through the real MCP server: an
//! excluded Note is indistinguishable from an id that was never used, in
//! every tool's success and error output alike, and nothing about it reaches
//! stdout or stderr. The desktop side is the core store opened directly.
//! Synthetic data only.

mod common;

use common::{Data, Server};
use serde_json::{Value, json};
use tasknotes_core::links::NoteLink;
use tasknotes_core::note_folders::{CreateNoteFolder, NoteFolder};
use tasknotes_core::notes::{CreateNote, Note};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{CreateTask, Task};

const SECRET: &str = "secret-marker-5512";
const PRIVATE_TITLE: &str = "Private synthetic journal";
const NEVER: &str = "00000000-0000-4000-8000-00000000beef";
const NEVER_LINK: &str = "00000000-0000-4000-8000-00000000cafe";

pub struct World {
    data: Data,
    server: Server,
    task: Task,
    vault: NoteFolder,
    shared: Note,
    private: Note,
    removed_private: Note,
    private_link: NoteLink,
}

impl World {
    fn leaks(&self) -> [&str; 5] {
        [
            SECRET,
            PRIVATE_TITLE,
            self.private.id.as_str(),
            self.removed_private.id.as_str(),
            self.private_link.id.as_str(),
        ]
    }

    /// Call, and prove nothing about the private Notes came back.
    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let envelope = self.server.call(tool, arguments.clone());
        let out = envelope.to_string();
        for leak in self.leaks() {
            assert!(
                !out.contains(leak),
                "{tool} {arguments} leaked {leak:?}: {out}"
            );
        }
        envelope
    }

    fn assert_quiet(&self) {
        let stderr = self.server.stderr();
        assert!(stderr.is_empty(), "stderr: {stderr}");
    }
}

fn world() -> World {
    let data = Data::trusted();
    let desktop = data.desktop();
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
    let put = |markdown: &str, folder: Option<&str>| {
        desktop
            .create_note(CreateNote {
                markdown: markdown.into(),
                folder_id: folder.map(str::to_owned),
            })
            .unwrap()
    };
    let shared = put("# Shared synthetic plan\n\nGarden layout", None);
    let private = put(
        &format!("# {PRIVATE_TITLE}\n\n{SECRET} garden layout"),
        Some(&vault.id),
    );
    let removed_private = put(&format!("# Removed journal\n\n{SECRET} two"), None);
    desktop.link_note(&task.id, &shared.id).unwrap();
    let private_link = desktop.link_note(&task.id, &private.id).unwrap();
    let removed_private = desktop
        .set_note_removed(SetRemoved {
            id: removed_private.id,
            expected_revision: 1,
            removed: true,
        })
        .unwrap();
    for note in [&private, &removed_private] {
        desktop
            .set_note_exclude_from_ai(SetNoteExcludeFromAi {
                id: note.id.clone(),
                exclude_from_ai: true,
            })
            .unwrap();
    }
    let server = data.server();
    World {
        data,
        server,
        task,
        vault,
        shared,
        private,
        removed_private,
        private_link,
    }
}

/// A tool's arguments naming the given id.
type Arguments = Box<dyn Fn(&str) -> Value>;

/// The envelope with `id` written as `<ID>`, so two answers about different
/// ids can be compared byte for byte.
fn masked(envelope: &Value, id: &str) -> String {
    envelope.to_string().replace(id, "<ID>")
}

#[test]
fn every_tool_naming_an_excluded_note_answers_as_for_an_unused_id() {
    let mut w = world();
    let task = w.task.id.clone();
    let calls: Vec<(&str, Arguments)> = vec![
        ("note_get", Box::new(|id| json!({"id": id}))),
        (
            "note_update",
            Box::new(|id| json!({"id": id, "revision": 1, "markdown": "# AI text"})),
        ),
        (
            "note_move",
            Box::new(|id| json!({"id": id, "revision": 1, "no_folder": true})),
        ),
        (
            "note_remove",
            Box::new(|id| json!({"id": id, "revision": 1})),
        ),
        (
            "note_restore",
            Box::new(|id| json!({"id": id, "revision": 2})),
        ),
        ("note_tasks", Box::new(|id| json!({"id": id}))),
        ("audit_list", Box::new(|id| json!({"entity_id": id}))),
        (
            "link_create",
            Box::new(move |id| json!({"task_id": task, "note_id": id})),
        ),
    ];
    for note in [w.private.id.clone(), w.removed_private.id.clone()] {
        for (tool, arguments) in &calls {
            let private = w.call(tool, arguments(&note));
            let never = w.call(tool, arguments(NEVER));
            assert_eq!(masked(&private, &note), masked(&never, NEVER), "{tool}");
        }
    }
    let link = w.private_link.id.clone();
    for tool in ["link_get", "link_remove", "link_restore", "audit_list"] {
        let key = if tool == "audit_list" {
            "entity_id"
        } else {
            "id"
        };
        let private = w.call(tool, json!({ key: link }));
        let never = w.call(tool, json!({ key: NEVER_LINK }));
        assert_eq!(
            masked(&private, &link),
            masked(&never, NEVER_LINK),
            "{tool}"
        );
    }
    // Refused attempts changed nothing the person can see.
    let desktop = w.data.desktop();
    let note = desktop.get_note(&w.private.id).unwrap().unwrap();
    assert!(note.markdown.contains(SECRET));
    assert_eq!(note.revision, w.private.revision);
    assert_eq!(
        desktop
            .get_link(&w.private_link.id)
            .unwrap()
            .unwrap()
            .unlinked_at,
        None
    );
    w.assert_quiet();
}

#[test]
fn listings_searches_counts_and_links_leave_excluded_notes_out() {
    let mut w = world();
    let task = w.task.id.clone();
    let vault = w.vault.id.clone();
    let notes = w.call("note_list", json!({}));
    assert_eq!(notes["items"].as_array().unwrap().len(), 1);
    assert_eq!(notes["items"][0]["id"], w.shared.id);
    let hits = w.call("note_list", json!({"text": "garden"}));
    assert_eq!(hits["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        w.call("note_list", json!({"text": SECRET}))["items"],
        json!([])
    );
    assert_eq!(
        w.call("note_list", json!({"folder_id": vault}))["items"],
        json!([])
    );
    assert_eq!(
        w.call("folder_get", json!({"id": vault}))["item"]["note_count"],
        0
    );
    assert_eq!(
        w.call("folder_list", json!({}))["items"][0]["note_count"],
        0
    );
    let linked = w.call("task_notes", json!({"id": task}));
    assert_eq!(linked["items"].as_array().unwrap().len(), 1);
    assert_eq!(linked["items"][0]["note_id"], w.shared.id);
    assert_eq!(w.call("removed_list", json!({}))["items"], json!([]));
    assert_eq!(
        w.call("removed_list", json!({"text": SECRET}))["items"],
        json!([])
    );
    let audit = w.call("audit_list", json!({"limit": 100}));
    assert!(!audit["items"].as_array().unwrap().is_empty());
    w.call("task_list", json!({"text": SECRET}));
    w.call("task_get", json!({"id": task}));
    w.assert_quiet();
}

#[test]
fn an_ai_cannot_change_the_exclude_flag_or_see_it() {
    let mut w = world();
    let shared = w.call("note_get", json!({"id": w.shared.id}));
    assert!(shared["item"].get("exclude_from_ai").is_none());
    let names: Vec<String> = w
        .server
        .tools()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        names
            .iter()
            .all(|n| !n.contains("privacy") && !n.contains("exclude")),
        "{names:?}"
    );
    // An unknown argument is refused, not silently applied.
    let error = w.server.refused(
        "note_update",
        json!({"id": w.shared.id, "revision": 1, "markdown": "# x", "exclude_from_ai": false}),
        "invalid_input",
    );
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("exclude_from_ai")
    );
}
