//! Ticket 10: Won't Do is a resolution of a Completed Task, not a fourth
//! work status (ADR 0008). Synthetic data only.

use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::Value;
use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::bulk::BulkAction;
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateTask, Priority, RecordProgress, SCHEMA_VERSION, Task, TaskError, TaskQuery,
    TaskStore, WorkStatus,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn task(store: &TaskStore, title: &str) -> Task {
    store
        .create_task(CreateTask {
            title: title.into(),
            description: String::new(),
        })
        .unwrap()
}

fn change() -> ChangeTask {
    ChangeTask {
        id: String::new(),
        expected_revision: 0,
        status: None,
        priority: None,
        progress: None,
        difficulty: None,
        wont_do: None,
    }
}

fn apply(store: &TaskStore, task: &Task, input: ChangeTask) -> Result<Task, TaskError> {
    store.change_task(ChangeTask {
        id: task.id.clone(),
        expected_revision: task.revision,
        ..input
    })
}

fn at_progress(store: &TaskStore, title: &str, progress: u8) -> Task {
    let created = task(store, title);
    if progress == 0 {
        return created;
    }
    apply(
        store,
        &created,
        ChangeTask {
            progress: Some(progress),
            ..change()
        },
    )
    .unwrap()
}

fn wont_do(store: &TaskStore, task: &Task) -> Result<Task, TaskError> {
    apply(
        store,
        task,
        ChangeTask {
            wont_do: Some(true),
            ..change()
        },
    )
}

fn transition_message(error: &TaskError) -> &'static str {
    match error {
        TaskError::InvalidTransition(message) => message,
        other => panic!("expected an invalid transition, got {other:?}"),
    }
}

#[test]
fn a_new_task_is_not_wont_do() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Synthetic chore");
    assert!(!created.wont_do);
    assert!(!store.get_task(&created.id).unwrap().unwrap().wont_do);
}

#[test]
fn marking_wont_do_completes_the_task_keeps_its_progress_and_records_a_progress_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = at_progress(&store, "Synthetic draft", 40);

    let set_aside = wont_do(&store, &started).unwrap();
    assert_eq!(set_aside.status, WorkStatus::Completed);
    assert!(set_aside.wont_do);
    assert_eq!(set_aside.progress, 40, "keeps the progress it had");
    assert_eq!(set_aside.revision, started.revision + 1);
    assert_eq!(store.get_task(&started.id).unwrap().unwrap(), set_aside);

    let history = store.list_progress_entries(&started.id, 50, None).unwrap();
    let entry = &history.items[0];
    assert_eq!(
        (entry.previous_status, entry.status),
        (WorkStatus::InProgress, WorkStatus::Completed)
    );
    assert_eq!((entry.previous_progress, entry.progress), (40, 40));
    assert_eq!((entry.previous_wont_do, entry.wont_do), (false, true));
    // The earlier entry (0% → 40%) was not Won't Do on either side.
    assert_eq!(
        (history.items[1].previous_wont_do, history.items[1].wont_do),
        (false, false)
    );

    let audit = store
        .list_audit_entries(Some(&started.id), 10, None)
        .unwrap()
        .items;
    assert_eq!(audit[0].action, AuditAction::TaskChange);
    assert_eq!(audit[0].outcome, AuditOutcome::Applied);
    assert_eq!(audit[0].revision, Some(set_aside.revision));
}

#[test]
fn a_ready_task_can_be_wont_do_at_zero_percent() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let ready = task(&store, "Synthetic idea");
    let set_aside = wont_do(&store, &ready).unwrap();
    assert_eq!(
        (set_aside.status, set_aside.progress, set_aside.wont_do),
        (WorkStatus::Completed, 0, true)
    );
}

#[test]
fn wont_do_may_name_completed_but_no_other_status_or_a_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = at_progress(&store, "Synthetic plan", 30);
    for refused in [
        ChangeTask {
            wont_do: Some(true),
            status: Some(WorkStatus::InProgress),
            ..change()
        },
        ChangeTask {
            wont_do: Some(true),
            status: Some(WorkStatus::Ready),
            ..change()
        },
        ChangeTask {
            wont_do: Some(true),
            progress: Some(30),
            ..change()
        },
    ] {
        let error = apply(&store, &started, refused).unwrap_err();
        transition_message(&error);
    }
    assert_eq!(store.get_task(&started.id).unwrap().unwrap(), {
        let mut same = started.clone();
        same.tags = Vec::new();
        same
    });

    let named = apply(
        &store,
        &started,
        ChangeTask {
            wont_do: Some(true),
            status: Some(WorkStatus::Completed),
            ..change()
        },
    )
    .unwrap();
    assert_eq!(
        (named.status, named.progress, named.wont_do),
        (WorkStatus::Completed, 30, true)
    );
}

