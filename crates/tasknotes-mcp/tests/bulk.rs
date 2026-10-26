//! High-risk bulk changes over MCP: bulk_preview shows the change and hands
//! out a token; only bulk_confirm with that token applies it, all or
//! nothing, and only while nothing previewed has changed.

mod common;

use common::Data;
use serde_json::{Value, json};
use tasknotes_core::notes::CreateNote;
use tasknotes_core::privacy::SetNoteExcludeFromAi;

fn task_ids(s: &mut common::Server, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            s.ok(
                "task_create",
                json!({"title": format!("Synthetic batch {i}")}),
            )["item"]["id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn active(s: &mut common::Server) -> usize {
    s.ok("task_list", json!({}))["items"]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn bulk_tools_are_listed_and_a_preview_changes_nothing() {
    let data = Data::trusted();
    let mut s = data.server();
    let names: Vec<String> = s
        .tools()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"bulk_preview".to_owned()));
    assert!(names.contains(&"bulk_confirm".to_owned()));

    let ids = task_ids(&mut s, 3);
    let preview = s.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": ids}),
    );
    let item = &preview["item"];
    assert_eq!(item["action"], "remove_tasks");
    assert_eq!(item["count"], 3);
    assert!(item["token"].as_str().unwrap().len() >= 32);
    assert!(item["expires_at"].as_i64().unwrap() > 0);
    let changes = item["items"].as_array().unwrap();
    assert_eq!(changes[0]["before"]["removed"], false);
    assert_eq!(changes[0]["after"]["removed"], true);
    assert_eq!(changes[0]["after"]["id"], ids[0]);
    assert_eq!(active(&mut s), 3, "a preview applies nothing");
    let audit = s.ok("audit_list", json!({"limit": 100}));
    assert_eq!(
        audit["items"].as_array().unwrap().len(),
        3,
        "only the creates"
    );
}

