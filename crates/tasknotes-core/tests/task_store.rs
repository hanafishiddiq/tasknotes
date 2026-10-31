use std::collections::HashSet;

use rusqlite::Connection;
use tasknotes_core::tasks::{CreateTask, Priority, TaskStore, UpdateTask, WorkStatus};

fn new_store(dir: &tempfile::TempDir) -> TaskStore {
    TaskStore::open(&dir.path().join("tasknotes.db")).expect("open local SQLite")
}

#[test]
fn user_creates_edits_and_reopens_a_task_without_duplication() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(&dir);

    let created = store
        .create_task(CreateTask {
            title: "  Finish the proposal  ".into(),
            description: "Outline three sections".into(),
        })
        .unwrap();

    assert!(!created.id.is_empty());
    assert_eq!(created.title, "Finish the proposal");
    assert_eq!(created.description, "Outline three sections");
    assert_eq!(created.status, WorkStatus::Ready);
    assert_eq!(created.priority, Priority::Medium);
    assert_eq!(created.progress, 0);
    assert_eq!(created.revision, 1);

    let edited = store
        .update_task(UpdateTask {
            id: created.id.clone(),
            title: "Finish the proposal".into(),
            description: "The final outline and references".into(),
            expected_revision: created.revision,
        })
        .unwrap();
    assert_eq!(edited.revision, 2);
    assert_eq!(edited.description, "The final outline and references");

    drop(store);
    let reopened = new_store(&dir);
    assert_eq!(reopened.get_task(&created.id).unwrap().unwrap(), edited);
    let page = reopened.list_tasks(50, None).unwrap();
    assert_eq!(page.items, vec![edited]);
    assert!(page.next_cursor.is_none());
}

#[test]
fn invalid_creates_and_edits_do_not_leave_partial_records() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(&dir);
    assert!(
        store
            .create_task(CreateTask {
                title: "  ".into(),
                description: "Not stored".into()
            })
            .is_err()
    );
    assert!(store.list_tasks(50, None).unwrap().items.is_empty());

    let created = store
        .create_task(CreateTask {
            title: "Original".into(),
            description: "Keeps its content".into(),
        })
        .unwrap();

    assert!(
        store
            .update_task(UpdateTask {
                id: created.id.clone(),
                title: "".into(),
                description: "Must not overwrite".into(),
                expected_revision: 1,
            })
            .is_err()
    );
    assert_eq!(store.get_task(&created.id).unwrap().unwrap(), created);
}

#[test]
fn page_cursors_limit_results_and_never_repeat_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(&dir);
    for i in 0..5 {
        store
            .create_task(CreateTask {
                title: format!("Task {i}"),
                description: "".into(),
            })
            .unwrap();
    }
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    let mut sizes = vec![];
    loop {
        let page = store.list_tasks(2, cursor.as_deref()).unwrap();
        assert!(page.items.len() <= 2);
        sizes.push(page.items.len());
        for task in page.items {
            assert!(seen.insert(task.id), "no duplicated task between pages");
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(sizes, vec![2, 2, 1]);
    assert_eq!(seen.len(), 5);
    assert!(store.list_tasks(0, None).is_err());
    assert!(store.list_tasks(101, None).is_err());
    assert!(store.list_tasks(2, Some("not-a-real-cursor")).is_err());
}

#[test]
fn stale_edit_is_rejected_instead_of_overwriting_recent_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = new_store(&dir);
    let first = store
        .create_task(CreateTask {
            title: "Shared task".into(),
            description: "Before".into(),
        })
        .unwrap();
    let winning = store
        .update_task(UpdateTask {
            id: first.id.clone(),
            title: first.title.clone(),
            description: "Newer edit".into(),
            expected_revision: first.revision,
        })
        .unwrap();
    let stale = store.update_task(UpdateTask {
        id: first.id.clone(),
        title: first.title.clone(),
        description: "Stale edit".into(),
        expected_revision: first.revision,
    });
    assert!(stale.unwrap_err().to_string().contains("conflict"));
    assert_eq!(store.get_task(&first.id).unwrap().unwrap(), winning);
}

#[test]
fn newer_incompatible_schema_is_refused_without_mutating_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasknotes.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    let error = TaskStore::open(&path).unwrap_err();
    assert!(error.to_string().contains("schema"));
    let connection = rusqlite::Connection::open(&path).unwrap();
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 99);
}

/// Several processes may open the same database at the same moment: the app
/// and an MCP client started together, on a fresh folder or after an update.
/// One upgrades it; the others wait and find it done. Threads stand in for
/// processes: each open takes its own file lock and its own connection.
#[test]
fn stores_opening_at_the_same_moment_all_succeed() {
    for older in [
        None,
        Some(
            "DROP TABLE retired_revisions;
                 DROP TABLE bulk_requests;
                 ALTER TABLE audit_entries DROP COLUMN requested_by;
                 PRAGMA user_version = 13;",
        ),
    ] {
        for _round in 0..10 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("tasknotes.db");
            if let Some(sql) = older {
                drop(TaskStore::open(&path).unwrap());
                Connection::open(&path).unwrap().execute_batch(sql).unwrap();
            }
            let start = std::sync::Arc::new(std::sync::Barrier::new(8));
            let openers: Vec<_> = (0..8)
                .map(|_| {
                    let (start, path) = (start.clone(), path.clone());
                    std::thread::spawn(move || {
                        start.wait();
                        TaskStore::open(&path)
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    })
                })
                .collect();
            for opener in openers {
                assert_eq!(opener.join().unwrap(), Ok(()));
            }
        }
    }
}