#[test]
fn a_finished_task_is_not_set_aside_and_marking_twice_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let finished = at_progress(&store, "Synthetic done", 100);
    assert_eq!(finished.status, WorkStatus::Completed);
    let error = wont_do(&store, &finished).unwrap_err();
    assert!(transition_message(&error).contains("reopen"), "{error:?}");

    let set_aside = wont_do(&store, &task(&store, "Synthetic maybe")).unwrap();
    let again = wont_do(&store, &set_aside).unwrap_err();
    assert!(matches!(again, TaskError::EmptyProgressUpdate), "{again:?}");
}

#[test]
fn reopening_through_the_status_clears_wont_do_by_the_usual_progress_rules() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));

    // In Progress returns to the progress the Task kept.
    let started = wont_do(&store, &at_progress(&store, "Synthetic started", 40)).unwrap();
    let reopened = apply(
        &store,
        &started,
        ChangeTask {
            status: Some(WorkStatus::InProgress),
            ..change()
        },
    )
    .unwrap();
    assert_eq!(
        (reopened.status, reopened.progress, reopened.wont_do),
        (WorkStatus::InProgress, 40, false)
    );
    let entry = &store
        .list_progress_entries(&started.id, 1, None)
        .unwrap()
        .items[0];
    assert_eq!((entry.previous_wont_do, entry.wont_do), (true, false));
    assert_eq!(
        (entry.previous_status, entry.status),
        (WorkStatus::Completed, WorkStatus::InProgress)
    );

    // Ready is open again to a Won't Do Task that never started…
    let unstarted = wont_do(&store, &task(&store, "Synthetic unstarted")).unwrap();
    let ready = apply(
        &store,
        &unstarted,
        ChangeTask {
            status: Some(WorkStatus::Ready),
            ..change()
        },
    )
    .unwrap();
    assert_eq!(
        (ready.status, ready.progress, ready.wont_do),
        (WorkStatus::Ready, 0, false)
    );

    // …but not to one with progress.
    let partial = wont_do(&store, &at_progress(&store, "Synthetic partial", 20)).unwrap();
    let error = apply(
        &store,
        &partial,
        ChangeTask {
            status: Some(WorkStatus::Ready),
            ..change()
        },
    )
    .unwrap_err();
    transition_message(&error);
}

#[test]
fn clearing_wont_do_alone_reopens_to_ready_or_in_progress_by_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let clear = ChangeTask {
        wont_do: Some(false),
        ..change()
    };

    let unstarted = wont_do(&store, &task(&store, "Synthetic zero")).unwrap();
    let reopened = apply(&store, &unstarted, clear.clone()).unwrap();
    assert_eq!(
        (reopened.status, reopened.progress, reopened.wont_do),
        (WorkStatus::Ready, 0, false)
    );

    let started = wont_do(&store, &at_progress(&store, "Synthetic sixty", 60)).unwrap();
    let reopened = apply(&store, &started, clear.clone()).unwrap();
    assert_eq!(
        (reopened.status, reopened.progress, reopened.wont_do),
        (WorkStatus::InProgress, 60, false)
    );

    // Clearing a Task that is not Won't Do changes nothing.
    let error = apply(&store, &reopened, clear).unwrap_err();
    assert!(matches!(error, TaskError::EmptyProgressUpdate), "{error:?}");
}

#[test]
fn changing_progress_reopens_and_a_normal_complete_clears_wont_do() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));

    let lowered = wont_do(&store, &at_progress(&store, "Synthetic lowered", 50)).unwrap();
    let reopened = apply(
        &store,
        &lowered,
        ChangeTask {
            progress: Some(20),
            ..change()
        },
    )
    .unwrap();
    assert_eq!(
        (reopened.status, reopened.progress, reopened.wont_do),
        (WorkStatus::InProgress, 20, false)
    );

    let to_full = wont_do(&store, &at_progress(&store, "Synthetic full", 50)).unwrap();
    let finished = apply(
        &store,
        &to_full,
        ChangeTask {
            progress: Some(100),
            ..change()
        },
    )
    .unwrap();
    assert_eq!(
        (finished.status, finished.progress, finished.wont_do),
        (WorkStatus::Completed, 100, false)
    );

    let completed = wont_do(&store, &at_progress(&store, "Synthetic complete", 70)).unwrap();
    let finished = apply(
        &store,
        &completed,
        ChangeTask {
            status: Some(WorkStatus::Completed),
            ..change()
        },
    )
    .unwrap();
    assert_eq!(
        (finished.status, finished.progress, finished.wont_do),
        (WorkStatus::Completed, 100, false)
    );
    let entry = &store
        .list_progress_entries(&completed.id, 1, None)
        .unwrap()
        .items[0];
    assert_eq!((entry.previous_wont_do, entry.wont_do), (true, false));
    assert_eq!((entry.previous_progress, entry.progress), (70, 100));
}

