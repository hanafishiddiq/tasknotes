use tasknotes_core::tasks::{
    ChangeTask, CreateTask, Priority, RecordProgress, TaskStore, WorkStatus,
};

fn task(store: &TaskStore) -> tasknotes_core::tasks::Task {
    store
        .create_task(CreateTask {
            title: "Ship the release".into(),
            description: "Avoid losing progress".into(),
        })
        .unwrap()
}

#[test]
fn dated_progress_note_and_next_step_survive_restart_with_an_immutable_entry() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("tasks.db");
    let store = TaskStore::open(&db).unwrap();
    let created = task(&store);

    let updated = store
        .record_progress(RecordProgress {
            id: created.id.clone(),
            expected_revision: created.revision,
            status: None,
            progress: Some(45),
            next_step: Some("Review the acceptance checklist".into()),
            note: Some("Wrote the first three sections".into()),
        })
        .unwrap();
    assert_eq!(updated.status, WorkStatus::InProgress);
    assert_eq!(updated.progress, 45);
    assert_eq!(updated.next_step, "Review the acceptance checklist");
    assert_eq!(updated.revision, created.revision + 1);

    let page = store.list_progress_entries(&created.id, 10, None).unwrap();
    assert!(page.next_cursor.is_none());
    assert_eq!(page.items.len(), 1);
    let entry = &page.items[0];
    assert!(!entry.id.is_empty());
    assert_eq!(entry.previous_status, WorkStatus::Ready);
    assert_eq!(entry.status, WorkStatus::InProgress);
    assert_eq!(entry.previous_progress, 0);
    assert_eq!(entry.progress, 45);
    assert_eq!(entry.note, "Wrote the first three sections");
    assert_eq!(entry.next_step, "Review the acceptance checklist");
    assert!(entry.created_at >= created.created_at);

    drop(store);
    let store = TaskStore::open(&db).unwrap();
    assert_eq!(store.get_task(&created.id).unwrap().unwrap(), updated);
    assert_eq!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items[0],
        *entry
    );
}

#[test]
fn existing_status_transitions_also_append_entries_and_reopen_at_previous_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let created = task(&store);
    let underway = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: created.revision,
            status: None,
            priority: None,
            progress: Some(60),
        })
        .unwrap();
    let done = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: underway.revision,
            status: Some(WorkStatus::Completed),
            priority: None,
            progress: None,
        })
        .unwrap();
    let reopened = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: done.revision,
            status: Some(WorkStatus::InProgress),
            priority: None,
            progress: None,
        })
        .unwrap();
    assert_eq!(reopened.status, WorkStatus::InProgress);
    assert_eq!(reopened.progress, 60);

    let page = store.list_progress_entries(&created.id, 10, None).unwrap();
    assert_eq!(page.items.len(), 3);
    assert_eq!(
        page.items.iter().map(|e| e.progress).collect::<Vec<_>>(),
        vec![60, 100, 60]
    );
    assert_eq!(
        page.items
            .iter()
            .map(|e| e.previous_progress)
            .collect::<Vec<_>>(),
        vec![100, 60, 0]
    );
    assert_eq!(
        page.items.iter().map(|e| e.status).collect::<Vec<_>>(),
        vec![
            WorkStatus::InProgress,
            WorkStatus::Completed,
            WorkStatus::InProgress
        ]
    );

    // Changing only Priority does not fabricate a Progress Entry.
    let _ = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: reopened.revision,
            status: None,
            priority: Some(Priority::VeryHigh),
            progress: None,
        })
        .unwrap();
    assert_eq!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items,
        page.items
    );
}

#[test]
fn note_only_update_records_work_without_changing_percentage_and_paginates_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let original = task(&store);
    let mut current = original;
    for i in 0..7 {
        current = store
            .record_progress(RecordProgress {
                id: current.id.clone(),
                expected_revision: current.revision,
                status: None,
                progress: None,
                next_step: if i == 0 {
                    Some("Start tests".into())
                } else {
                    None
                },
                note: Some(format!(" Session {i} ")),
            })
            .unwrap();
        assert_eq!(current.status, WorkStatus::Ready);
        assert_eq!(current.progress, 0);
        assert_eq!(current.next_step, "Start tests");
    }

    let mut cursor = None;
    let mut collected = Vec::new();
    let mut timestamps = Vec::new();
    loop {
        let page = store.list_progress_entries(&current.id, 3, cursor).unwrap();
        assert!(page.items.len() <= 3);
        for entry in page.items {
            collected.push(entry.note);
            timestamps.push((entry.sequence, entry.created_at));
        }
        if page.next_cursor.is_none() {
            break;
        }
        cursor = page.next_cursor;
    }
    assert_eq!(
        collected,
        (0..7)
            .rev()
            .map(|i| format!("Session {i}"))
            .collect::<Vec<_>>()
    );
    assert!(timestamps.windows(2).all(|pair| pair[0].0 > pair[1].0));
    assert!(store.list_progress_entries(&current.id, 0, None).is_err());
    assert!(store.list_progress_entries(&current.id, 51, None).is_err());
    assert!(
        store
            .list_progress_entries(&current.id, 2, Some(-1))
            .is_err()
    );
    assert!(store.list_progress_entries("not-a-task", 2, None).is_err());
}

