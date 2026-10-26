//! Malformed input, cursors and storage failures get stable codes and exit
//! statuses, through the separate process.

mod common;

use common::{Cli, error_code, finish};

#[test]
fn malformed_commands_and_options_are_invalid_input() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Clean the gutters"])["item"].clone();
    let tid = task["id"].as_str().unwrap();
    for args in [
        vec![],
        vec!["task"],
        vec!["task", "explode"],
        vec!["note", "list"],
        vec!["task", "create"],
        vec!["task", "create", "--title"],
        vec!["task", "create", "--title", "   "],
        vec!["task", "create", "--title", "A", "--colour", "red"],
        vec!["task", "create", "--title", "A", "--title", "B"],
        vec!["task", "update", tid, "--title", "No revision"],
        vec!["task", "update", tid, "--revision", "two", "--title", "B"],
        vec!["task", "update", tid, "--revision", "0", "--title", "B"],
        vec![
            "task",
            "change",
            tid,
            "--revision",
            "1",
            "--status",
            "paused",
        ],
        vec![
            "task",
            "change",
            tid,
            "--revision",
            "1",
            "--progress",
            "250",
        ],
        vec![
            "task",
            "details",
            tid,
            "--revision",
            "1",
            "--deadline",
            "2031-02-30",
        ],
        vec!["task", "get"],
        vec!["task", "get", tid, "extra"],
        vec!["task", "list", "--limit", "0"],
        vec!["task", "list", "--limit", "500"],
        vec!["task", "list", "--inbox=yes"],
        vec!["task", "move", tid, "--revision", "1"],
        // A partial edit that names no field.
        vec!["task", "update", tid, "--revision", "1"],
        vec!["task", "details", tid, "--revision", "1"],
    ] {
        let run = cli.run(&args);
        assert_eq!(error_code(&run, 2), "invalid_input", "{args:?}");
        assert!(
            run.json["error"]["message"]
                .as_str()
                .is_some_and(|m| !m.is_empty())
        );
    }
    // Nothing was changed by any of them.
    assert_eq!(cli.ok(&["task", "get", tid])["item"]["revision"], 1);
}

#[test]
fn a_forged_or_foreign_cursor_is_refused() {
    let cli = Cli::trusted();
    for n in 0..3 {
        cli.ok(&[
            "task",
            "create",
            "--title",
            &format!("Synthetic errand {n}"),
        ]);
    }
    let page = cli.ok(&["task", "list", "--limit", "1"]);
    let cursor = page["next_cursor"].as_str().unwrap();
    let first = page["items"][0]["id"].as_str().unwrap().to_owned();
    for args in [
        vec!["task", "list", "--cursor", "not-a-cursor"],
        // A cursor belongs to the query that produced it.
        vec!["task", "list", "--text", "errand", "--cursor", cursor],
        vec!["list", "list", "--cursor", "v2|x|y|active"],
        vec!["audit", "list", "--cursor", "a1|-4"],
        vec!["task", "history", &first, "--cursor", "h1|x"],
    ] {
        assert_eq!(error_code(&cli.run(&args), 2), "invalid_cursor", "{args:?}");
    }
}

#[test]
fn rule_refusals_use_their_own_codes() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Fix the bike"])["item"].clone();
    let tid = task["id"].as_str().unwrap();
    cli.ok(&["task", "remove", tid, "--revision", "1"]);
    let run = cli.run(&[
        "task",
        "update",
        tid,
        "--revision",
        "2",
        "--title",
        "Fix it",
    ]);
    assert_eq!(error_code(&run, 5), "removed");
    let run = cli.run(&["task", "remove", tid, "--revision", "2"]);
    assert_eq!(error_code(&run, 5), "unchanged");

    cli.ok(&["list", "create", "--name", "Chores"]);
    let run = cli.run(&["list", "create", "--name", "chores"]);
    assert_eq!(error_code(&run, 5), "name_conflict");

    let ready = cli.ok(&["task", "create", "--title", "Plan a trip"])["item"].clone();
    let rid = ready["id"].as_str().unwrap();
    cli.ok(&["task", "change", rid, "--revision", "1", "--progress", "20"]);
    let run = cli.run(&[
        "task",
        "change",
        rid,
        "--revision",
        "2",
        "--status",
        "ready",
    ]);
    assert_eq!(error_code(&run, 5), "invalid_transition");
}

#[test]
fn a_newer_database_is_refused_and_left_untouched() {
    let cli = Cli::trusted();
    cli.ok(&["task", "create", "--title", "Sort photos"]);
    rusqlite::Connection::open(cli.db())
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    let run = cli.run(&["task", "list"]);
    assert_eq!(error_code(&run, 7), "unsupported_schema");
    let version: i64 = rusqlite::Connection::open(cli.db())
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 99);
}

#[test]
fn a_write_that_cannot_get_the_database_lock_fails_safely() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Call the plumber"])["item"].clone();
    let tid = task["id"].as_str().unwrap();
    let holder = rusqlite::Connection::open(cli.db()).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE;").unwrap();
    // The core waits its busy timeout (5 s), then gives up.
    let run = cli.run(&[
        "task",
        "update",
        tid,
        "--revision",
        "1",
        "--title",
        "Call Sam",
    ]);
    assert_eq!(error_code(&run, 7), "storage_unavailable");
    holder.execute_batch("ROLLBACK;").unwrap();
    let after = cli.ok(&["task", "get", tid]);
    assert_eq!(
        (
            after["item"]["title"].as_str(),
            after["item"]["revision"].as_i64()
        ),
        (Some("Call the plumber"), Some(1))
    );
}

#[cfg(unix)]
#[test]
fn an_argument_that_is_not_utf8_is_invalid_input() {
    use std::os::unix::ffi::OsStrExt;
    let cli = Cli::trusted();
    let mut command = cli.command(&["task", "create", "--title"]);
    command.arg(std::ffi::OsStr::from_bytes(b"caf\xe9"));
    let run = finish(command.output().unwrap());
    assert_eq!(error_code(&run, 2), "invalid_input");
}

#[test]
fn a_refusal_whose_audit_entry_cannot_be_written_says_so_in_the_envelope() {
    let cli = Cli::trusted();
    let task = cli.ok(&["task", "create", "--title", "Return the parcel"])["item"].clone();
    let tid = task["id"].as_str().unwrap();
    cli.ok(&[
        "task",
        "update",
        tid,
        "--revision",
        "1",
        "--title",
        "Return it",
    ]);
    let stale = ["task", "update", tid, "--revision", "1", "--title", "Late"];

    let recorded = cli.run(&stale);
    assert_eq!(error_code(&recorded, 4), "revision_conflict");
    assert!(recorded.json["error"].get("audit_recorded").is_none());

    // Stand-in for a storage failure that outlasts the busy timeout.
    rusqlite::Connection::open(cli.db())
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_rejections BEFORE INSERT ON audit_entries
             WHEN NEW.outcome = 'rejected'
             BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
        )
        .unwrap();
    let unrecorded = cli.run(&stale);
    assert_eq!(error_code(&unrecorded, 4), "revision_conflict");
    assert_eq!(unrecorded.json["error"]["audit_recorded"], false);
}
