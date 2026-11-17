//! A Task's checklist through the CLI (batch 4): the core's rules, the same
//! envelope as every command. Synthetic data only.

mod common;

use common::{Cli, error_code};
use serde_json::Value;

fn id(item: &Value) -> &str {
    item["id"].as_str().unwrap()
}

fn rev(item: &Value) -> String {
    item["revision"].to_string()
}

#[test]
fn a_checklist_is_kept_through_the_cli() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Synthetic move"])["item"].clone();
    let first = cli.ok(&["subtask", "add", id(&task), "--text", "Pack the books"])["item"].clone();
    assert_eq!(first["task_id"], task["id"]);
    assert_eq!(first["done"], false);
    let second =
        cli.ok(&["subtask", "add", id(&task), "--text", "Label the boxes"])["item"].clone();

    let done = cli.ok(&[
        "subtask",
        "update",
        id(&first),
        "--revision",
        &rev(&first),
        "--done",
        "true",
    ])["item"]
        .clone();
    assert_eq!(done["done"], true);
    let read = cli.ok(&["task", "get", id(&task)])["item"].clone();
    assert_eq!(
        (
            read["subtasks_done"].as_u64(),
            read["subtasks_total"].as_u64()
        ),
        (Some(1), Some(2))
    );
    assert_eq!(read["progress"], 0, "the progress stays the person's own");

    cli.ok(&[
        "subtask",
        "move",
        id(&second),
        "--revision",
        &rev(&second),
        "--index",
        "0",
    ]);
    let list = cli.ok(&["subtask", "list", id(&task)]);
    let texts: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["Label the boxes", "Pack the books"]);

    let gone = cli.ok(&["subtask", "remove", id(&done), "--revision", &rev(&done)])["item"].clone();
    assert_eq!(gone["removed"], true);
    let back = cli.ok(&["subtask", "restore", id(&gone)])["item"].clone();
    assert_eq!(back["removed"], false);
    let audit = cli.ok(&["audit", "list"]);
    assert!(
        audit["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "subtask.restore")
    );
}

#[test]
fn checklist_refusals_use_their_own_codes() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Synthetic plan"])["item"].clone();
    assert_eq!(
        error_code(&cli.run(&["subtask", "add", id(&task), "--text", "  "]), 2),
        "invalid_input"
    );
    let item = cli.ok(&["subtask", "add", id(&task), "--text", "Draft"])["item"].clone();
    let stale = cli.run(&[
        "subtask",
        "update",
        id(&item),
        "--revision",
        "9",
        "--done",
        "true",
    ]);
    assert_eq!(error_code(&stale, 4), "revision_conflict");
    let bad = cli.run(&[
        "subtask",
        "update",
        id(&item),
        "--revision",
        "1",
        "--done",
        "maybe",
    ]);
    assert_eq!(error_code(&bad, 2), "invalid_input");
    assert_eq!(
        error_code(&cli.run(&["subtask", "restore", "no-such-item"]), 3),
        "not_found"
    );
}
