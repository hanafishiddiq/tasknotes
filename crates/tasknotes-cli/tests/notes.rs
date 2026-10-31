//! Adapter contract (Ticket 11): Notes, folders and links through the
//! separate `tasknotes` process. Synthetic data only.

mod common;

use common::{Cli, error_code};
use serde_json::Value;

fn create(cli: &Cli, markdown: &str) -> Value {
    cli.ok(&["note", "create", "--markdown", markdown])["item"].clone()
}

fn id(value: &Value) -> &str {
    value["id"].as_str().unwrap()
}

#[test]
fn a_note_created_by_the_cli_is_readable_editable_and_listed_with_revisions() {
    let cli = Cli::trusted();
    let created = create(&cli, "# Synthetic shopping list\n\n- Oats\n- Pears");
    assert_eq!(created["title"], "Synthetic shopping list");
    assert_eq!(
        created["markdown"],
        "# Synthetic shopping list\n\n- Oats\n- Pears"
    );
    assert_eq!(created["revision"], 1);
    assert_eq!(created["removed"], false);
    assert!(created["folder_id"].is_null() && created["folder_path"].is_null());
    assert!(
        created.get("exclude_from_ai").is_none(),
        "AI output never mentions the flag"
    );

    let fetched = cli.ok(&["note", "get", id(&created)]);
    assert_eq!(fetched["item"], created);

    let updated = cli.ok(&[
        "note",
        "update",
        id(&created),
        "--revision",
        "1",
        "--markdown",
        "# Synthetic shopping list\n\n- Oats",
    ])["item"]
        .clone();
    assert_eq!(updated["revision"], 2);

    let stale = cli.run(&[
        "note",
        "update",
        id(&created),
        "--revision",
        "1",
        "--markdown",
        "# Overwrite",
    ]);
    assert_eq!(error_code(&stale, 4), "revision_conflict");

    let page = cli.ok(&["note", "list", "--text", "oats"]);
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], created["id"]);
    assert_eq!(items[0]["revision"], 2);
    assert!(items[0]["snippet"].as_str().unwrap().contains("Oats"));
    assert!(page["next_cursor"].is_null());
}

#[test]
fn notes_page_fifty_by_default_and_follow_next_cursor() {
    let cli = Cli::trusted();
    {
        let store = tasknotes_core::tasks::TaskStore::open(&cli.db()).unwrap();
        for n in 0..53 {
            store
                .create_note(tasknotes_core::notes::CreateNote {
                    markdown: format!("# Synthetic note {n:02}"),
                    folder_id: None,
                })
                .unwrap();
        }
    }
    let first = cli.ok(&["note", "list"]);
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = cli.ok(&["note", "list", "--cursor", cursor]);
    assert_eq!(second["items"].as_array().unwrap().len(), 3);
    assert!(second["next_cursor"].is_null());
}

#[test]
fn folders_show_their_full_path_and_id_on_notes_and_in_listings() {
    let cli = Cli::trusted();
    let work = cli.ok(&["folder", "create", "--name", "Work"])["item"].clone();
    let client = cli.ok(&[
        "folder",
        "create",
        "--name",
        "Client A",
        "--parent",
        id(&work),
    ])["item"]
        .clone();
    assert_eq!(client["path"], "Work/Client A");
    assert_eq!(client["parent_id"], work["id"]);
    assert_eq!(client["note_count"], 0);

    let note = cli.ok(&[
        "note",
        "create",
        "--markdown",
        "# Synthetic brief",
        "--folder",
        id(&client),
    ])["item"]
        .clone();
    assert_eq!(note["folder_id"], client["id"]);
    assert_eq!(note["folder_path"], "Work/Client A");

    let listed = cli.ok(&["folder", "list", "--parent", id(&work)]);
    assert_eq!(listed["items"][0]["note_count"], 1);
    assert_eq!(listed["items"][0]["path"], "Work/Client A");
    let in_folder = cli.ok(&["note", "list", "--folder", id(&client)]);
    assert_eq!(in_folder["items"][0]["folder_path"], "Work/Client A");

    let moved =
        cli.ok(&["note", "move", id(&note), "--revision", "1", "--no-folder"])["item"].clone();
    assert!(moved["folder_path"].is_null());
    let renamed = cli.ok(&[
        "folder",
        "rename",
        id(&client),
        "--revision",
        "1",
        "--name",
        "Client B",
    ])["item"]
        .clone();
    assert_eq!(renamed["path"], "Work/Client B");
    let top = cli.ok(&["folder", "move", id(&client), "--revision", "2", "--top"])["item"].clone();
    assert_eq!(top["path"], "Client B");
    let gone = cli.ok(&["folder", "remove", id(&client), "--revision", "3"])["item"].clone();
    assert_eq!(gone["removed"], true);
    let back = cli.ok(&["folder", "restore", id(&client), "--revision", "4"])["item"].clone();
    assert_eq!(back["removed"], false);
}

