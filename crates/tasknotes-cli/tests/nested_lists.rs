//! Ticket 12: Lists inside Lists through the separate `tasknotes` process.
//! Synthetic data only.

mod common;

use common::Cli;
use serde_json::Value;

fn id(item: &Value) -> String {
    item["id"].as_str().unwrap().to_owned()
}

fn rev(item: &Value) -> String {
    item["revision"].as_i64().unwrap().to_string()
}

fn code(cli: &Cli, args: &[&str]) -> String {
    cli.run(args).json["error"]["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Each List as `name<parent name`, in the order the tree is read.
fn shape(cli: &Cli) -> Vec<String> {
    let items = cli.ok(&["list", "list"])["items"]
        .as_array()
        .unwrap()
        .clone();
    items
        .iter()
        .map(|list| match list["parent_id"].as_str() {
            None => list["name"].as_str().unwrap().to_owned(),
            Some(parent) => {
                let parent = items.iter().find(|p| p["id"] == parent).unwrap();
                format!(
                    "{}<{}",
                    list["name"].as_str().unwrap(),
                    parent["name"].as_str().unwrap()
                )
            }
        })
        .collect()
}

#[test]
fn lists_are_created_inside_lists_moved_and_read_as_a_tree() {
    let cli = Cli::trusted();
    let work = cli.ok(&["list", "create", "--name", "Work"])["item"].clone();
    assert!(work["parent_id"].is_null());
    assert_eq!(work["has_children"], false);
    let reports = cli.ok(&[
        "list",
        "create",
        "--name",
        "Reports",
        "--parent",
        &id(&work),
    ])["item"]
        .clone();
    assert_eq!(reports["parent_id"], id(&work).as_str());
    let home = cli.ok(&["list", "create", "--name", "Home"])["item"].clone();
    assert_eq!(shape(&cli), ["Work", "Reports<Work", "Home"]);
    let listed = cli.ok(&["list", "list"])["items"][0].clone();
    assert_eq!(listed["has_children"], true);

    // Moving: into another List (at the bottom), then back to the top level.
    let moved = cli.ok(&[
        "list",
        "move",
        &id(&reports),
        "--revision",
        &rev(&reports),
        "--parent",
        &id(&home),
    ])["item"]
        .clone();
    assert_eq!(moved["parent_id"], id(&home).as_str());
    assert_eq!(shape(&cli), ["Work", "Home", "Reports<Home"]);
    let moved = cli.ok(&[
        "list",
        "move",
        &id(&reports),
        "--revision",
        &rev(&moved),
        "--top",
    ])["item"]
        .clone();
    assert!(moved["parent_id"].is_null());
    assert_eq!(
        code(
            &cli,
            &["list", "move", &id(&reports), "--revision", &rev(&moved)]
        ),
        "invalid_input",
        "a destination is required"
    );

    // Reordering can name a parent too: one step, like a sidebar drop.
    let placed = cli.ok(&[
        "list",
        "reorder",
        &id(&reports),
        "--revision",
        &rev(&moved),
        "--first",
        "--parent",
        &id(&work),
    ])["item"]
        .clone();
    assert_eq!(placed["parent_id"], id(&work).as_str());
    assert_eq!(shape(&cli), ["Work", "Reports<Work", "Home"]);

    // A List cannot land inside itself.
    let work_now = cli.ok(&["list", "list"])["items"][0].clone();
    assert_eq!(
        code(
            &cli,
            &[
                "list",
                "move",
                &id(&work),
                "--revision",
                &rev(&work_now),
                "--parent",
                &id(&reports)
            ]
        ),
        "invalid_input"
    );
}

#[test]
fn depth_above_five_is_refused() {
    let cli = Cli::trusted();
    let mut parent = cli.ok(&["list", "create", "--name", "Level 1"])["item"].clone();
    for level in 2..=5 {
        parent = cli.ok(&[
            "list",
            "create",
            "--name",
            &format!("Level {level}"),
            "--parent",
            &id(&parent),
        ])["item"]
            .clone();
    }
    assert_eq!(
        code(
            &cli,
            &[
                "list",
                "create",
                "--name",
                "Level 6",
                "--parent",
                &id(&parent)
            ]
        ),
        "invalid_input"
    );
}

#[test]
fn removing_a_list_that_holds_a_list_needs_approval_through_the_cli() {
    let cli = Cli::trusted();
    let work = cli.ok(&["list", "create", "--name", "Work"])["item"].clone();
    cli.ok(&[
        "list",
        "create",
        "--name",
        "Reports",
        "--parent",
        &id(&work),
    ]);
    assert_eq!(
        code(
            &cli,
            &["list", "remove", &id(&work), "--revision", &rev(&work)]
        ),
        "needs_approval"
    );
    assert_eq!(shape(&cli), ["Work", "Reports<Work"], "nothing changed");
}

/// Owner decision Q33: names are unique among siblings only, as for Note
/// Folders, and the CLI names a List by id, so two Lists called Reports in
/// different parents never make a command ambiguous.
#[test]
fn list_names_are_unique_among_siblings_and_lists_are_named_by_id() {
    let cli = Cli::trusted();
    let it = cli.ok(&["list", "create", "--name", "Adamaja IT"])["item"].clone();
    let personal = cli.ok(&["list", "create", "--name", "Personal"])["item"].clone();
    let reports =
        cli.ok(&["list", "create", "--name", "Reports", "--parent", &id(&it)])["item"].clone();
    let other = cli.ok(&[
        "list",
        "create",
        "--name",
        "Reports",
        "--parent",
        &id(&personal),
    ])["item"]
        .clone();
    assert_eq!(
        shape(&cli),
        [
            "Adamaja IT",
            "Reports<Adamaja IT",
            "Personal",
            "Reports<Personal"
        ]
    );

    let refused = cli.run(&["list", "create", "--name", "reports", "--parent", &id(&it)]);
    assert_eq!(refused.json["error"]["code"], "name_conflict");
    assert_eq!(
        refused.json["error"]["message"],
        "A List with this name already exists here"
    );
    assert_eq!(
        code(
            &cli,
            &[
                "list",
                "move",
                &id(&other),
                "--revision",
                &rev(&other),
                "--parent",
                &id(&it)
            ]
        ),
        "name_conflict",
        "a move into a parent that has a Reports is refused"
    );

    // `--list` takes the id: each Reports gets its own Task.
    let task = cli.ok(&[
        "task",
        "create",
        "--title",
        "Synthetic report",
        "--list",
        &id(&reports),
    ])["item"]
        .clone();
    assert_eq!(task["list_id"], id(&reports).as_str());
    assert_eq!(
        code(
            &cli,
            &["task", "create", "--title", "By name", "--list", "Reports"]
        ),
        "not_found",
        "a name is never taken for an id"
    );
}
