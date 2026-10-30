//! Bulk Changes over MCP (owner decisions A and B, 2026-10-26): bulk_preview
//! only *asks*; the person approves or declines in the desktop app, and the
//! AI reads the outcome with bulk_status. No tool approves. Synthetic data.

mod common;

use common::Data;
use serde_json::{Value, json};
use tasknotes_core::note_folders::CreateNoteFolder;
use tasknotes_core::notes::CreateNote;
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::PatchTask;

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

fn request_id(envelope: &Value) -> String {
    envelope["item"]["request_id"].as_str().unwrap().to_owned()
}

#[test]
fn a_preview_is_a_pending_request_that_no_tool_can_approve() {
    let data = Data::trusted();
    let mut s = data.server();
    let names: Vec<String> = s
        .tools()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"bulk_preview".to_owned()));
    assert!(names.contains(&"bulk_status".to_owned()));
    assert!(
        !names
            .iter()
            .any(|n| n.contains("confirm") || n.contains("approve")),
        "{names:?}"
    );

    let ids = task_ids(&mut s, 3);
    let preview = s.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": ids}),
    );
    let item = &preview["item"];
    assert_eq!(item["action"], "remove_tasks");
    assert_eq!(item["status"], "pending");
    assert_eq!(item["count"], 3);
    assert_eq!(item["requested_by"], "mcp");
    assert!(item.get("token").is_none());
    let changes = item["items"].as_array().unwrap();
    assert_eq!(changes[0]["before"]["removed"], false);
    assert_eq!(changes[0]["after"]["removed"], true);
    assert_eq!(active(&mut s), 3, "a request applies nothing");
    // The old confirm step is gone; the request still waits.
    let gone = s.request(
        "tools/call",
        json!({"name": "bulk_confirm", "arguments": {"token": "x"}}),
    );
    assert_eq!(gone["error"]["code"], -32602);
    let id = request_id(&preview);
    assert_eq!(
        s.ok("bulk_status", json!({"id": id}))["item"]["status"],
        "pending"
    );
    assert_eq!(data.desktop().count_pending_bulk_requests().unwrap(), 1);
}

#[test]
fn the_person_approves_in_the_desktop_and_the_ai_reads_the_outcome() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 3);
    let id = request_id(&s.ok(
        "bulk_preview",
        json!({"action": "set_task_status", "ids": ids, "status": "completed"}),
    ));
    // A request survives the server: a new session reads it.
    drop(s);
    let mut s = data.server();
    data.desktop().approve_bulk_request(&id).unwrap();
    let status = s.ok("bulk_status", json!({"id": id}))["item"].clone();
    assert_eq!(status["status"], "applied");
    let done = s.ok("task_get", json!({"id": ids[0]}));
    assert_eq!(done["item"]["status"], "completed");
    // Audit: the items under the desktop, asked for by MCP.
    let audit = s.ok("audit_list", json!({"entity_id": ids[0]}));
    let change = &audit["items"][0];
    assert_eq!(change["action"], "task.change");
    assert_eq!(change["interface"], "desktop");
    assert_eq!(change["requested_by"], "mcp");
}

#[test]
fn a_declined_or_stale_request_changes_nothing_and_says_so() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 2);
    let declined = request_id(&s.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": ids}),
    ));
    let stale = request_id(&s.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": ids}),
    ));
    let desktop = data.desktop();
    desktop.decline_bulk_request(&declined).unwrap();
    desktop
        .patch_task(PatchTask {
            id: ids[1].clone(),
            expected_revision: 1,
            title: Some("Edited by the person".into()),
            description: None,
        })
        .unwrap();
    assert_eq!(
        desktop.approve_bulk_request(&stale).unwrap_err().code(),
        "stale_preview"
    );
    assert_eq!(
        s.ok("bulk_status", json!({"id": declined}))["item"]["status"],
        "declined"
    );
    assert_eq!(
        s.ok("bulk_status", json!({"id": stale}))["item"]["status"],
        "stale"
    );
    assert_eq!(active(&mut s), 2);
    s.refused(
        "bulk_status",
        json!({"id": "00000000-0000-4000-8000-000000000000"}),
        "not_found",
    );
}