#[test]
fn priority_and_notes_on_a_wont_do_task_keep_it_wont_do() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let set_aside = wont_do(&store, &at_progress(&store, "Synthetic kept", 10)).unwrap();
    let prioritized = apply(
        &store,
        &set_aside,
        ChangeTask {
            priority: Some(Priority::High),
            ..change()
        },
    )
    .unwrap();
    assert!(prioritized.wont_do);
    let noted = store
        .record_progress(RecordProgress {
            id: set_aside.id.clone(),
            expected_revision: prioritized.revision,
            status: None,
            progress: None,
            next_step: None,
            note: Some("Synthetic reason it was set aside".into()),
        })
        .unwrap();
    assert!(noted.wont_do);
    assert_eq!((noted.status, noted.progress), (WorkStatus::Completed, 10));
    let entry = &store
        .list_progress_entries(&noted.id, 1, None)
        .unwrap()
        .items[0];
    assert_eq!((entry.previous_wont_do, entry.wont_do), (true, true));
}

#[test]
fn a_stale_revision_or_a_removed_task_cannot_be_marked_wont_do() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = at_progress(&store, "Synthetic stale", 10);
    let mut stale = started.clone();
    stale.revision -= 1;
    let error = wont_do(&store, &stale).unwrap_err();
    assert!(matches!(error, TaskError::RevisionConflict), "{error:?}");
    assert!(
        store
            .list_audit_entries(Some(&started.id), 10, None)
            .unwrap()
            .items
            .iter()
            .any(|e| e.outcome == AuditOutcome::Rejected
                && e.error_code.as_deref() == Some("revision_conflict"))
    );

    let removed = store
        .set_task_removed(SetRemoved {
            id: started.id.clone(),
            expected_revision: started.revision,
            removed: true,
        })
        .unwrap();
    assert!(wont_do(&store, &removed).is_err());
    assert!(!store.get_task(&started.id).unwrap().unwrap().wont_do);
}

#[test]
fn a_query_filters_by_wont_do_and_keeps_it_in_the_completed_group() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    at_progress(&store, "Synthetic finished", 100);
    let set_aside = wont_do(&store, &at_progress(&store, "Synthetic set aside", 30)).unwrap();
    at_progress(&store, "Synthetic open", 30);

    let titles = |query: &TaskQuery| -> Vec<String> {
        store
            .query_tasks(query, 50, None)
            .unwrap()
            .items
            .into_iter()
            .map(|t| t.title)
            .collect()
    };
    assert_eq!(
        titles(&TaskQuery {
            wont_do: Some(true),
            ..TaskQuery::default()
        }),
        ["Synthetic set aside"]
    );
    let mut finished = titles(&TaskQuery {
        status: Some(WorkStatus::Completed),
        wont_do: Some(false),
        ..TaskQuery::default()
    });
    finished.sort();
    assert_eq!(finished, ["Synthetic finished"]);
    // Unfiltered, the Won't Do Task sits in the Completed group, after open work.
    let all = titles(&TaskQuery::default());
    assert_eq!(all[0], "Synthetic open");
    assert!(all[1..].contains(&set_aside.title));

    // The desktop sends the filter inside a camelCase query.
    let query: TaskQuery = serde_json::from_value(serde_json::json!({"wontDo": true})).unwrap();
    assert_eq!(query.wont_do, Some(true));
}

#[test]
fn the_desktop_change_reads_wont_do_in_camel_case() {
    let input: ChangeTask = serde_json::from_value(serde_json::json!({
        "id": "x", "expectedRevision": 1, "wontDo": true
    }))
    .unwrap();
    assert_eq!(input.wont_do, Some(true));
    let task = Task {
        wont_do: true,
        ..serde_json::from_value(serde_json::json!({
            "id": "x", "title": "t", "description": "", "status": "completed",
            "priority": "medium", "difficulty": null, "progress": 10, "nextStep": "",
            "listId": null, "dueDate": null, "revision": 1, "createdAt": 1,
            "updatedAt": 1, "removed": false, "tags": [], "wontDo": false
        }))
        .unwrap()
    };
    assert_eq!(serde_json::to_value(&task).unwrap()["wontDo"], true);
}

/// Puts a database back exactly as schema v17 left it.
fn back_to_v17(db: &Path) {
    let conn = Connection::open(db).unwrap();
    conn.execute_batch(
        "ALTER TABLE tasks DROP COLUMN wont_do;
         ALTER TABLE progress_entries DROP COLUMN previous_wont_do;
         ALTER TABLE progress_entries DROP COLUMN wont_do;
         PRAGMA user_version = 17;",
    )
    .unwrap();
}

