//! The MCP stdio protocol end to end: handshake, tool listing, a Task round
//! trip shared with the desktop, clean shutdown and no network listener.

mod common;

use common::{Data, PROTOCOL};
use serde_json::json;

#[test]
fn a_trusted_client_completes_the_handshake_and_lists_the_tools() {
    let data = Data::trusted();
    let mut server = common::Server::spawn(data.path());
    let init = server.initialize();
    let result = &init["result"];
    assert_eq!(result["protocolVersion"], PROTOCOL);
    assert_eq!(result["serverInfo"]["name"], "tasknotes");
    assert!(result["capabilities"]["tools"].is_object());

    let tools = server.tools();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "task_list",
        "task_get",
        "task_create",
        "task_update",
        "task_progress",
        "task_remove",
        "task_restore",
        "list_create",
        "note_create",
        "note_list",
        "note_update",
        "link_create",
        "removed_list",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }
    // The whole set, so a tool added or lost shows up here (and in the docs).
    let mut all = names.clone();
    all.sort_unstable();
    assert_eq!(
        all,
        [
            "audit_list",
            "bulk_preview",
            "bulk_status",
            "folder_create",
            "folder_get",
            "folder_list",
            "folder_move",
            "folder_remove",
            "folder_rename",
            "folder_reorder",
            "folder_restore",
            "link_create",
            "link_get",
            "link_remove",
            "link_restore",
            "list_create",
            "list_list",
            "list_remove",
            "list_rename",
            "list_reorder",
            "list_restore",
            "note_create",
            "note_get",
            "note_list",
            "note_move",
            "note_remove",
            "note_restore",
            "note_tasks",
            "note_update",
            "removed_list",
            "task_change",
            "task_create",
            "task_details",
            "task_get",
            "task_history",
            "task_list",
            "task_move",
            "task_notes",
            "task_progress",
            "task_remove",
            "task_restore",
            "task_update",
        ]
    );
    assert_eq!(tools.len(), 42);
    for tool in &tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["inputSchema"].get("oneOf").is_none(), "{tool}");
        assert!(tool["description"].as_str().unwrap().len() > 10, "{tool}");
    }
    assert_eq!(server.close(), Some(0));
}

#[test]
fn a_task_created_through_mcp_is_the_desktops_task() {
    let data = Data::trusted();
    let mut server = data.server();
    let created = server.ok(
        "task_create",
        json!({"title": "Draft the synthetic report", "description": "fixture"}),
    );
    let task = &created["item"];
    assert_eq!(task["status"], "ready");
    assert_eq!(task["revision"], 1);
    let id = task["id"].as_str().unwrap();

    let read = server.ok("task_get", json!({"id": id}));
    assert_eq!(read["item"], *task);

    let desktop = data.desktop().get_task(id).unwrap().unwrap();
    assert_eq!(desktop.title, "Draft the synthetic report");

    let audit = data
        .desktop()
        .list_audit_entries(Some(id), 10, None)
        .unwrap();
    assert_eq!(audit.items.len(), 1);
    assert_eq!(audit.items[0].interface.as_str(), "mcp");
    assert!(server.stderr().is_empty(), "stderr: {}", server.stderr());
}

#[test]
fn the_server_listens_on_no_network_port() {
    let data = Data::trusted();
    let mut server = data.server();
    server.ok("task_list", json!({}));
    let pid = server.pid().to_string();
    let Ok(output) = std::process::Command::new("lsof")
        .args(["-nP", "-a", "-p", &pid, "-i"])
        .output()
    else {
        eprintln!("lsof is not available; skipping the socket check");
        return;
    };
    // lsof prints nothing (and exits 1) when the process has no internet socket.
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "the server opened a socket: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
