use tasknotes_core::tasks::{ChangeTask, CreateTask, Priority, TaskStore, WorkStatus};

#[test]
fn priority_and_lifecycle_are_persisted_even_while_idle() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.db");
    let store = TaskStore::open(&path).unwrap();
    let task = store
        .create_task(CreateTask {
            title: "Write a report".into(),
            description: "Research".into(),
        })
        .unwrap();
    let urgent = store
        .change_task(ChangeTask {
            id: task.id.clone(),
            expected_revision: task.revision,
            priority: Some(Priority::VeryHigh),
            status: None,
            progress: None,
        })
        .unwrap();
    assert_eq!(urgent.priority, Priority::VeryHigh);
    assert_eq!(urgent.status, WorkStatus::Ready);
    assert_eq!(urgent.description, "Research");
    let started = store
        .change_task(ChangeTask {
            id: urgent.id.clone(),
            expected_revision: urgent.revision,
            priority: None,
            status: Some(WorkStatus::InProgress),
            progress: None,
        })
        .unwrap();
    assert_eq!(started.status, WorkStatus::InProgress);
    assert_eq!(started.progress, 0);
    drop(store);
    let reopened = TaskStore::open(&path).unwrap();
    assert_eq!(reopened.get_task(&started.id).unwrap().unwrap(), started);
}

#[test]
fn percent_and_status_transitions_preserve_last_incomplete_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let ready = store
        .create_task(CreateTask {
            title: "Finish outline".into(),
            description: "".into(),
        })
        .unwrap();
    let update = |previous: &tasknotes_core::tasks::Task, status, progress| {
        store
            .change_task(ChangeTask {
                id: previous.id.clone(),
                expected_revision: previous.revision,
                priority: None,
                status,
                progress,
            })
            .unwrap()
    };
    let started = update(&ready, None, Some(60));
    assert_eq!(
        (started.status, started.progress),
        (WorkStatus::InProgress, 60)
    );
    let done = update(&started, None, Some(100));
    assert_eq!((done.status, done.progress), (WorkStatus::Completed, 100));
    let reopened = update(&done, Some(WorkStatus::InProgress), None);
    assert_eq!(
        (reopened.status, reopened.progress),
        (WorkStatus::InProgress, 60)
    );
    let idle = update(&reopened, None, Some(0));
    assert_eq!(idle.status, WorkStatus::InProgress);
    let finished = update(&idle, Some(WorkStatus::Completed), None);
    assert_eq!(finished.progress, 100);
    let edited = update(&finished, None, Some(30));
    assert_eq!(
        (edited.status, edited.progress),
        (WorkStatus::InProgress, 30)
    );
}

#[test]
fn contradicting_status_progress_or_reset_to_ready_is_rejected_without_changes() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();
    let ready = store
        .create_task(CreateTask {
            title: "Do work".into(),
            description: "".into(),
        })
        .unwrap();
    let underway = store
        .change_task(ChangeTask {
            id: ready.id.clone(),
            expected_revision: ready.revision,
            priority: None,
            status: None,
            progress: Some(45),
        })
        .unwrap();

    for (status, progress) in [
        (Some(WorkStatus::Ready), None),
        (Some(WorkStatus::Ready), Some(0)),
        (Some(WorkStatus::Ready), Some(45)),
        (Some(WorkStatus::InProgress), Some(100)),
        (Some(WorkStatus::Completed), Some(55)),
        (None, Some(101)),
    ] {
        assert!(
            store
                .change_task(ChangeTask {
                    id: underway.id.clone(),
                    expected_revision: underway.revision,
                    priority: None,
                    status,
                    progress,
                })
                .is_err()
        );
        assert_eq!(store.get_task(&ready.id).unwrap().unwrap(), underway);
    }
    assert!(
        store
            .change_task(ChangeTask {
                id: underway.id.clone(),
                expected_revision: underway.revision,
                priority: None,
                status: None,
                progress: None,
            })
            .is_err()
    );
    let stale = store
        .change_task(ChangeTask {
            id: underway.id.clone(),
            expected_revision: ready.revision,
            priority: Some(Priority::High),
            status: None,
            progress: None,
        })
        .unwrap_err();
    assert!(stale.to_string().to_lowercase().contains("conflict"));

    let completed = store
        .change_task(ChangeTask {
            id: underway.id.clone(),
            expected_revision: underway.revision,
            priority: None,
            status: Some(WorkStatus::Completed),
            progress: None,
        })
        .unwrap();
    assert!(
        store
            .change_task(ChangeTask {
                id: completed.id.clone(),
                expected_revision: completed.revision,
                priority: None,
                status: Some(WorkStatus::Ready),
                progress: None,
            })
            .is_err()
    );
    assert_eq!(store.get_task(&ready.id).unwrap().unwrap(), completed);
}