#[test]
fn upgrading_from_v17_leaves_every_task_and_entry_not_wont_do_and_changes_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade.db");
    let store = store_at(&db);
    let finished = at_progress(&store, "Synthetic finished", 100);
    let open = at_progress(&store, "Synthetic open", 40);
    drop(store);
    back_to_v17(&db);
    let snapshot = |conn: &Connection| -> Vec<(String, String, i64, i64, i64)> {
        let mut statement = conn
            .prepare("SELECT id, status, progress, revision, updated_at FROM tasks ORDER BY id")
            .unwrap();
        statement
            .query_map(params![], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    let before = snapshot(&Connection::open(&db).unwrap());

    let store = store_at(&db);
    assert_eq!(SCHEMA_VERSION, 18);
    for id in [&finished.id, &open.id] {
        assert!(!store.get_task(id).unwrap().unwrap().wont_do);
        for entry in store.list_progress_entries(id, 50, None).unwrap().items {
            assert_eq!((entry.previous_wont_do, entry.wont_do), (false, false));
        }
    }
    let conn = Connection::open(&db).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(snapshot(&conn), before);
    // Only a Completed Task can be Won't Do, even written by hand.
    let invalid = conn.execute(
        "UPDATE tasks SET wont_do = 1 WHERE id = ?1",
        params![open.id],
    );
    assert!(invalid.is_err());
}

#[test]
fn exports_include_wont_do() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let finished = at_progress(&store, "Synthetic finished", 100);
    let set_aside = wont_do(&store, &at_progress(&store, "Synthetic set aside", 30)).unwrap();
    let desktop = TaskStore::open_as(&dir.path().join("t.db"), Interface::Desktop).unwrap();

    let json = dir.path().join("export.json");
    desktop
        .export_data(ExportRequest {
            format: ExportFormat::Json,
            destination: json.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    let record = |id: &str| {
        doc["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(record(&finished.id)["wont_do"], false);
    let set_aside_record = record(&set_aside.id);
    assert_eq!(set_aside_record["wont_do"], true);
    assert_eq!(set_aside_record["progress"], 30);
    let last = set_aside_record["progress_entries"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        (last["previous_wont_do"].clone(), last["wont_do"].clone()),
        (false.into(), true.into())
    );

    let markdown = dir.path().join("Markdown export");
    desktop
        .export_data(ExportRequest {
            format: ExportFormat::Markdown,
            destination: markdown.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let inbox = std::fs::read_to_string(markdown.join("Tasks/Inbox.md")).unwrap();
    assert!(
        inbox.contains("- Status: Completed (Won't Do) · Priority: Medium · Progress: 30%"),
        "{inbox}"
    );
    assert!(inbox.contains("won't do at 30%"), "{inbox}");
    assert_eq!(inbox.matches("Completed (Won't Do)").count(), 1, "{inbox}");
}

#[test]
fn a_bulk_status_request_can_mark_wont_do_after_approval() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let desktop = store_at(&path);
    let ai = TaskStore::open_as(&path, Interface::Mcp).unwrap();
    let a = at_progress(&ai, "Synthetic bulk a", 25);
    let b = task(&ai, "Synthetic bulk b");

    let action = BulkAction::SetTaskStatus {
        status: WorkStatus::Completed,
        wont_do: true,
    };
    let requested = ai
        .request_bulk(action.clone(), vec![a.id.clone(), b.id.clone()])
        .unwrap();
    // Nothing changes until the person approves it.
    assert!(!desktop.get_task(&a.id).unwrap().unwrap().wont_do);
    desktop.approve_bulk_request(&requested.request.id).unwrap();
    let (a, b) = (
        desktop.get_task(&a.id).unwrap().unwrap(),
        desktop.get_task(&b.id).unwrap().unwrap(),
    );
    assert_eq!(
        (a.status, a.progress, a.wont_do),
        (WorkStatus::Completed, 25, true)
    );
    assert_eq!(
        (b.status, b.progress, b.wont_do),
        (WorkStatus::Completed, 0, true)
    );

    // The action's serde form names it; older requests without the field
    // still read as a plain status change.
    assert_eq!(
        serde_json::to_value(&action).unwrap(),
        serde_json::json!({"action": "set_task_status", "status": "completed", "wont_do": true})
    );
    let plain: BulkAction = serde_json::from_value(
        serde_json::json!({"action": "set_task_status", "status": "completed"}),
    )
    .unwrap();
    assert_eq!(
        plain,
        BulkAction::SetTaskStatus {
            status: WorkStatus::Completed,
            wont_do: false
        }
    );
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        serde_json::json!({"action": "set_task_status", "status": "completed"})
    );

    // Won't Do with another status is refused at request time, as for one Task.
    let error = ai
        .request_bulk(
            BulkAction::SetTaskStatus {
                status: WorkStatus::InProgress,
                wont_do: true,
            },
            vec![a.id.clone()],
        )
        .unwrap_err();
    assert_eq!(error.code(), "invalid_transition");
}
