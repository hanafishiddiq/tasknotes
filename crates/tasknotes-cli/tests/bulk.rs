//! Adapter contract: Bulk Change requests through the `tasknotes` process.
//! The CLI can ask and read the status; only the desktop approves (owner
//! decision A). Synthetic data only.

mod common;

use common::{Cli, error_code};
use serde_json::Value;
use tasknotes_core::tasks::TaskStore;

fn create(cli: &Cli, title: &str) -> Value {
    cli.ok(&["task", "create", "--title", title])["item"].clone()
}

fn desktop(cli: &Cli) -> TaskStore {
    TaskStore::open(&cli.db()).unwrap()
}

#[test]
fn a_bulk_preview_from_the_cli_waits_for_the_desktop_and_reports_its_status() {
    let cli = Cli::trusted();
    let a = create(&cli, "Synthetic bulk A");
    let b = create(&cli, "Synthetic bulk B");
    let ids = [a["id"].as_str().unwrap(), b["id"].as_str().unwrap()];
    let asked = cli.ok(&[
        "bulk",
        "preview",
        "--action",
        "remove_tasks",
        "--id",
        ids[0],
        "--id",
        ids[1],
    ])["item"]
        .clone();
    assert_eq!(asked["action"], "remove_tasks");
    assert_eq!(asked["status"], "pending");
    assert_eq!(asked["count"], 2);
    assert_eq!(asked["requested_by"], "cli");
    assert_eq!(asked["items"][0]["before"], a);
    assert_eq!(asked["items"][1]["after"]["removed"], true);
    assert!(
        asked["message"].as_str().unwrap().contains("Tasknotes app"),
        "{asked}"
    );
    let id = asked["request_id"].as_str().unwrap();
    assert_eq!(
        cli.ok(&["task", "get", ids[0]])["item"],
        a,
        "nothing applied"
    );

    // The person approves in the window; the CLI reads the outcome.
    desktop(&cli).approve_bulk_request(id).unwrap();
    let status = cli.ok(&["bulk", "status", id])["item"].clone();
    assert_eq!(status["status"], "applied");
    assert!(status["decided_at"].is_i64());
    assert!(status.get("items").is_none());
    assert_eq!(cli.ok(&["task", "get", ids[0]])["item"]["removed"], true);

    let missing = cli.run(&["bulk", "status", "00000000-0000-4000-8000-000000000000"]);
    assert_eq!(error_code(&missing, 3), "not_found");
}

#[test]
fn bulk_moves_use_the_same_move_target_rule_as_task_move() {
    let cli = Cli::trusted();
    let task = create(&cli, "Synthetic move");
    let id = task["id"].as_str().unwrap();
    let list = cli.ok(&["list", "create", "--name", "Synthetic target"])["item"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for extra in [&[][..], &["--list", list.as_str(), "--inbox"][..]] {
        let mut args = vec!["bulk", "preview", "--action", "move_tasks", "--id", id];
        args.extend_from_slice(extra);
        let run = cli.run(&args);
        assert_eq!(error_code(&run, 2), "invalid_input");
        assert_eq!(
            run.json["error"]["message"],
            "Give exactly one of --list ID or --inbox"
        );
        let single = cli.run(&{
            let mut single = vec!["task", "move", id, "--revision", "1"];
            single.extend_from_slice(extra);
            single
        });
        assert_eq!(single.json["error"], run.json["error"], "one rule");
    }
    let asked = cli.ok(&[
        "bulk",
        "preview",
        "--action",
        "move_tasks",
        "--id",
        id,
        "--list",
        &list,
    ]);
    assert_eq!(asked["item"]["list_id"], list.as_str());
}

#[test]
fn bulk_preview_refuses_unknown_actions_and_options_that_do_not_belong() {
    let cli = Cli::trusted();
    let id = create(&cli, "Synthetic refusal")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for args in [
        vec![
            "bulk",
            "preview",
            "--action",
            "delete_everything",
            "--id",
            &id,
        ],
        vec![
            "bulk",
            "preview",
            "--action",
            "remove_tasks",
            "--id",
            &id,
            "--status",
            "ready",
        ],
        vec![
            "bulk",
            "preview",
            "--action",
            "set_task_status",
            "--id",
            &id,
        ],
        vec!["bulk", "preview", "--id", &id],
        vec!["bulk", "approve", &id],
    ] {
        let run = cli.run(&args);
        assert_eq!(error_code(&run, 2), "invalid_input", "{args:?}");
    }
    assert_eq!(desktop(&cli).count_pending_bulk_requests().unwrap(), 0);
}

#[test]
fn removing_a_list_that_holds_tasks_needs_approval_but_an_empty_one_does_not() {
    let cli = Cli::trusted();
    let empty = cli.ok(&["list", "create", "--name", "Empty synthetic"])["item"].clone();
    cli.ok(&[
        "list",
        "remove",
        empty["id"].as_str().unwrap(),
        "--revision",
        "1",
    ]);
    let full = cli.ok(&["list", "create", "--name", "Full synthetic"])["item"].clone();
    let full_id = full["id"].as_str().unwrap();
    cli.ok(&["task", "create", "--title", "Inside", "--list", full_id]);
    let run = cli.run(&["list", "remove", full_id, "--revision", "1"]);
    assert_eq!(error_code(&run, 5), "needs_approval");
    let asked = cli.ok(&[
        "bulk",
        "preview",
        "--action",
        "remove_list",
        "--id",
        full_id,
    ])["item"]
        .clone();
    assert_eq!(asked["status"], "pending");
    assert!(
        asked.get("count").is_none(),
        "no count for a container: {asked}"
    );
    assert!(asked.get("items").is_none(), "{asked}");
}
