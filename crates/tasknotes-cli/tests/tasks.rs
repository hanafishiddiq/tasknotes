//! Adapter contract: Task CRUD through the separate `tasknotes` process.

mod common;

use common::{Cli, error_code};
use serde_json::Value;

fn create(cli: &Cli, title: &str) -> Value {
    cli.ok(&["task", "create", "--title", title])["item"].clone()
}

#[test]
fn a_task_created_by_the_cli_is_readable_with_stable_id_and_revision() {
    let cli = Cli::trusted();
    let created = cli.ok(&[
        "task",
        "create",
        "--title",
        "  Book the dentist  ",
        "--description",
        "Ask for a morning slot",
    ]);
    let item = &created["item"];
    assert_eq!(item["title"], "Book the dentist");
    assert_eq!(item["status"], "ready");
    assert_eq!(item["priority"], "medium");
    assert_eq!(item["progress"], 0);
    assert_eq!(item["revision"], 1);
    assert_eq!(item["removed"], false);
    assert!(item["list_id"].is_null());

    let id = item["id"].as_str().unwrap();
    let fetched = cli.ok(&["task", "get", id]);
    assert_eq!(fetched["item"], *item);

    let missing = cli.run(&["task", "get", "00000000-0000-4000-8000-000000000000"]);
    assert_eq!(error_code(&missing, 3), "not_found");
}

#[test]
fn listing_pages_fifty_tasks_by_default_and_follows_next_cursor() {
    let cli = Cli::trusted();
    {
        let store = tasknotes_core::tasks::TaskStore::open(&cli.db()).unwrap();
        for n in 0..55 {
            store
                .create_task(tasknotes_core::tasks::CreateTask {
                    title: format!("Synthetic chore {n:02}"),
                    description: String::new(),
                })
                .unwrap();
        }
    }
    let first = cli.ok(&["task", "list"]);
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = cli.ok(&["task", "list", "--cursor", cursor]);
    assert_eq!(second["items"].as_array().unwrap().len(), 5);
    assert!(second["next_cursor"].is_null());

    let mut ids: Vec<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        55,
        "no Task repeats or goes missing across pages"
    );

    let small = cli.ok(&["task", "list", "--limit", "2"]);
    assert_eq!(small["items"].as_array().unwrap().len(), 2);
}

