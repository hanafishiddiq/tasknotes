//! An MCP client the person has not authorized (Q47) is refused on every
//! tool call, before its arguments are read or the database is opened.

mod common;

use common::Data;
use serde_json::{Value, json};

fn not_trusted(server: &mut common::Server, tool: &str, arguments: Value) -> Value {
    let error = server.refused(tool, arguments, "not_trusted");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("tasknotes access grant --mcp"),
        "{error}"
    );
    error
}

#[test]
fn an_unconfigured_client_is_refused_without_creating_a_database() {
    let data = Data::untrusted();
    let mut server = data.server();
    // Discovery works, so the person can see what they would be granting.
    assert!(!server.tools().is_empty());
    for (tool, arguments) in [
        ("task_list", json!({})),
        ("task_create", json!({"title": "Synthetic"})),
        ("note_list", json!({})),
        ("note_create", json!({"markdown": "# Synthetic"})),
        ("audit_list", json!({})),
        // Malformed arguments are refused as untrusted, not inspected.
        ("task_get", json!({"nope": true})),
    ] {
        not_trusted(&mut server, tool, arguments);
    }
    assert!(!data.db().exists(), "the database was opened or created");
    assert!(server.stderr().is_empty(), "stderr: {}", server.stderr());
}

#[test]
fn cli_access_or_a_damaged_file_does_not_authorize_mcp() {
    for text in [
        r#"{"schema_version":1,"cli":true}"#,
        r#"{"schema_version":1,"mcp":"yes"}"#,
        r#"{"schema_version":2,"mcp":true}"#,
        "not json",
    ] {
        let data = Data::untrusted();
        data.write_access(text);
        let mut server = data.server();
        not_trusted(&mut server, "task_list", json!({}));
        assert!(!data.db().exists(), "{text}: the database was opened");
    }
}

#[test]
fn an_existing_record_is_refused_exactly_like_a_missing_one() {
    let data = Data::untrusted();
    let task = data
        .desktop()
        .create_task(tasknotes_core::tasks::CreateTask {
            title: "Synthetic private plan".into(),
            description: String::new(),
        });
    let id = task.unwrap().id;
    let mut server = data.server();
    let existing = server.call("task_get", json!({"id": id}));
    let missing = server.call(
        "task_get",
        json!({"id": "00000000-0000-4000-8000-000000000000"}),
    );
    assert_eq!(existing, missing);
    assert!(!existing.to_string().contains("Synthetic"));
}

#[test]
fn granting_and_revoking_apply_to_the_next_call() {
    let data = Data::untrusted();
    let mut server = data.server();
    not_trusted(&mut server, "task_list", json!({}));
    data.write_access(r#"{"schema_version":1,"mcp":true}"#);
    server.ok("task_list", json!({}));
    data.write_access(r#"{"schema_version":1,"mcp":false}"#);
    not_trusted(&mut server, "task_create", json!({"title": "After revoke"}));
    assert_eq!(data.desktop().list_tasks(10, None).unwrap().items.len(), 0);
}
