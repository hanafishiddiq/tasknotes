//! Ticket 01: icons and colours for Lists and Note Folders through the
//! separate `tasknotes` process. Synthetic data only.

mod common;

use common::{Cli, error_code};
use serde_json::Value;

fn id(value: &Value) -> &str {
    value["id"].as_str().unwrap()
}

fn code(cli: &Cli, args: &[&str]) -> String {
    cli.run(args).json["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn lists_read_set_and_clear_an_icon_and_a_colour_with_revision_checks() {
    let cli = Cli::trusted();
    let made = cli.ok(&["list", "create", "--name", "🏠 Synthetic home"])["item"].clone();
    assert!(made["icon"].is_null() && made["color"].is_null());

    let styled = cli.ok(&[
        "list",
        "style",
        id(&made),
        "--revision",
        "1",
        "--icon",
        "house",
        "--color",
        "green",
    ])["item"]
        .clone();
    assert_eq!(styled["icon"], "house");
    assert_eq!(styled["color"], "green");
    assert_eq!(styled["revision"], 2);
    assert_eq!(styled["name"], "🏠 Synthetic home");
    let listed = cli.ok(&["list", "list"])["items"][0].clone();
    assert_eq!(listed, styled);

    // A flag left out keeps its value; `none` clears it.
    let cleared = cli.ok(&[
        "list",
        "style",
        id(&made),
        "--revision",
        "2",
        "--icon",
        "none",
    ])["item"]
        .clone();
    assert!(cleared["icon"].is_null());
    assert_eq!(cleared["color"], "green");

    let stale = cli.run(&[
        "list",
        "style",
        id(&made),
        "--revision",
        "2",
        "--color",
        "red",
    ]);
    assert_eq!(error_code(&stale, 4), "revision_conflict");
    assert_eq!(
        code(
            &cli,
            &[
                "list",
                "style",
                id(&made),
                "--revision",
                "3",
                "--icon",
                "nope"
            ]
        ),
        "invalid_input"
    );
    assert_eq!(
        code(
            &cli,
            &[
                "list",
                "style",
                id(&made),
                "--revision",
                "3",
                "--color",
                "magenta"
            ]
        ),
        "invalid_input"
    );
    assert_eq!(
        code(&cli, &["list", "style", id(&made), "--revision", "3"]),
        "invalid_input"
    );
    assert_eq!(
        code(
            &cli,
            &[
                "list",
                "style",
                id(&made),
                "--revision",
                "3",
                "--color",
                "green"
            ]
        ),
        "unchanged"
    );

    let audit = cli.ok(&["audit", "list", "--entity", id(&made)]);
    let actions: Vec<&str> = audit["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        ["list.style", "list.style", "list.style", "list.create"]
    );
}

#[test]
fn folders_read_set_and_clear_an_icon_and_a_colour() {
    let cli = Cli::trusted();
    let made = cli.ok(&["folder", "create", "--name", "Synthetic recipes"])["item"].clone();
    assert!(made["icon"].is_null() && made["color"].is_null());

    let styled = cli.ok(&[
        "folder",
        "style",
        id(&made),
        "--revision",
        "1",
        "--icon",
        "chef-hat",
        "--color",
        "pink",
    ])["item"]
        .clone();
    assert_eq!(styled["icon"], "chef-hat");
    assert_eq!(styled["color"], "pink");
    assert_eq!(styled["revision"], 2);
    assert_eq!(styled["path"], "Synthetic recipes");
    assert_eq!(cli.ok(&["folder", "get", id(&made)])["item"], styled);

    let plain = cli.ok(&[
        "folder",
        "style",
        id(&made),
        "--revision",
        "2",
        "--icon",
        "none",
        "--color",
        "none",
    ])["item"]
        .clone();
    assert!(plain["icon"].is_null() && plain["color"].is_null());
    let stale = cli.run(&[
        "folder",
        "style",
        id(&made),
        "--revision",
        "2",
        "--icon",
        "star",
    ]);
    assert_eq!(error_code(&stale, 4), "revision_conflict");
}

#[test]
fn style_options_lists_the_colours_and_icons_the_core_accepts() {
    let cli = Cli::trusted();
    let options = cli.ok(&["style", "options"])["item"].clone();
    assert_eq!(options["colors"].as_array().unwrap().len(), 12);
    assert_eq!(options["colors"][0], "red");
    let icons = options["icons"].as_array().unwrap();
    assert!(icons.len() >= 150);
    assert!(icons.iter().any(|icon| icon == "folder"));
}
