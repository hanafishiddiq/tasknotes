//! Task, List, Note, folder, link, Removed and audit tools through the real
//! stdio server, with the desktop window closed. Synthetic data only.

mod common;

use common::Data;
use serde_json::{Value, json};

fn id(envelope: &Value) -> String {
    envelope["item"]["id"].as_str().unwrap().to_owned()
}

fn rev(envelope: &Value) -> i64 {
    envelope["item"]["revision"].as_i64().unwrap()
}

#[test]
fn tasks_and_lists_round_trip_with_revisions_progress_and_restore() {
    let data = Data::trusted();
    let mut s = data.server();
    let list = s.ok("list_create", json!({"name": "Synthetic Errands"}));
    let task = s.ok(
        "task_create",
        json!({"title": "Buy synthetic paint", "list_id": id(&list)}),
    );
    let t = id(&task);
    assert_eq!(task["item"]["list_id"], id(&list));

    let updated = s.ok(
        "task_update",
        json!({"id": t, "revision": rev(&task), "description": "two litres"}),
    );
    assert_eq!(updated["item"]["title"], "Buy synthetic paint");
    assert_eq!(updated["item"]["description"], "two litres");

    let progressed = s.ok(
        "task_progress",
        json!({"id": t, "revision": rev(&updated), "progress": 40, "next_step": "Pick colour", "note": "Visited shop"}),
    );
    assert_eq!(progressed["item"]["status"], "in_progress");
    assert_eq!(progressed["item"]["next_step"], "Pick colour");
    let history = s.ok("task_history", json!({"id": t}));
    assert_eq!(history["items"][0]["note"], "Visited shop");
    assert_eq!(history["next_cursor"], Value::Null);

    let changed = s.ok(
        "task_change",
        json!({"id": t, "revision": rev(&progressed), "priority": "very_high"}),
    );
    let detailed = s.ok(
        "task_details",
        json!({"id": t, "revision": rev(&changed), "tags": ["home"], "deadline": "2030-01-31"}),
    );
    assert_eq!(detailed["item"]["tags"], json!(["home"]));
    assert_eq!(detailed["item"]["deadline"], "2030-01-31");
    let cleared = s.ok(
        "task_details",
        json!({"id": t, "revision": rev(&detailed), "deadline": null}),
    );
    assert_eq!(cleared["item"]["deadline"], Value::Null);
    assert_eq!(
        cleared["item"]["tags"],
        json!(["home"]),
        "left-out tags kept"
    );

    let moved = s.ok(
        "task_move",
        json!({"id": t, "revision": rev(&cleared), "inbox": true}),
    );
    assert_eq!(moved["item"]["list_id"], Value::Null);

    let found = s.ok(
        "task_list",
        json!({"text": "synthetic paint", "priority": "very_high"}),
    );
    assert_eq!(found["items"].as_array().unwrap().len(), 1);
    assert_eq!(found["schema_version"], 1);

    let removed = s.ok("task_remove", json!({"id": t, "revision": rev(&moved)}));
    assert_eq!(removed["item"]["removed"], true);
    assert_eq!(s.ok("task_list", json!({}))["items"], json!([]));
    let bin = s.ok("removed_list", json!({}));
    assert_eq!(bin["items"][0]["id"], t);
    assert_eq!(bin["items"][0]["kind"], "task");
    let restored = s.ok(
        "task_restore",
        json!({"id": t, "revision": bin["items"][0]["revision"]}),
    );
    assert_eq!(restored["item"]["removed"], false);

    let renamed = s.ok(
        "list_rename",
        json!({"id": id(&list), "revision": rev(&list), "name": "Synthetic Chores"}),
    );
    let gone = s.ok(
        "list_remove",
        json!({"id": id(&list), "revision": rev(&renamed)}),
    );
    s.ok(
        "list_restore",
        json!({"id": id(&list), "revision": rev(&gone)}),
    );
    assert_eq!(
        s.ok("list_list", json!({}))["items"][0]["name"],
        "Synthetic Chores"
    );

    let audit = s.ok("audit_list", json!({"entity_id": t}));
    let actions: Vec<&str> = audit["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        [
            "task.restore",
            "task.remove",
            "task.move",
            "task.details",
            "task.details",
            "task.change",
            "task.progress",
            "task.update",
            "task.create"
        ]
    );
    assert!(
        audit["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["interface"] == "mcp")
    );
}

