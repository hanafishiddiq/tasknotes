//! Ticket 13 through the CLI: backups, terminal-confirmed restore and the
//! AI-facing export. Separate processes against temporary data only.

mod common;

use std::ffi::OsString;

use common::{Cli, error_code};
use serde_json::Value;
use tasknotes_cli::access::Environment;
use tasknotes_core::notes::CreateNote;
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::{CreateTask, TaskStore};

fn item(json: &Value) -> &Value {
    &json["item"]
}

/// Run in this process as if at an interactive terminal where the person
/// answers `answer`; returns the prompt shown and the outcome.
fn at_terminal(cli: &Cli, args: &[&str], answer: &str) -> (Vec<String>, tasknotes_cli::Outcome) {
    let mut argv: Vec<OsString> = vec!["--data-dir".into(), cli.data_dir().into()];
    argv.extend(args.iter().map(OsString::from));
    let env = Environment {
        stdin_is_terminal: true,
        ..Environment::default()
    };
    let mut prompts = Vec::new();
    let outcome = tasknotes_cli::run(&argv, &env, |prompt| {
        prompts.push(prompt.to_owned());
        Ok(format!("{answer}\n"))
    });
    (prompts, outcome)
}

#[test]
fn any_data_command_takes_the_days_backup_and_backups_can_be_listed_and_made() {
    let cli = Cli::trusted();
    cli.ok(&["task", "create", "--title", "Synthetic first task"]);
    let listed = cli.ok(&["backup", "list"]);
    let items = listed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["kind"], "daily");
    // Idempotent for the day.
    cli.ok(&["task", "list"]);
    assert_eq!(
        cli.ok(&["backup", "list"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let made = cli.ok(&["backup", "create"]);
    let backup = item(&made);
    assert_eq!(backup["kind"], "manual");
    for key in ["id", "created_at", "schema_version", "size_bytes", "sha256"] {
        assert!(!backup[key].is_null(), "{key} in {backup}");
    }
    // Manual backups are their own bucket: the day's Daily one stays.
    let after = cli.ok(&["backup", "list"]);
    assert_eq!(after["items"][0]["id"], backup["id"]);
    assert_eq!(after["items"][1]["kind"], "daily");
    assert!(after.get("warnings").is_none(), "{after}");
}

/// Review finding: a failed automatic backup was silently ignored. The
/// command still runs (a backup must not block work) but says so in the
/// envelope; stderr stays empty per the CLI contract.
#[test]
fn a_failed_automatic_backup_is_reported_in_the_envelope() {
    let cli = Cli::trusted();
    // Something that is not a directory where the backups belong.
    std::fs::write(cli.data_dir().join("backups"), b"synthetic").unwrap();
    let run = cli.run(&["task", "create", "--title", "Synthetic task"]);
    assert_eq!(run.code, 0, "{}", run.json);
    assert_eq!(item(&run.json)["title"], "Synthetic task");
    let warnings = run.json["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{}", run.json);
    assert_eq!(warnings[0]["code"], "backup_failed");
    assert!(warnings[0]["message"].as_str().unwrap().contains("backup"));
    assert!(run.stderr.is_empty(), "{}", run.stderr);
    // A failed command carries the warning too.
    let failed = cli.run(&["task", "get", "00000000-0000-0000-0000-000000000000"]);
    assert_eq!(error_code(&failed, 3), "not_found");
    assert_eq!(failed.json["warnings"][0]["code"], "backup_failed");
}

#[test]
fn an_untrusted_cli_can_neither_back_up_nor_export() {
    let cli = Cli::untrusted();
    for args in [
        &["backup", "list"][..],
        &["backup", "create"],
        &["backup", "restore", "x"],
        &["export", "json", "--out", "x.json"],
    ] {
        assert_eq!(error_code(&cli.run(args), 6), "not_trusted", "{args:?}");
    }
    assert!(!cli.db().exists());
}

#[test]
fn restore_needs_the_person_at_a_terminal() {
    let cli = Cli::trusted();
    cli.ok(&["task", "create", "--title", "Synthetic task"]);
    let id = item(&cli.ok(&["backup", "create"]))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // An AI client runs without a terminal.
    let run = cli.run(&["backup", "restore", &id]);
    assert_eq!(error_code(&run, 6), "not_trusted");

    let (prompts, outcome) = at_terminal(&cli, &["backup", "restore", &id], "no");
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].contains("restore"), "{prompts:?}");
    assert_eq!(outcome.exit_code, 6, "{}", outcome.body);
    assert_eq!(
        cli.ok(&["task", "list"])["items"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn a_confirmed_restore_brings_back_the_backup_and_stops_stale_writers() {
    let cli = Cli::trusted();
    let kept = cli.ok(&["task", "create", "--title", "Synthetic kept task"]);
    let backup = item(&cli.ok(&["backup", "create"]))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cli.ok(&["task", "create", "--title", "Synthetic later task"]);
    // The desktop window is open the whole time.
    let window = TaskStore::open(&cli.db()).unwrap();

    let (_, outcome) = at_terminal(&cli, &["backup", "restore", &backup], "restore");
    assert_eq!(outcome.exit_code, 0, "{}", outcome.body);
    assert_eq!(outcome.body["item"]["restored"]["id"], backup.as_str());
    assert_eq!(outcome.body["item"]["pre_restore"]["kind"], "pre_restore");

    let titles: Vec<Value> = cli.ok(&["task", "list"])["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].clone())
        .collect();
    assert_eq!(titles, ["Synthetic kept task"]);

    // The window opened before must reload before it can write again.
    let stale = window.create_task(CreateTask {
        title: "Synthetic stale window write".into(),
        description: String::new(),
    });
    assert_eq!(stale.unwrap_err().code(), "database_restored");
    let task_id = kept["item"]["id"].as_str().unwrap();
    cli.ok(&["task", "get", task_id]);

    // An id that names a path is no backup.
    let (_, bad) = at_terminal(&cli, &["backup", "restore", "../../tasknotes"], "restore");
    assert_eq!(bad.exit_code, 3, "{}", bad.body);
    assert_eq!(bad.body["error"]["code"], "not_found");
}

#[test]
fn a_damaged_backup_is_refused_through_the_cli() {
    let cli = Cli::trusted();
    cli.ok(&["task", "create", "--title", "Synthetic task"]);
    let id = item(&cli.ok(&["backup", "create"]))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let snapshot = cli.data_dir().join(format!("backups/{id}.sqlite3"));
    let bytes = std::fs::read(&snapshot).unwrap();
    std::fs::write(&snapshot, &bytes[..bytes.len() / 3]).unwrap();
    let (_, outcome) = at_terminal(&cli, &["backup", "restore", &id], "restore");
    assert_eq!(outcome.exit_code, 5, "{}", outcome.body);
    assert_eq!(outcome.body["error"]["code"], "invalid_backup");
    assert_eq!(
        cli.ok(&["task", "list"])["items"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn the_cli_export_is_ai_facing_and_leaves_private_notes_out() {
    let cli = Cli::trusted();
    cli.ok(&["task", "create", "--title", "Synthetic exported task"]);
    let desktop = TaskStore::open(&cli.db()).unwrap();
    let private = desktop
        .create_note(CreateNote {
            markdown: "# Synthetic private journal".into(),
            folder_id: None,
        })
        .unwrap();
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    cli.ok(&["note", "create", "--markdown", "# Synthetic shared note"]);

    let json_out = cli.data_dir().join("export.json");
    let done = cli.ok(&["export", "json", "--out", json_out.to_str().unwrap()]);
    assert_eq!(item(&done)["notes"], 1);
    assert_eq!(item(&done)["private_notes"], 0);
    let text = std::fs::read_to_string(&json_out).unwrap();
    assert!(text.contains("Synthetic shared note"));
    assert!(!text.contains("Synthetic private journal"));
    assert!(!text.contains(&private.id));

    let md_out = cli.data_dir().join("export-md");
    cli.ok(&["export", "markdown", "--out", md_out.to_str().unwrap()]);
    assert!(md_out.join("Notes/Synthetic shared note.md").is_file());
    assert!(!md_out.join("Notes/Synthetic private journal.md").exists());

    // There is no way to ask for private Notes here, and no overwriting.
    let asked = cli.run(&[
        "export",
        "json",
        "--out",
        "other.json",
        "--include-private-notes",
    ]);
    assert_eq!(error_code(&asked, 2), "invalid_input");
    let again = cli.run(&["export", "json", "--out", json_out.to_str().unwrap()]);
    assert_eq!(error_code(&again, 2), "invalid_input");
    let unknown = cli.run(&["export", "pdf", "--out", "x.pdf"]);
    assert_eq!(error_code(&unknown, 2), "invalid_input");
}
