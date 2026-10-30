//! Malformed, unknown and pipelined requests get safe, deterministic answers
//! and never stop the server.

mod common;

use common::Data;
use serde_json::{Value, json};

#[test]
fn malformed_lines_and_unknown_methods_do_not_stop_the_server() {
    let data = Data::trusted();
    let mut s = data.server();
    // Not JSON-RPC 2.0: at most a JSON-RPC error without an id, never a
    // result, and the server keeps serving.
    for line in [
        "this is not json",
        "[1,2]",
        r#"{"id":99,"method":"tools/list"}"#,
    ] {
        s.send_line(line);
    }
    s.send_line(r#"{"jsonrpc":"2.0","id":"probe","method":"bogus/method","params":{}}"#);
    let unknown_method = loop {
        let message = s.read();
        if message["id"] == "probe" {
            break message;
        }
        assert!(message.get("result").is_none(), "{message}");
        assert_eq!(message["id"], Value::Null, "{message}");
        let code = message["error"]["code"].as_i64().unwrap();
        assert!([-32700, -32600].contains(&code), "{message}");
    };
    assert_eq!(unknown_method["error"]["code"], -32601);
    let unknown_tool = s.request(
        "tools/call",
        json!({"name": "drop_database", "arguments": {}}),
    );
    assert_eq!(unknown_tool["error"]["code"], -32602);
    assert_eq!(
        unknown_tool["error"]["message"],
        "Unknown tool: drop_database"
    );
    let no_name = s.request("tools/call", json!({"arguments": {}}));
    assert!(no_name.get("error").is_some(), "{no_name}");
    // Still serving.
    s.ok("task_list", json!({}));
    assert!(s.stderr().is_empty(), "stderr: {}", s.stderr());
}

#[test]
fn a_line_that_is_not_json_gets_no_reply_and_the_next_request_is_answered() {
    let data = Data::trusted();
    let mut s = data.server();
    // rmcp, like the other official MCP SDKs, skips a line it cannot parse
    // as JSON: deterministically no reply at all (not even an id-less
    // error), and the server keeps serving.
    for line in [
        "this is not json",
        "{\"jsonrpc\": \"2.0\", \"id\": 7, ",
        "\u{1}\u{2}",
    ] {
        s.send_line(line);
    }
    assert!(
        s.silent_for(std::time::Duration::from_millis(500)),
        "no reply to unparsable lines"
    );
    let tools = s.request("tools/list", json!({}));
    assert!(tools["result"]["tools"].is_array(), "{tools}");
    s.ok("task_list", json!({}));
    assert!(s.stderr().is_empty(), "stderr: {}", s.stderr());
}

#[test]
fn bad_arguments_are_invalid_input_and_change_nothing() {
    let data = Data::trusted();
    let mut s = data.server();
    let cases = [
        ("task_create", json!({}), "missing field `title`"),
        ("task_create", json!({"title": 7}), "expected a string"),
        (
            "task_create",
            json!({"title": "x", "colour": "red"}),
            "unknown field `colour`",
        ),
        (
            "task_change",
            json!({"id": "x", "revision": 1, "status": "done"}),
            "unknown variant `done`",
        ),
        (
            "task_change",
            json!({"id": "x", "revision": 1, "progress": 300}),
            "invalid value",
        ),
        (
            "task_update",
            json!({"id": "x", "revision": 0, "title": "y"}),
            "revision must be",
        ),
        (
            "task_move",
            json!({"id": "x", "revision": 1}),
            "exactly one of list_id or inbox",
        ),
        (
            "note_move",
            json!({"id": "x", "revision": 1, "folder_id": "f", "no_folder": true}),
            "exactly one",
        ),
    ];
    for (tool, arguments, message) in cases {
        let error = s.refused(tool, arguments.clone(), "invalid_input");
        assert!(
            error["message"].as_str().unwrap().contains(message),
            "{tool} {arguments}: {error}"
        );
    }
    // A non-object `arguments` is a malformed request, not a tool call.
    let response = s.request(
        "tools/call",
        json!({"name": "task_create", "arguments": "x"}),
    );
    assert!(response.get("error").is_some(), "{response}");
    assert_eq!(s.ok("task_list", json!({}))["items"], json!([]));
    assert_eq!(s.ok("audit_list", json!({}))["items"], json!([]));
}

#[test]
fn pipelined_calls_on_a_new_data_directory_all_succeed() {
    let data = Data::trusted();
    let mut s = data.server();
    let count = 12;
    for id in 0..count {
        let message = json!({
            "jsonrpc": "2.0", "id": 1000 + id, "method": "tools/call",
            "params": {"name": "task_create", "arguments": {"title": format!("Pipelined {id}")}},
        });
        s.send_line(&message.to_string());
    }
    let mut answered = Vec::new();
    for _ in 0..count {
        let response = s.read();
        let envelope: &Value = &response["result"]["structuredContent"];
        assert_eq!(envelope["ok"], true, "{response}");
        answered.push(response["id"].as_i64().unwrap());
    }
    answered.sort();
    assert_eq!(answered, (1000..1000 + count).collect::<Vec<_>>());
    assert_eq!(
        data.desktop().list_tasks(50, None).unwrap().items.len(),
        count as usize
    );
}