#[test]
fn notes_folders_and_links_round_trip() {
    let data = Data::trusted();
    let mut s = data.server();
    let work = s.ok("folder_create", json!({"name": "Synthetic Work"}));
    let client = s.ok(
        "folder_create",
        json!({"name": "Client A", "parent_id": id(&work)}),
    );
    assert_eq!(client["item"]["path"], "Synthetic Work/Client A");
    let note = s.ok(
        "note_create",
        json!({"markdown": "# Kickoff notes\n\nSynthetic agenda", "folder_id": id(&client)}),
    );
    let n = id(&note);
    assert_eq!(note["item"]["title"], "Kickoff notes");
    assert_eq!(note["item"]["folder_path"], "Synthetic Work/Client A");

    let updated = s.ok(
        "note_update",
        json!({"id": n, "revision": rev(&note), "markdown": "# Kickoff notes\n\nRevised agenda"}),
    );
    assert_eq!(s.ok("note_get", json!({"id": n}))["item"], updated["item"]);
    let hits = s.ok("note_list", json!({"text": "revised"}));
    assert_eq!(hits["items"][0]["id"], n);
    assert!(
        hits["items"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("Revised")
    );
    assert_eq!(
        s.ok("folder_get", json!({"id": id(&client)}))["item"]["note_count"],
        1
    );

    let moved = s.ok(
        "note_move",
        json!({"id": n, "revision": rev(&updated), "no_folder": true}),
    );
    assert_eq!(moved["item"]["folder_id"], Value::Null);
    let folder_moved = s.ok(
        "folder_move",
        json!({"id": id(&client), "revision": rev(&client), "top": true}),
    );
    assert_eq!(folder_moved["item"]["path"], "Client A");
    assert_eq!(
        s.ok("folder_list", json!({}))["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let renamed = s.ok(
        "folder_rename",
        json!({"id": id(&client), "revision": rev(&folder_moved), "name": "Client B"}),
    );
    let folder_gone = s.ok(
        "folder_remove",
        json!({"id": id(&client), "revision": rev(&renamed)}),
    );
    s.ok(
        "folder_restore",
        json!({"id": id(&client), "revision": rev(&folder_gone)}),
    );

    let task = s.ok("task_create", json!({"title": "Prepare synthetic kickoff"}));
    let link = s.ok("link_create", json!({"task_id": id(&task), "note_id": n}));
    let l = id(&link);
    assert_eq!(s.ok("link_get", json!({"id": l}))["item"], link["item"]);
    assert_eq!(
        s.ok("task_notes", json!({"id": id(&task)}))["items"][0]["note_id"],
        n
    );
    assert_eq!(
        s.ok("note_tasks", json!({"id": n}))["items"][0]["task_id"],
        id(&task)
    );
    let unlinked = s.ok("link_remove", json!({"id": l}));
    assert!(unlinked["item"]["unlinked_at"].is_i64());
    assert_eq!(
        s.ok("task_notes", json!({"id": id(&task)}))["items"],
        json!([])
    );
    s.ok("link_restore", json!({"id": l}));

    let removed = s.ok("note_remove", json!({"id": n, "revision": rev(&moved)}));
    assert_eq!(s.ok("note_list", json!({}))["items"], json!([]));
    s.ok("note_restore", json!({"id": n, "revision": rev(&removed)}));
    assert_eq!(s.ok("note_list", json!({}))["items"][0]["id"], n);
}

#[test]
fn pages_carry_an_opaque_cursor_through_to_the_end() {
    let data = Data::trusted();
    let mut s = data.server();
    for i in 0..5 {
        s.ok(
            "task_create",
            json!({"title": format!("Synthetic task {i}")}),
        );
    }
    let mut seen = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let mut args = json!({"limit": 2});
        if !cursor.is_null() {
            args["cursor"] = cursor.clone();
        }
        let page = s.ok("task_list", args);
        seen.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["id"].clone()),
        );
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(seen.len(), 5);
    s.refused(
        "task_list",
        json!({"cursor": "not-a-cursor"}),
        "invalid_cursor",
    );
    s.refused("task_list", json!({"limit": 0}), "invalid_input");
}