#[test]
fn v1_database_migrates_without_losing_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.db");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE tasks(
                id TEXT PRIMARY KEY NOT NULL, title TEXT NOT NULL,
                description TEXT NOT NULL, status TEXT NOT NULL,
                priority TEXT NOT NULL, progress INTEGER NOT NULL,
                revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL);
             INSERT INTO tasks VALUES(
                '00000000-0000-4000-8000-000000000001',
                'Old task', 'Keep this data', 'ready', 'medium', 0, 1, 100, 100);
             INSERT INTO tasks VALUES(
                '00000000-0000-4000-8000-000000000002',
                'Already started', 'Keep 60 percent', 'in_progress', 'high', 60, 1, 101, 101);
             PRAGMA user_version=1;",
        )
        .unwrap();
    }
    let store = TaskStore::open(&path).unwrap();
    let old = store
        .get_task("00000000-0000-4000-8000-000000000001")
        .unwrap()
        .unwrap();
    assert_eq!(old.title, "Old task");
    assert_eq!(old.description, "Keep this data");
    let existing = store
        .get_task("00000000-0000-4000-8000-000000000002")
        .unwrap()
        .unwrap();
    assert_eq!(existing.progress, 60);
    let completed = store
        .change_task(ChangeTask {
            id: existing.id.clone(),
            expected_revision: existing.revision,
            status: Some(WorkStatus::Completed),
            priority: None,
            progress: None,
        })
        .unwrap();
    let reopened = store
        .change_task(ChangeTask {
            id: existing.id.clone(),
            expected_revision: completed.revision,
            status: Some(WorkStatus::InProgress),
            priority: None,
            progress: None,
        })
        .unwrap();
    assert_eq!(
        reopened.progress, 60,
        "migration must preserve old in-progress progress"
    );
    drop(store);
    let db = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 8);
}

#[test]
fn group_then_priority_sort_and_cursor_preserve_all_records() {
    use std::collections::HashSet;
    use tasknotes_core::tasks::SortMode;

    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasks.db")).unwrap();

    let add = |title: &str, status: Option<WorkStatus>, priority: Priority| {
        let created = store
            .create_task(CreateTask {
                title: title.into(),
                description: "".into(),
            })
            .unwrap();
        store
            .change_task(ChangeTask {
                id: created.id.clone(),
                expected_revision: created.revision,
                priority: Some(priority),
                status,
                progress: None,
            })
            .unwrap()
    };
    add("ready very low", None, Priority::VeryLow);
    add("ready very high", None, Priority::VeryHigh);
    add(
        "in progress low",
        Some(WorkStatus::InProgress),
        Priority::Low,
    );
    add(
        "in progress high",
        Some(WorkStatus::InProgress),
        Priority::High,
    );
    add(
        "completed high",
        Some(WorkStatus::Completed),
        Priority::High,
    );

    let expected = [
        "in progress high",
        "in progress low",
        "ready very high",
        "ready very low",
        "completed high",
    ];
    let mut cursor: Option<String> = None;
    let mut titles = Vec::new();
    let mut ids = HashSet::new();
    loop {
        let page = store
            .list_tasks_sorted(2, cursor.as_deref(), SortMode::Priority)
            .unwrap();
        for task in page.items {
            assert!(
                ids.insert(task.id.clone()),
                "never duplicate a Task between pages"
            );
            titles.push(task.title);
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(titles, expected);
    assert_eq!(ids.len(), 5);

    let mut newest_ids = HashSet::new();
    let mut newest_cursor = None;
    loop {
        let page = store
            .list_tasks_sorted(2, newest_cursor.as_deref(), SortMode::Newest)
            .unwrap();
        for task in page.items {
            assert!(
                newest_ids.insert(task.id),
                "newest cursor must not duplicate"
            );
        }
        match page.next_cursor {
            Some(next) => newest_cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(newest_ids.len(), 5);

    let newest = store.list_tasks_sorted(20, None, SortMode::Newest).unwrap();
    let statuses: Vec<_> = newest.items.iter().map(|t| t.status).collect();
    assert_eq!(
        statuses,
        [
            WorkStatus::InProgress,
            WorkStatus::InProgress,
            WorkStatus::Ready,
            WorkStatus::Ready,
            WorkStatus::Completed,
        ]
    );
    let page = store
        .list_tasks_sorted(2, None, SortMode::Priority)
        .unwrap();
    let wrong_sort_cursor = page.next_cursor.unwrap();
    assert!(
        store
            .list_tasks_sorted(2, Some(&wrong_sort_cursor), SortMode::Newest)
            .is_err()
    );
    assert!(
        store
            .list_tasks_sorted(2, Some("v2|priority|3|99|0|invalid"), SortMode::Priority)
            .is_err()
    );
}

#[test]
fn equal_priority_tasks_have_a_deterministic_order_across_pages_and_restarts() {
    use tasknotes_core::tasks::SortMode;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.db");
    let store = TaskStore::open(&path).unwrap();
    let mut created = Vec::new();
    for i in 0..8 {
        let task = store
            .create_task(CreateTask {
                title: format!("Same priority {i}"),
                description: "".into(),
            })
            .unwrap();
        assert_eq!(task.priority, Priority::Medium);
        created.push(task);
    }

    let mut first_pass = Vec::new();
    let mut cursor = None;
    loop {
        let page = store
            .list_tasks_sorted(1, cursor.as_deref(), SortMode::Priority)
            .unwrap();
        first_pass.extend(page.items.iter().map(|t| (t.created_at, t.id.clone())));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(first_pass.len(), 8);
    assert!(first_pass.windows(2).all(|pair| pair[0] > pair[1]));
    drop(store);

    let store = TaskStore::open(&path).unwrap();
    let after_restart = store
        .list_tasks_sorted(20, None, SortMode::Priority)
        .unwrap();
    let restart_order: Vec<_> = after_restart
        .items
        .iter()
        .map(|t| (t.created_at, t.id.clone()))
        .collect();
    assert_eq!(restart_order, first_pass);
}
