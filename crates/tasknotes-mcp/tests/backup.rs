//! Ticket 13 meets Ticket 12: backups run while any Tasknotes process is
//! active, the MCP server included, and no client can write over restored
//! data with what it read before. A failed backup never fails a tool.
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

#[test]
fn a_backup_follows_soon_after_the_first_write_creates_the_database() {
    let data = Data::trusted();
    let mut server = data.server();
    server.ok("task_create", json!({"title": "Synthetic"}));
    assert_eq!(wait_for_backups(&data).len(), 1);
}

#[test]
fn a_backup_follows_soon_after_the_person_grants_access() {
    let data = Data::untrusted();
    data.desktop();
    let mut server = data.server();
    assert!(server.silent_for(Duration::from_millis(200)));
    assert!(data.desktop().list_backups().unwrap().is_empty());
    data.write_access(r#"{"schema_version":1,"mcp":true}"#);
    assert_eq!(wait_for_backups(&data).len(), 1);
}

/// Restoring rewinds revisions, so a revision a client read before can match
/// again: the Restore Epoch has to stop it, across calls of one server.
#[test]
fn a_write_from_before_a_restore_is_refused_once_and_the_persons_edit_survives() {
    let data = Data::trusted();
    let desktop = data.desktop();
    let mut server = data.server();
    let task = server.ok("task_create", json!({"title": "Synthetic original"}));
    let id = task["item"]["id"].as_str().unwrap().to_owned();
    let backup = desktop.create_backup().unwrap().backup;
    server.ok(
        "task_update",
        json!({"id": id, "revision": 1, "title": "Synthetic AI saw this"}),
    );
    desktop.restore_backup(&backup.id).unwrap();
    // The person carries on in the restored data: revision 2 again.
    desktop
        .patch_task(tasknotes_core::tasks::PatchTask {
            id: id.clone(),
            expected_revision: 1,
            title: Some("Synthetic persons edit".into()),
            description: None,
        })
        .unwrap();

    server.refused(
        "task_update",
        json!({"id": id, "revision": 2, "title": "Synthetic stale write"}),
        "database_restored",
    );
    let title = |d: &tasknotes_core::tasks::TaskStore| d.get_task(&id).unwrap().unwrap().title;
    assert_eq!(title(&desktop), "Synthetic persons edit");

    // Told once: the client reads again and carries on.
    let read = server.ok("task_get", json!({"id": id}));
    assert_eq!(read["item"]["title"], "Synthetic persons edit");
    server.ok(
        "task_update",
        json!({"id": id, "revision": read["item"]["revision"], "title": "Synthetic fresh write"}),
    );
    assert_eq!(title(&desktop), "Synthetic fresh write");
}

#[test]
fn a_restore_does_not_open_a_private_note_to_a_running_server() {
    let data = Data::trusted();
    let desktop = data.desktop();
    let note = desktop
        .create_note(tasknotes_core::notes::CreateNote {
            markdown: "# Synthetic private later".into(),
            folder_id: None,
        })
        .unwrap();
    let backup = desktop.create_backup().unwrap().backup;
    desktop
        .set_note_exclude_from_ai(tasknotes_core::privacy::SetNoteExcludeFromAi {
            id: note.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let mut server = data.server();
    server.refused("note_get", json!({"id": note.id}), "not_found");

    desktop.restore_backup(&backup.id).unwrap();

    server.refused("note_get", json!({"id": note.id}), "database_restored");
    server.refused("note_get", json!({"id": note.id}), "not_found");
}

/// What a running restore looks like from outside: its lock held and the live
/// epoch a fence token. Every call is refused and nothing is written.
#[test]
fn every_call_is_refused_while_a_restore_runs_and_works_again_after() {
    let data = Data::trusted();
    let desktop = data.desktop();
    let task = desktop
        .create_task(tasknotes_core::tasks::CreateTask {
            title: "Synthetic".into(),
            description: String::new(),
        })
        .unwrap();
    drop(desktop);
    std::fs::create_dir_all(data.path().join("backups")).unwrap();
    let lock = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.path().join("backups/.restore.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    rusqlite::Connection::open(data.db())
        .unwrap()
        .execute(
            "UPDATE database_epoch SET epoch = 'restoring-synthetic'",
            [],
        )
        .unwrap();
    let mut server = data.server();
    for (tool, arguments) in [
        ("task_list", json!({})),
        ("task_get", json!({"id": task.id})),
        ("task_create", json!({"title": "Synthetic during restore"})),
    ] {
        server.refused(tool, arguments, "storage_unavailable");
    }
    drop(lock);
    let listed = server.ok("task_list", json!({}));
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
}