#[test]
fn a_stale_revision_is_refused_and_the_desktop_edit_survives() {
    let data = Data::trusted();
    let mut s = data.server();
    let task = s.ok("task_create", json!({"title": "Synthetic shared draft"}));
    let t = id(&task);
    // The person edits it in the window after the AI read it.
    data.desktop()
        .patch_task(tasknotes_core::tasks::PatchTask {
            id: t.clone(),
            expected_revision: 1,
            title: Some("Edited by the person".into()),
            description: None,
        })
        .unwrap();
    let error = s.refused(
        "task_update",
        json!({"id": t, "revision": 1, "title": "AI overwrite"}),
        "revision_conflict",
    );
    assert!(error["message"].as_str().unwrap().contains("reload"));
    assert_eq!(
        data.desktop().get_task(&t).unwrap().unwrap().title,
        "Edited by the person"
    );
    let note = s.ok(
        "note_create",
        json!({"markdown": "# Synthetic shared note"}),
    );
    s.ok(
        "note_update",
        json!({"id": id(&note), "revision": 1, "markdown": "# First AI edit"}),
    );
    s.refused(
        "note_update",
        json!({"id": id(&note), "revision": 1, "markdown": "# Stale AI edit"}),
        "revision_conflict",
    );
    // The refusal is an Audit Entry the person can investigate.
    let audit = s.ok("audit_list", json!({"entity_id": t}));
    assert_eq!(audit["items"][0]["outcome"], "rejected");
    assert_eq!(audit["items"][0]["error_code"], "revision_conflict");
}

#[test]
fn lists_and_folders_reorder_with_the_same_rules_as_the_window() {
    let data = Data::trusted();
    let mut s = data.server();
    let a = s.ok("list_create", json!({"name": "Synthetic A"}));
    let b = s.ok("list_create", json!({"name": "Synthetic B"}));
    let c = s.ok("list_create", json!({"name": "Synthetic C"}));
    let names = |page: Value| -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap().to_owned())
            .collect()
    };

    let moved = s.ok(
        "list_reorder",
        json!({"id": id(&c), "revision": rev(&c), "after_id": id(&a)}),
    );
    assert_eq!(rev(&moved), rev(&c) + 1);
    assert_eq!(
        names(s.ok("list_list", json!({}))),
        ["Synthetic A", "Synthetic C", "Synthetic B"]
    );
    s.ok(
        "list_reorder",
        json!({"id": id(&b), "revision": rev(&b), "first": true}),
    );
    assert_eq!(
        names(s.ok("list_list", json!({}))),
        ["Synthetic B", "Synthetic A", "Synthetic C"]
    );
    // The revision it last read is out of date now.
    s.refused(
        "list_reorder",
        json!({"id": id(&c), "revision": rev(&c), "first": true}),
        "revision_conflict",
    );
    s.refused(
        "list_reorder",
        json!({"id": id(&a), "revision": rev(&a), "first": true, "after_id": id(&b)}),
        "invalid_input",
    );

    let work = s.ok("folder_create", json!({"name": "Synthetic Work"}));
    let home = s.ok("folder_create", json!({"name": "Synthetic Home"}));
    let inner = s.ok(
        "folder_create",
        json!({"name": "Synthetic Inner", "parent_id": id(&work)}),
    );
    let placed = s.ok(
        "folder_reorder",
        json!({"id": id(&home), "revision": rev(&home), "first": true}),
    );
    assert_eq!(
        names(s.ok("folder_list", json!({}))),
        ["Synthetic Home", "Synthetic Work"]
    );
    let nested = s.ok(
        "folder_reorder",
        json!({
            "id": id(&placed), "revision": rev(&placed),
            "parent_id": id(&work), "after_id": id(&inner),
        }),
    );
    assert_eq!(nested["item"]["parent_id"], id(&work));
    assert_eq!(
        names(s.ok("folder_list", json!({"parent_id": id(&work)}))),
        ["Synthetic Inner", "Synthetic Home"]
    );
    s.ok(
        "folder_reorder",
        json!({"id": id(&nested), "revision": rev(&nested), "top": true, "first": true}),
    );
    assert_eq!(
        names(s.ok("folder_list", json!({}))),
        ["Synthetic Home", "Synthetic Work"]
    );
}