#[test]
fn links_note_removal_and_restore_work_through_the_cli() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Synthetic chore"])["item"].clone();
    let note = create(&cli, "# Synthetic steps");
    let link =
        cli.ok(&["link", "create", "--task", id(&task), "--note", id(&note)])["item"].clone();
    assert_eq!(link["task_id"], task["id"]);
    assert_eq!(link["note_id"], note["id"]);

    let notes = cli.ok(&["task", "notes", id(&task)]);
    assert_eq!(notes["items"][0]["note_id"], note["id"]);
    assert_eq!(notes["items"][0]["title"], "Synthetic steps");
    let tasks = cli.ok(&["note", "tasks", id(&note)]);
    assert_eq!(tasks["items"][0]["task_id"], task["id"]);
    assert_eq!(tasks["items"][0]["status"], "ready");

    let unlinked = cli.ok(&["link", "remove", id(&link)])["item"].clone();
    assert!(unlinked["unlinked_at"].is_number());
    let relinked = cli.ok(&["link", "restore", id(&link)])["item"].clone();
    assert!(relinked["unlinked_at"].is_null());

    let removed = cli.ok(&["note", "remove", id(&note), "--revision", "1"])["item"].clone();
    assert_eq!(removed["removed"], true);
    let in_removed = cli.ok(&["removed", "list"]);
    assert_eq!(in_removed["items"][0]["kind"], "note");
    assert_eq!(in_removed["items"][0]["id"], note["id"]);
    let restored = cli.ok(&["note", "restore", id(&note), "--revision", "2"])["item"].clone();
    assert_eq!(restored["removed"], false);

    let audit = cli.ok(&["audit", "list", "--entity", id(&note)]);
    let actions: Vec<&str> = audit["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(actions, ["note.restore", "note.remove", "note.create"]);
}

#[test]
fn lists_and_folders_reorder_through_the_cli() {
    let cli = Cli::trusted();
    let names = |page: Value| -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let a = cli.ok(&["list", "create", "--name", "Synthetic A"])["item"].clone();
    let b = cli.ok(&["list", "create", "--name", "Synthetic B"])["item"].clone();
    let moved = cli.ok(&["list", "reorder", id(&b), "--revision", "1", "--first"])["item"].clone();
    assert_eq!(moved["revision"], 2);
    assert_eq!(
        names(cli.ok(&["list", "list"])),
        ["Synthetic B", "Synthetic A"]
    );
    let stale = cli.run(&[
        "list",
        "reorder",
        id(&b),
        "--revision",
        "1",
        "--after",
        id(&a),
    ]);
    assert_eq!(error_code(&stale, 4), "revision_conflict");

    let work = cli.ok(&["folder", "create", "--name", "Synthetic Work"])["item"].clone();
    let home = cli.ok(&["folder", "create", "--name", "Synthetic Home"])["item"].clone();
    let inner = cli.ok(&[
        "folder",
        "create",
        "--name",
        "Synthetic Inner",
        "--parent",
        id(&work),
    ])["item"]
        .clone();
    cli.ok(&["folder", "reorder", id(&home), "--revision", "1", "--first"]);
    assert_eq!(
        names(cli.ok(&["folder", "list"])),
        ["Synthetic Home", "Synthetic Work"]
    );
    let nested = cli.ok(&[
        "folder",
        "reorder",
        id(&home),
        "--revision",
        "2",
        "--parent",
        id(&work),
        "--after",
        id(&inner),
    ])["item"]
        .clone();
    assert_eq!(nested["parent_id"], work["id"]);
    assert_eq!(
        names(cli.ok(&["folder", "list", "--parent", id(&work)])),
        ["Synthetic Inner", "Synthetic Home"]
    );
    let refused = cli.run(&[
        "folder",
        "reorder",
        id(&work),
        "--revision",
        "1",
        "--parent",
        id(&inner),
        "--first",
    ]);
    assert_eq!(refused.json["error"]["code"], "invalid_input");
}
