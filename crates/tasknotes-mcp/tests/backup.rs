//! Ticket 13 meets Ticket 12: backups run while any Tasknotes process is
//! active, the MCP server included. A failed backup never fails a tool.
//! Synthetic data only.

mod common;

use std::time::{Duration, Instant};

use common::Data;
use serde_json::json;

/// The Backups a person would see, waiting for the server to take one.
fn wait_for_backups(data: &Data) -> Vec<tasknotes_core::backup::Backup> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let backups = data.desktop().list_backups().unwrap();
        if !backups.is_empty() {
            return backups;
        }
        assert!(Instant::now() < deadline, "no backup was taken");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_trusted_server_takes_the_days_backup_without_being_called() {
    let data = Data::trusted();
    data.desktop();
    let server = data.server();
    let backups = wait_for_backups(&data);
    assert_eq!(backups.len(), 1, "{backups:?}");
    assert!(server.stderr().is_empty(), "stderr: {}", server.stderr());
}

#[test]
fn an_untrusted_server_backs_up_nothing_and_opens_nothing() {
    let data = Data::untrusted();
    let mut server = data.server();
    assert!(server.silent_for(Duration::from_millis(500)));
    assert!(!data.db().exists(), "the database was opened");
    assert!(!data.path().join("backups").exists(), "a backup was taken");
    assert!(server.stderr().is_empty(), "stderr: {}", server.stderr());
}

#[test]
fn a_server_opens_no_database_that_does_not_exist_yet() {
    let data = Data::trusted();
    let mut server = data.server();
    assert!(server.silent_for(Duration::from_millis(500)));
    assert!(!data.db().exists(), "the database was created");
    assert!(server.stderr().is_empty(), "stderr: {}", server.stderr());
}

#[test]
fn a_failed_backup_never_fails_a_tool() {
    let data = Data::trusted();
    // A file where the backups folder belongs: every backup fails.
    data.desktop();
    std::fs::write(data.path().join("backups"), b"not a folder").unwrap();
    let mut server = data.server();
    let task = server.ok("task_create", json!({"title": "Synthetic"}));
    assert_eq!(task["item"]["title"], "Synthetic");
    let deadline = Instant::now() + Duration::from_secs(20);
    while server.stderr().is_empty() {
        assert!(Instant::now() < deadline, "the failure was not reported");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        server.stderr().contains("backup"),
        "stderr: {}",
        server.stderr()
    );
}