#[test]
fn confirming_applies_every_item_once() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 3);
    let token = s.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": ids}),
    )["item"]["token"]
        .clone();
    let done = s.ok("bulk_confirm", json!({"token": token}));
    assert_eq!(done["item"]["action"], "remove_tasks");
    assert_eq!(done["item"]["count"], 3);
    assert_eq!(done["item"]["items"][2]["after"]["removed"], true);
    assert_eq!(active(&mut s), 0);
    assert_eq!(
        s.ok("removed_list", json!({}))["items"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    // A token works once.
    s.refused("bulk_confirm", json!({"token": token}), "stale_preview");
    // Undo is another previewed bulk change.
    let token = s.ok(
        "bulk_preview",
        json!({"action": "restore_tasks", "ids": ids}),
    )["item"]["token"]
        .clone();
    s.ok("bulk_confirm", json!({"token": token}));
    assert_eq!(active(&mut s), 3);
}

#[test]
fn a_desktop_edit_after_the_preview_refuses_the_whole_batch() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 3);
    let token = s.ok(
        "bulk_preview",
        json!({"action": "set_task_status", "status": "completed", "ids": ids}),
    )["item"]["token"]
        .clone();
    data.desktop()
        .patch_task(tasknotes_core::tasks::PatchTask {
            id: ids[1].clone(),
            expected_revision: 1,
            title: Some("Edited by the person".into()),
            description: None,
        })
        .unwrap();
    let error = s.refused("bulk_confirm", json!({"token": token}), "stale_preview");
    assert!(error["message"].as_str().unwrap().contains("Preview again"));
    let tasks = s.ok("task_list", json!({}));
    assert!(
        tasks["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["status"] == "ready")
    );
    // The token is spent even though nothing was applied.
    s.refused("bulk_confirm", json!({"token": token}), "stale_preview");
    // A fresh preview of the current state goes through.
    let token = s.ok(
        "bulk_preview",
        json!({"action": "set_task_status", "status": "completed", "ids": ids}),
    )["item"]["token"]
        .clone();
    s.ok("bulk_confirm", json!({"token": token}));
    let tasks = s.ok("task_list", json!({}));
    assert!(
        tasks["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["status"] == "completed")
    );
}

#[test]
fn moving_tasks_names_one_target() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 2);
    let list = s.ok("list_create", json!({"name": "Synthetic target"}))["item"]["id"].clone();
    let token = s.ok(
        "bulk_preview",
        json!({"action": "move_tasks", "list_id": list, "ids": ids}),
    )["item"]["token"]
        .clone();
    s.ok("bulk_confirm", json!({"token": token}));
    let in_list = s.ok("task_list", json!({"list_id": list}));
    assert_eq!(in_list["items"].as_array().unwrap().len(), 2);
    let token = s.ok(
        "bulk_preview",
        json!({"action": "move_tasks", "inbox": true, "ids": ids}),
    )["item"]["token"]
        .clone();
    s.ok("bulk_confirm", json!({"token": token}));
    assert_eq!(
        s.ok("task_list", json!({"inbox": true}))["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn bad_selections_and_tokens_are_refused_deterministically() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 2);
    for (arguments, code) in [
        (
            json!({"action": "delete_tasks", "ids": ids}),
            "invalid_input",
        ),
        (
            json!({"action": "remove_tasks", "ids": []}),
            "invalid_input",
        ),
        (
            json!({"action": "remove_tasks", "ids": [ids[0], ids[0]]}),
            "invalid_input",
        ),
        (
            json!({"action": "set_task_status", "ids": ids}),
            "invalid_input",
        ),
        (
            json!({"action": "remove_tasks", "status": "completed", "ids": ids}),
            "invalid_input",
        ),
        (json!({"action": "move_tasks", "ids": ids}), "invalid_input"),
        (
            json!({"action": "move_tasks", "inbox": true, "list_id": "x", "ids": ids}),
            "invalid_input",
        ),
        (
            json!({"action": "remove_tasks", "ids": [ids[0], "never-used"]}),
            "not_found",
        ),
    ] {
        s.refused("bulk_preview", arguments, code);
    }
    let error = s.refused(
        "bulk_preview",
        json!({"action": "set_task_status", "status": "ready", "ids": ids}),
        "invalid_input",
    );
    assert!(
        error["message"].as_str().unwrap().starts_with("Item 1: "),
        "{error}"
    );
    s.refused("bulk_confirm", json!({"token": "made-up"}), "stale_preview");
    s.refused("bulk_confirm", json!({}), "invalid_input");
    assert_eq!(active(&mut s), 2);
}

#[test]
fn a_token_belongs_to_the_session_that_previewed() {
    let data = Data::trusted();
    let mut first = data.server();
    let ids = task_ids(&mut first, 1);
    let token = first.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": ids}),
    )["item"]["token"]
        .clone();
    let mut second = data.server();
    second.refused("bulk_confirm", json!({"token": token}), "stale_preview");
    assert_eq!(active(&mut second), 1);
    first.ok("bulk_confirm", json!({"token": token}));
}

#[test]
fn bulk_notes_treat_an_excluded_note_as_missing() {
    let data = Data::trusted();
    let desktop = data.desktop();
    let note = |markdown: &str| {
        desktop
            .create_note(CreateNote {
                markdown: markdown.into(),
                folder_id: None,
            })
            .unwrap()
    };
    let shared = note("# Shared synthetic note");
    let private = note("# Private synthetic secret-marker-9921");
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let mut s = data.server();
    let never = "00000000-0000-4000-8000-00000000beef";
    let refused = |s: &mut common::Server, id: &str| -> String {
        let error = s.refused(
            "bulk_preview",
            json!({"action": "remove_notes", "ids": [shared.id, id]}),
            "not_found",
        );
        error.to_string().replace(id, "<ID>")
    };
    assert_eq!(refused(&mut s, &private.id), refused(&mut s, never));

    let preview = s.ok(
        "bulk_preview",
        json!({"action": "remove_notes", "ids": [shared.id]}),
    );
    let change: &Value = &preview["item"]["items"][0];
    assert_eq!(change["before"]["title"], "Shared synthetic note");
    assert!(
        change["after"].get("markdown").is_none(),
        "previews stay small"
    );
    // Excluded after the preview: the generic stale refusal, nothing removed.
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: shared.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let error = s.refused(
        "bulk_confirm",
        json!({"token": preview["item"]["token"]}),
        "stale_preview",
    );
    assert!(!error.to_string().contains(&shared.id));
    assert!(!desktop.get_note(&shared.id).unwrap().unwrap().removed);
    assert!(s.stderr().is_empty());
}