#[test]
fn listing_filters_by_text_status_priority_tag_list_and_removed() {
    let cli = Cli::trusted();
    let list = cli.ok(&["list", "create", "--name", "Home"])["item"].clone();
    let list_id = list["id"].as_str().unwrap();
    let paint = cli.ok(&[
        "task",
        "create",
        "--title",
        "Paint the fence",
        "--list",
        list_id,
    ])["item"]
        .clone();
    let taxes = create(&cli, "File taxes");
    let taxes = cli.ok(&[
        "task",
        "change",
        taxes["id"].as_str().unwrap(),
        "--revision",
        "1",
        "--priority",
        "very_high",
        "--progress",
        "30",
    ])["item"]
        .clone();
    cli.ok(&[
        "task",
        "details",
        taxes["id"].as_str().unwrap(),
        "--revision",
        "2",
        "--tag",
        "money",
    ]);
    let old = create(&cli, "Old idea");
    cli.ok(&[
        "task",
        "remove",
        old["id"].as_str().unwrap(),
        "--revision",
        "1",
    ]);

    let titles = |args: &[&str]| -> Vec<String> {
        let mut full = vec!["task", "list"];
        full.extend_from_slice(args);
        cli.ok(&full)["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap().to_owned())
            .collect()
    };
    // Fixed group order: In Progress before Ready.
    assert_eq!(titles(&[]), ["File taxes", "Paint the fence"]);
    assert_eq!(titles(&["--text", "fence"]), ["Paint the fence"]);
    assert_eq!(titles(&["--status", "in_progress"]), ["File taxes"]);
    assert_eq!(titles(&["--priority", "very_high"]), ["File taxes"]);
    assert_eq!(titles(&["--tag", "MONEY"]), ["File taxes"]);
    assert_eq!(titles(&["--list", list_id]), ["Paint the fence"]);
    assert_eq!(titles(&["--inbox"]), ["File taxes"]);
    assert_eq!(titles(&["--removed"]), ["Old idea"]);
    assert_eq!(
        titles(&["--sort", "newest"]),
        ["File taxes", "Paint the fence"]
    );
    assert_eq!(paint["list_id"], list["id"]);
}

fn id(value: &Value) -> String {
    value["id"].as_str().unwrap().to_owned()
}

#[test]
fn a_task_is_edited_progressed_moved_removed_and_restored_through_the_cli() {
    let cli = Cli::trusted();
    let task = cli.ok(&[
        "task",
        "create",
        "--title",
        "Write the newsletter",
        "--description",
        "Three short sections",
    ])["item"]
        .clone();
    let tid = id(&task);

    // A partial update keeps the description.
    let task = cli.ok(&[
        "task",
        "update",
        &tid,
        "--revision",
        "1",
        "--title",
        "Write the June newsletter",
    ])["item"]
        .clone();
    assert_eq!(task["description"], "Three short sections");
    assert_eq!(task["revision"], 2);

    let task = cli.ok(&[
        "task",
        "progress",
        &tid,
        "--revision",
        "2",
        "--progress",
        "60",
        "--next-step",
        "Proofread section two",
        "--note",
        "Drafted sections one and two",
    ])["item"]
        .clone();
    assert_eq!(
        (
            task["status"].as_str(),
            task["progress"].as_u64(),
            task["next_step"].as_str()
        ),
        (Some("in_progress"), Some(60), Some("Proofread section two"))
    );
    let history = cli.ok(&["task", "history", &tid]);
    assert_eq!(history["items"][0]["note"], "Drafted sections one and two");
    assert_eq!(history["items"][0]["previous_progress"], 0);

    let task = cli.ok(&[
        "task",
        "details",
        &tid,
        "--revision",
        "3",
        "--tag",
        "writing",
        "--deadline",
        "2031-06-30",
    ])["item"]
        .clone();
    assert_eq!(task["tags"], serde_json::json!(["writing"]));
    assert_eq!(task["deadline"], "2031-06-30");
    // Changing only the deadline keeps the Tags.
    let task =
        cli.ok(&["task", "details", &tid, "--revision", "4", "--no-deadline"])["item"].clone();
    assert_eq!(task["tags"], serde_json::json!(["writing"]));
    assert!(task["deadline"].is_null());

    let list = cli.ok(&["list", "create", "--name", "Writing"])["item"].clone();
    let lid = id(&list);
    let task = cli.ok(&["task", "move", &tid, "--revision", "5", "--list", &lid])["item"].clone();
    assert_eq!(task["list_id"].as_str(), Some(lid.as_str()));

    let task = cli.ok(&["task", "remove", &tid, "--revision", "6"])["item"].clone();
    assert_eq!(task["removed"], true);
    assert!(
        cli.ok(&["task", "list"])["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let task = cli.ok(&["task", "restore", &tid, "--revision", "7"])["item"].clone();
    assert_eq!(
        (
            task["removed"].as_bool(),
            task["progress"].as_u64(),
            task["status"].as_str()
        ),
        (Some(false), Some(60), Some("in_progress")),
        "restore keeps details and work status"
    );
    let task = cli.ok(&["task", "move", &tid, "--revision", "8", "--inbox"])["item"].clone();
    assert!(task["list_id"].is_null());
    let task = cli.ok(&[
        "task",
        "change",
        &tid,
        "--revision",
        "9",
        "--status",
        "completed",
    ])["item"]
        .clone();
    assert_eq!(task["progress"], 100);
}

#[test]
fn lists_are_created_renamed_removed_and_restored_through_the_cli() {
    let cli = Cli::trusted();
    let list = cli.ok(&["list", "create", "--name", "Garden"])["item"].clone();
    let lid = id(&list);
    let list = cli.ok(&[
        "list",
        "rename",
        &lid,
        "--revision",
        "1",
        "--name",
        "Vegetable garden",
    ])["item"]
        .clone();
    assert_eq!(list["name"], "Vegetable garden");
    cli.ok(&["task", "create", "--title", "Sow beans", "--list", &lid]);

    // A List holding a Task is removed only with the person's approval in
    // the desktop (owner decision B); restoring it needs none.
    let run = cli.run(&["list", "remove", &lid, "--revision", "2"]);
    assert_eq!(error_code(&run, 5), "needs_approval");
    let asked = cli.ok(&["bulk", "preview", "--action", "remove_list", "--id", &lid]);
    tasknotes_core::tasks::TaskStore::open(&cli.db())
        .unwrap()
        .approve_bulk_request(asked["item"]["request_id"].as_str().unwrap())
        .unwrap();
    assert!(
        cli.ok(&["list", "list"])["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        cli.ok(&["list", "list", "--include-removed"])["items"][0]["removed"],
        true
    );
    // The Task is hidden with its List, not individually Removed.
    assert!(
        cli.ok(&["task", "list"])["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    cli.ok(&["list", "restore", &lid, "--revision", "3"]);
    assert_eq!(cli.ok(&["task", "list"])["items"][0]["title"], "Sow beans");
}

#[test]
fn cli_changes_and_refusals_appear_in_the_audit_log_without_task_text() {
    let cli = Cli::trusted();
    let task = cli.ok(&[
        "task",
        "create",
        "--title",
        "Renew insurance",
        "--description",
        "policy 12345",
    ])["item"]
        .clone();
    let tid = id(&task);
    cli.ok(&[
        "task",
        "update",
        &tid,
        "--revision",
        "1",
        "--title",
        "Renew car insurance",
    ]);
    let stale = cli.run(&[
        "task",
        "update",
        &tid,
        "--revision",
        "1",
        "--title",
        "Stale",
    ]);
    assert_eq!(error_code(&stale, 4), "revision_conflict");

    let audit = cli.ok(&["audit", "list", "--entity", &tid]);
    let rows: Vec<_> = audit["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["interface"].as_str().unwrap().to_owned(),
                e["action"].as_str().unwrap().to_owned(),
                e["outcome"].as_str().unwrap().to_owned(),
                e["error_code"].as_str().map(str::to_owned),
            )
        })
        .collect();
    let row = |a: &str, o: &str, c: Option<&str>| {
        (
            "cli".to_owned(),
            a.to_owned(),
            o.to_owned(),
            c.map(str::to_owned),
        )
    };
    assert_eq!(
        rows,
        vec![
            row("task.update", "rejected", Some("revision_conflict")),
            row("task.update", "applied", None),
            row("task.create", "applied", None),
        ]
    );
    let text = audit.to_string();
    assert!(!text.contains("insurance") && !text.contains("12345") && !text.contains("Stale"));

    let paged = cli.ok(&["audit", "list", "--limit", "2"]);
    let next = paged["next_cursor"].as_str().unwrap();
    assert_eq!(
        cli.ok(&["audit", "list", "--cursor", next])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