#[test]
fn invalid_progress_or_stale_revision_cannot_write_task_or_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let original = task(&store);
    let saved = store
        .record_progress(RecordProgress {
            id: original.id.clone(),
            expected_revision: original.revision,
            status: None,
            progress: Some(75),
            next_step: Some("Review output".into()),
            note: Some("Complete initial implementation".into()),
        })
        .unwrap();
    let existing = store
        .list_progress_entries(&saved.id, 10, None)
        .unwrap()
        .items;

    let bad = [
        RecordProgress {
            id: original.id.clone(),
            expected_revision: original.revision,
            status: None,
            progress: Some(80),
            next_step: None,
            note: Some("Stale".into()),
        },
        RecordProgress {
            id: original.id.clone(),
            expected_revision: saved.revision,
            status: None,
            progress: Some(101),
            next_step: None,
            note: Some("Invalid".into()),
        },
        RecordProgress {
            id: original.id.clone(),
            expected_revision: saved.revision,
            status: Some(WorkStatus::Ready),
            progress: None,
            next_step: None,
            note: Some("Reset".into()),
        },
        RecordProgress {
            id: original.id.clone(),
            expected_revision: saved.revision,
            status: None,
            progress: None,
            next_step: None,
            note: Some("  ".into()),
        },
        RecordProgress {
            id: original.id.clone(),
            expected_revision: saved.revision,
            status: None,
            progress: None,
            next_step: Some(" ".repeat(4001)),
            note: None,
        },
        RecordProgress {
            id: original.id.clone(),
            expected_revision: saved.revision,
            status: None,
            progress: None,
            next_step: None,
            note: Some("A".repeat(10001)),
        },
    ];
    for (index, request) in bad.into_iter().enumerate() {
        assert!(
            store.record_progress(request).is_err(),
            "invalid case {index} was accepted"
        );
        assert_eq!(store.get_task(&saved.id).unwrap().unwrap(), saved);
        assert_eq!(
            store
                .list_progress_entries(&saved.id, 10, None)
                .unwrap()
                .items,
            existing
        );
    }
}

#[test]
fn next_step_can_be_edited_and_cleared_with_a_dated_history_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let created = task(&store);
    let with_step = store
        .record_progress(RecordProgress {
            id: created.id.clone(),
            expected_revision: created.revision,
            status: None,
            progress: None,
            next_step: Some("  Draft slides  ".into()),
            note: None,
        })
        .unwrap();
    assert_eq!(with_step.next_step, "Draft slides");
    let cleared = store
        .record_progress(RecordProgress {
            id: created.id.clone(),
            expected_revision: with_step.revision,
            status: None,
            progress: None,
            next_step: Some(String::new()),
            note: None,
        })
        .unwrap();
    assert_eq!(cleared.next_step, "");
    let entries = store
        .list_progress_entries(&created.id, 10, None)
        .unwrap()
        .items;
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.next_step.as_str())
            .collect::<Vec<_>>(),
        vec!["", "Draft slides"]
    );
}

