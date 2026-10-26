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