#[test]
fn moves_bind_the_target_list_and_use_the_shared_target_rule() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 2);
    let list = s.ok("list_create", json!({"name": "Synthetic target"}))["item"].clone();
    let both = s.refused(
        "bulk_preview",
        json!({"action": "move_tasks", "ids": ids, "list_id": list["id"], "inbox": true}),
        "invalid_input",
    );
    let single = s.refused(
        "task_move",
        json!({"id": ids[0], "revision": 1, "list_id": list["id"], "inbox": true}),
        "invalid_input",
    );
    assert_eq!(both, single, "one move-target rule");
    let id = request_id(&s.ok(
        "bulk_preview",
        json!({"action": "move_tasks", "ids": ids, "list_id": list["id"]}),
    ));
    s.ok(
        "list_rename",
        json!({"id": list["id"], "revision": 1, "name": "Renamed synthetic"}),
    );
    assert_eq!(
        data.desktop().approve_bulk_request(&id).unwrap_err().code(),
        "stale_preview"
    );
}

#[test]
fn arguments_that_do_not_fit_the_action_are_invalid_input() {
    let data = Data::trusted();
    let mut s = data.server();
    let ids = task_ids(&mut s, 1);
    for arguments in [
        json!({"action": "delete_everything", "ids": ids}),
        json!({"action": "remove_tasks", "ids": ids, "status": "ready"}),
        json!({"action": "set_task_status", "ids": ids}),
        json!({"action": "remove_tasks", "ids": ids, "token": "x"}),
        json!({"ids": ids}),
        json!({"action": "remove_tasks", "ids": []}),
    ] {
        s.refused("bulk_preview", arguments, "invalid_input");
    }
    assert_eq!(data.desktop().count_pending_bulk_requests().unwrap(), 0);
}

#[test]
fn excluded_notes_cannot_be_named_and_a_private_folder_reveals_nothing() {
    let data = Data::trusted();
    let mut s = data.server();
    let desktop = data.desktop();
    let shared = s.ok("note_create", json!({"markdown": "# Shared synthetic"}))["item"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let private = desktop
        .create_note(CreateNote {
            markdown: "# Private synthetic".into(),
            folder_id: None,
        })
        .unwrap();
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let excluded = s.refused(
        "bulk_preview",
        json!({"action": "remove_notes", "ids": [shared, private.id]}),
        "not_found",
    );
    let unused = s.refused(
        "bulk_preview",
        json!({"action": "remove_notes", "ids": [shared, "00000000-0000-4000-8000-0000000000ff"]}),
        "not_found",
    );
    assert_eq!(excluded, unused);

    // A folder whose only Note is excluded: removing it needs approval, and
    // nothing the AI sees counts or names what is inside.
    let folder = desktop
        .create_note_folder(CreateNoteFolder {
            name: "Private synthetic folder".into(),
            parent_id: None,
        })
        .unwrap();
    desktop
        .move_note(tasknotes_core::notes::MoveNote {
            id: private.id.clone(),
            expected_revision: None,
            folder_id: Some(folder.id.clone()),
        })
        .unwrap();
    let refusal = s.refused(
        "folder_remove",
        json!({"id": folder.id, "revision": folder.revision}),
        "needs_approval",
    );
    let asked = s.ok(
        "bulk_preview",
        json!({"action": "remove_folder", "ids": [folder.id]}),
    )["item"]
        .clone();
    let shown = format!("{refusal}{asked}");
    assert!(!shown.contains(&private.id) && !shown.contains("Private synthetic\""));
    assert!(
        asked.get("count").is_none() && asked.get("items").is_none(),
        "{asked}"
    );
    assert!(s.stderr().is_empty());
}