#[test]
fn zero_one_ninety_nine_and_hundred_percent_have_dated_transitions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.db");
    let store = TaskStore::open(&path).unwrap();
    let initial = task(&store);
    let mut current = initial.clone();
    for (percent, expected_status) in [
        (0, WorkStatus::Ready),
        (1, WorkStatus::InProgress),
        (99, WorkStatus::InProgress),
        (100, WorkStatus::Completed),
    ] {
        // The first 0% is unchanged and therefore not a substantive entry.
        if percent == 0 {
            assert!(
                store
                    .record_progress(RecordProgress {
                        id: current.id.clone(),
                        expected_revision: current.revision,
                        progress: Some(percent),
                        status: None,
                        note: None,
                        next_step: None,
                    })
                    .is_err()
            );
            continue;
        }
        current = store
            .record_progress(RecordProgress {
                id: current.id.clone(),
                expected_revision: current.revision,
                progress: Some(percent),
                status: None,
                next_step: Some("Next action".into()),
                note: Some(format!("Reached {percent}%")),
            })
            .unwrap();
        assert_eq!(current.status, expected_status);
        assert_eq!(current.progress, percent);
    }
    let reopened = store
        .record_progress(RecordProgress {
            id: current.id.clone(),
            expected_revision: current.revision,
            status: Some(WorkStatus::InProgress),
            progress: None,
            next_step: None,
            note: Some("Reopened for review".into()),
        })
        .unwrap();
    assert_eq!(
        (reopened.status, reopened.progress),
        (WorkStatus::InProgress, 99)
    );
    assert_eq!(reopened.next_step, "Next action");
    let entries = store
        .list_progress_entries(&initial.id, 10, None)
        .unwrap()
        .items;
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.progress)
            .collect::<Vec<_>>(),
        vec![99, 100, 99, 1]
    );
    assert!(entries.iter().all(|entry| entry.created_at > 0));
    drop(store);
    let reopened_store = TaskStore::open(&path).unwrap();
    assert_eq!(
        reopened_store.get_task(&initial.id).unwrap().unwrap(),
        reopened
    );
    assert_eq!(
        reopened_store
            .list_progress_entries(&initial.id, 10, None)
            .unwrap()
            .items,
        entries
    );
}

#[test]
fn oversized_whitespace_progress_note_cannot_bypass_size_limit() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let created = task(&store);
    assert!(
        store
            .record_progress(RecordProgress {
                id: created.id.clone(),
                expected_revision: created.revision,
                status: None,
                progress: Some(5),
                next_step: None,
                note: Some(" ".repeat(10001)),
            })
            .is_err()
    );
    assert_eq!(store.get_task(&created.id).unwrap().unwrap(), created);
    assert!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn v2_database_upgrades_without_losing_completed_task_and_previous_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE tasks (
                 id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL,
                 status TEXT NOT NULL, priority TEXT NOT NULL, progress INTEGER NOT NULL,
                 revision INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                 previous_incomplete_progress INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO tasks VALUES (
                 '00000000-0000-4000-8000-000000000001',
                 'Existing completed work', 'Keep this description',
                 'completed', 'very_high', 100, 7, 1000, 1001, 43
             );
             PRAGMA user_version=2;",
        ).unwrap();
    }
    let store = TaskStore::open(&path).unwrap();
    let old = store
        .get_task("00000000-0000-4000-8000-000000000001")
        .unwrap()
        .unwrap();
    assert_eq!(old.title, "Existing completed work");
    assert_eq!(old.description, "Keep this description");
    assert_eq!(old.revision, 7);
    assert_eq!(old.priority, Priority::VeryHigh);
    assert_eq!(old.next_step, "");
    assert!(
        store
            .list_progress_entries(&old.id, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
    let reopened = store
        .change_task(ChangeTask {
            id: old.id.clone(),
            expected_revision: old.revision,
            status: Some(WorkStatus::InProgress),
            priority: None,
            progress: None,
        })
        .unwrap();
    assert_eq!(reopened.progress, 43);
    assert_eq!(
        store
            .list_progress_entries(&old.id, 10, None)
            .unwrap()
            .items
            .len(),
        1
    );
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6);
}

#[test]
fn failed_history_insert_rolls_back_progress_and_revision_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.db");
    let store = TaskStore::open(&path).unwrap();
    let created = task(&store);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER synthetic_history_failure
             BEFORE INSERT ON progress_entries
             BEGIN
                 SELECT RAISE(ABORT, 'synthetic history write failure');
             END;",
        )
        .unwrap();
    }
    let failed = store.record_progress(RecordProgress {
        id: created.id.clone(),
        expected_revision: created.revision,
        status: None,
        progress: Some(80),
        note: Some("Must not partially save".into()),
        next_step: Some("Still needs QA".into()),
    });
    assert!(failed.is_err(), "SQLite insert trigger must abort");
    assert_eq!(store.get_task(&created.id).unwrap().unwrap(), created);
    assert!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("DROP TRIGGER synthetic_history_failure;")
            .unwrap();
    }
    let saved = store
        .record_progress(RecordProgress {
            id: created.id.clone(),
            expected_revision: created.revision,
            status: None,
            progress: Some(80),
            note: Some("Now safe to save".into()),
            next_step: Some("Continue QA".into()),
        })
        .unwrap();
    assert_eq!(saved.revision, created.revision + 1);
    assert_eq!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items
            .len(),
        1
    );
}