/// Ticket 09: an AI client reads, sets, clears, filters and sorts by
/// Difficulty; `difficulty` is `null` for No Difficulty.
#[test]
fn difficulty_is_set_cleared_filtered_and_sorted_through_mcp() {
    let data = Data::trusted();
    let mut s = data.server();
    let schema = s
        .tools()
        .into_iter()
        .find(|tool| tool["name"] == "task_change")
        .unwrap()["inputSchema"]["properties"]["difficulty"]
        .clone();
    assert_eq!(schema["type"], json!(["string", "null"]), "{schema}");
    assert_eq!(
        schema["enum"],
        json!(["very_easy", "easy", "medium", "hard", "very_hard", null])
    );
    let list = s
        .tools()
        .into_iter()
        .find(|tool| tool["name"] == "task_list")
        .unwrap();
    let filter = list["inputSchema"]["properties"]["difficulty"].clone();
    assert_eq!(
        filter["enum"],
        json!(["very_easy", "easy", "medium", "hard", "very_hard", "none"]),
        "{filter}"
    );
    assert!(
        list["description"]
            .as_str()
            .unwrap()
            .contains("difficulty \"none\""),
        "{list}"
    );

    let plain = s.ok("task_create", json!({"title": "Synthetic plain"}));
    assert!(plain["item"]["difficulty"].is_null());
    let easy = s.ok("task_create", json!({"title": "Synthetic easy"}));
    let hard = s.ok("task_create", json!({"title": "Synthetic hard"}));
    let hard = s.ok(
        "task_change",
        json!({"id": id(&hard), "revision": rev(&hard), "difficulty": "very_hard"}),
    );
    assert_eq!(hard["item"]["difficulty"], "very_hard");
    s.ok(
        "task_change",
        json!({"id": id(&easy), "revision": rev(&easy), "difficulty": "easy"}),
    );

    let titles = |s: &mut common::Server, args: Value| -> Vec<String> {
        s.ok("task_list", args)["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        titles(&mut s, json!({"sort": "difficulty"})),
        ["Synthetic hard", "Synthetic easy", "Synthetic plain"]
    );
    assert_eq!(
        titles(&mut s, json!({"difficulty": "easy"})),
        ["Synthetic easy"]
    );
    assert_eq!(
        titles(&mut s, json!({"difficulty": "none"})),
        ["Synthetic plain"]
    );
    s.refused(
        "task_list",
        json!({"difficulty": "brutal"}),
        "invalid_input",
    );

    // A priority change leaves the Difficulty alone; null clears it.
    let kept = s.ok(
        "task_change",
        json!({"id": id(&hard), "revision": rev(&hard), "priority": "high"}),
    );
    assert_eq!(kept["item"]["difficulty"], "very_hard");
    let cleared = s.ok(
        "task_change",
        json!({"id": id(&kept), "revision": rev(&kept), "difficulty": null}),
    );
    assert!(cleared["item"]["difficulty"].is_null());
    s.refused(
        "task_change",
        json!({"id": id(&cleared), "revision": rev(&cleared), "difficulty": "brutal"}),
        "invalid_input",
    );
}

/// Ticket 01: an AI client reads, sets and clears a List's or Note Folder's
/// icon and colour with revision checks; `null` is the default look.
#[test]
fn icons_and_colours_are_read_set_and_cleared_through_mcp() {
    let data = Data::trusted();
    let mut s = data.server();
    let made = s.ok("list_create", json!({"name": "🏠 Synthetic home"}));
    assert!(made["item"]["icon"].is_null() && made["item"]["color"].is_null());

    let styled = s.ok(
        "list_style",
        json!({"id": id(&made), "revision": rev(&made), "icon": "house", "color": "green"}),
    );
    assert_eq!(styled["item"]["icon"], "house");
    assert_eq!(styled["item"]["color"], "green");
    assert_eq!(styled["item"]["name"], "🏠 Synthetic home");
    assert_eq!(s.ok("list_list", json!({}))["items"][0], styled["item"]);

    // Left out keeps; null clears.
    let cleared = s.ok(
        "list_style",
        json!({"id": id(&made), "revision": rev(&styled), "icon": null}),
    );
    assert!(cleared["item"]["icon"].is_null());
    assert_eq!(cleared["item"]["color"], "green");
    s.refused(
        "list_style",
        json!({"id": id(&made), "revision": rev(&styled), "color": "red"}),
        "revision_conflict",
    );
    s.refused(
        "list_style",
        json!({"id": id(&made), "revision": rev(&cleared), "icon": "not-an-icon"}),
        "invalid_input",
    );
    s.refused(
        "list_style",
        json!({"id": id(&made), "revision": rev(&cleared), "color": "magenta"}),
        "invalid_input",
    );

    let folder = s.ok("folder_create", json!({"name": "Synthetic recipes"}));
    let styled = s.ok(
        "folder_style",
        json!({"id": id(&folder), "revision": rev(&folder), "icon": "chef-hat", "color": "pink"}),
    );
    assert_eq!(styled["item"]["icon"], "chef-hat");
    assert_eq!(styled["item"]["color"], "pink");
    assert_eq!(
        s.ok("folder_get", json!({"id": id(&folder)}))["item"],
        styled["item"]
    );
    let plain = s.ok(
        "folder_style",
        json!({"id": id(&folder), "revision": rev(&styled), "icon": null, "color": null}),
    );
    assert!(plain["item"]["icon"].is_null() && plain["item"]["color"].is_null());

    let options = s.ok("style_options", json!({}));
    assert_eq!(options["item"]["colors"].as_array().unwrap().len(), 12);
    assert!(
        options["item"]["icons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|icon| icon == "chef-hat")
    );
}

/// Ticket 10: an AI client sets, clears and filters Won't Do; the history
/// records the transition.
#[test]
fn wont_do_is_set_cleared_and_filtered_through_mcp() {
    let data = Data::trusted();
    let mut s = data.server();
    let change = s
        .tools()
        .into_iter()
        .find(|tool| tool["name"] == "task_change")
        .unwrap();
    assert_eq!(
        change["inputSchema"]["properties"]["wont_do"]["type"],
        json!(["boolean", "null"]),
        "{change}"
    );
    assert!(
        change["description"].as_str().unwrap().contains("Won't Do"),
        "{change}"
    );
    let list = s
        .tools()
        .into_iter()
        .find(|tool| tool["name"] == "task_list")
        .unwrap();
    assert_eq!(
        list["inputSchema"]["properties"]["wont_do"]["type"],
        json!(["boolean", "null"]),
        "{list}"
    );

    let open = s.ok("task_create", json!({"title": "Synthetic open"}));
    assert_eq!(open["item"]["wont_do"], false);
    let aside = s.ok("task_create", json!({"title": "Synthetic set aside"}));
    let aside = s.ok(
        "task_change",
        json!({"id": id(&aside), "revision": rev(&aside), "progress": 30}),
    );
    let aside = s.ok(
        "task_change",
        json!({"id": id(&aside), "revision": rev(&aside), "wont_do": true}),
    );
    assert_eq!(aside["item"]["status"], "completed");
    assert_eq!(aside["item"]["wont_do"], true);
    assert_eq!(aside["item"]["progress"], 30);
    let history = s.ok("task_history", json!({"id": id(&aside)}))["items"][0].clone();
    assert_eq!(
        (
            history["previous_wont_do"].clone(),
            history["wont_do"].clone()
        ),
        (json!(false), json!(true))
    );

    let titles = |s: &mut common::Server, args: Value| -> Vec<String> {
        s.ok("task_list", args)["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        titles(&mut s, json!({"wont_do": true})),
        ["Synthetic set aside"]
    );
    assert_eq!(
        titles(&mut s, json!({"wont_do": false})),
        ["Synthetic open"]
    );

    s.refused(
        "task_change",
        json!({"id": id(&aside), "revision": rev(&aside), "wont_do": true, "progress": 50}),
        "invalid_transition",
    );
    let reopened = s.ok(
        "task_change",
        json!({"id": id(&aside), "revision": rev(&aside), "wont_do": false}),
    );
    assert_eq!(reopened["item"]["wont_do"], false);
    assert_eq!(reopened["item"]["status"], "in_progress");
    assert_eq!(reopened["item"]["progress"], 30);
}

/// Ticket 12: a List inside a List, moved and read as a tree; removing one
/// that holds anything waits for the person.
#[test]
fn lists_nest_move_and_read_as_a_tree() {
    let data = Data::trusted();
    let mut s = data.server();
    let work = s.ok("list_create", json!({"name": "Synthetic Work"}));
    let home = s.ok("list_create", json!({"name": "Synthetic Home"}));
    let reports = s.ok(
        "list_create",
        json!({"name": "Synthetic Reports", "parent_id": id(&work)}),
    );
    assert_eq!(reports["item"]["parent_id"], id(&work));
    let tree = |s: &mut common::Server| -> Vec<(String, Value)> {
        s.ok("list_list", json!({}))["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["name"].as_str().unwrap().to_owned(),
                    l["parent_id"].clone(),
                )
            })
            .collect()
    };
    assert_eq!(
        tree(&mut s),
        [
            ("Synthetic Work".to_owned(), Value::Null),
            ("Synthetic Reports".to_owned(), json!(id(&work))),
            ("Synthetic Home".to_owned(), Value::Null),
        ]
    );

    let moved = s.ok(
        "list_move",
        json!({"id": id(&reports), "revision": rev(&reports), "parent_id": id(&home)}),
    );
    assert_eq!(moved["item"]["parent_id"], id(&home));
    s.refused(
        "list_move",
        json!({"id": id(&reports), "revision": rev(&moved)}),
        "invalid_input",
    );
    let home_now = s.ok("list_list", json!({}))["items"][1].clone();
    s.refused(
        "list_move",
        json!({"id": id(&home), "revision": home_now["revision"], "parent_id": id(&reports)}),
        "invalid_input",
    );
    let placed = s.ok(
        "list_reorder",
        json!({"id": id(&reports), "revision": rev(&moved), "first": true, "top": true}),
    );
    assert!(placed["item"]["parent_id"].is_null());
    assert_eq!(tree(&mut s)[0].0, "Synthetic Reports");

    let inner = s.ok(
        "list_create",
        json!({"name": "Synthetic Inner", "parent_id": id(&work)}),
    );
    let work_now = s.ok("list_list", json!({}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == id(&work))
        .unwrap()
        .clone();
    assert_eq!(work_now["has_children"], true);
    s.refused(
        "list_remove",
        json!({"id": id(&work), "revision": work_now["revision"]}),
        "needs_approval",
    );
    assert_eq!(
        tree(&mut s)
            .iter()
            .filter(|(name, _)| name == "Synthetic Inner")
            .count(),
        1,
        "nothing was hidden: {}",
        id(&inner)
    );
}
