//! Owner decision Q48 (2026-11-11, ADR 0006 addendum): reopening a Completed
//! Task returns it to the work status it had before it was completed. A Task
//! completed from Ready goes back to Ready at 0%; one completed from In
//! Progress goes back to In Progress with its previous incomplete progress.
//! Where it returns is read from the Progress Entry that completed it, so no
//! schema changes; without such an entry the earlier rule stands. Synthetic
//! data only.

use std::path::Path;

use tasknotes_core::audit::Interface;
use tasknotes_core::bulk::{BulkAction, StatusTarget};
use tasknotes_core::tasks::{
    ChangeTask, CreateTask, RecordProgress, Task, TaskError, TaskStore, WorkStatus,
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

fn apply(store: &TaskStore, task: &Task, input: ChangeTask) -> Result<Task, TaskError> {
    store.change_task(ChangeTask {
        id: task.id.clone(),
        expected_revision: task.revision,
        ..input
    })
}

fn status(status: WorkStatus) -> ChangeTask {
    ChangeTask {
        status: Some(status),
        ..ChangeTask::default()
    }
}

fn progress(progress: u8) -> ChangeTask {
    ChangeTask {
        progress: Some(progress),
        ..ChangeTask::default()
    }
}

fn state(task: &Task) -> (WorkStatus, u8, Option<WorkStatus>) {
    (task.status, task.progress, task.completed_from)
}

#[test]
fn a_task_completed_from_ready_reopens_to_ready_at_zero() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let ready = task(&store, "Synthetic never started");
    assert_eq!(ready.completed_from, None);

    let done = apply(&store, &ready, status(WorkStatus::Completed)).unwrap();
    // The Task names where reopening returns it, wherever it is read.
    assert_eq!(
        state(&done),
        (WorkStatus::Completed, 100, Some(WorkStatus::Ready))
    );
    assert_eq!(store.get_task(&done.id).unwrap().unwrap(), done);

    let reopened = apply(&store, &done, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&reopened), (WorkStatus::Ready, 0, None));
    let entry = &store
        .list_progress_entries(&done.id, 1, None)
        .unwrap()
        .items[0];
    assert_eq!(
        (
            entry.previous_status,
            entry.status,
            entry.previous_progress,
            entry.progress
        ),
        (WorkStatus::Completed, WorkStatus::Ready, 100, 0)
    );

    // The window's Undo names both, the status and the 0% it had.
    let done = apply(&store, &reopened, status(WorkStatus::Completed)).unwrap();
    let undone = apply(
        &store,
        &done,
        ChangeTask {
            status: Some(WorkStatus::Ready),
            progress: Some(0),
            ..ChangeTask::default()
        },
    )
    .unwrap();
    assert_eq!(state(&undone), (WorkStatus::Ready, 0, None));

    // Lowering the progress to 0 is a reopen too, and lands on Ready.
    let done = apply(&store, &undone, status(WorkStatus::Completed)).unwrap();
    let lowered = apply(&store, &done, progress(0)).unwrap();
    assert_eq!(state(&lowered), (WorkStatus::Ready, 0, None));
    // Any other progress has started it.
    let done = apply(&store, &lowered, status(WorkStatus::Completed)).unwrap();
    let partial = apply(&store, &done, progress(30)).unwrap();
    assert_eq!(state(&partial), (WorkStatus::InProgress, 30, None));

    // Choosing In Progress outright starts it, as for a Won't Do Task (ADR 0008).
    let fresh = task(&store, "Synthetic started on purpose");
    let done = apply(&store, &fresh, status(WorkStatus::Completed)).unwrap();
    let started = apply(&store, &done, status(WorkStatus::InProgress)).unwrap();
    assert_eq!(state(&started), (WorkStatus::InProgress, 0, None));
}

#[test]
fn a_task_completed_from_in_progress_reopens_to_its_previous_progress_and_never_to_ready() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = apply(&store, &task(&store, "Synthetic forty"), progress(40)).unwrap();
    let done = apply(&store, &started, status(WorkStatus::Completed)).unwrap();
    assert_eq!(
        state(&done),
        (WorkStatus::Completed, 100, Some(WorkStatus::InProgress))
    );

    let refused = apply(&store, &done, status(WorkStatus::Ready)).unwrap_err();
    assert!(
        matches!(refused, TaskError::InvalidTransition(_)),
        "{refused:?}"
    );
    let reopened = apply(&store, &done, status(WorkStatus::InProgress)).unwrap();
    assert_eq!(state(&reopened), (WorkStatus::InProgress, 40, None));

    // Started and idle at 0% (ADR 0006) is In Progress too, not Ready.
    let idle = apply(
        &store,
        &task(&store, "Synthetic idle"),
        status(WorkStatus::InProgress),
    )
    .unwrap();
    assert_eq!((idle.status, idle.progress), (WorkStatus::InProgress, 0));
    let done = apply(&store, &idle, status(WorkStatus::Completed)).unwrap();
    assert_eq!(done.completed_from, Some(WorkStatus::InProgress));
    assert!(apply(&store, &done, status(WorkStatus::Ready)).is_err());
    let lowered = apply(&store, &done, progress(0)).unwrap();
    assert_eq!(state(&lowered), (WorkStatus::InProgress, 0, None));
}

#[test]
fn every_way_of_completing_names_the_status_it_completed_from() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store = store_at(&path);

    // Progress 100% from Ready, and from In Progress.
    let ready = task(&store, "Synthetic full from ready");
    let done = apply(&store, &ready, progress(100)).unwrap();
    assert_eq!(done.completed_from, Some(WorkStatus::Ready));
    let started = apply(&store, &task(&store, "Synthetic full"), progress(55)).unwrap();
    let done = apply(&store, &started, progress(100)).unwrap();
    assert_eq!(done.completed_from, Some(WorkStatus::InProgress));

    // A dated update that reaches 100%.
    let ready = task(&store, "Synthetic recorded");
    let recorded = store
        .record_progress(RecordProgress {
            id: ready.id.clone(),
            expected_revision: ready.revision,
            status: None,
            progress: Some(100),
            next_step: None,
            note: Some("Synthetic finished note".into()),
        })
        .unwrap();
    assert_eq!(recorded.completed_from, Some(WorkStatus::Ready));

    // A note, priority or text change afterwards keeps it.
    let noted = store
        .record_progress(RecordProgress {
            id: recorded.id.clone(),
            expected_revision: recorded.revision,
            status: None,
            progress: None,
            next_step: Some("Synthetic later step".into()),
            note: Some("Synthetic later note".into()),
        })
        .unwrap();
    assert_eq!(noted.completed_from, Some(WorkStatus::Ready));
    let reopened = apply(&store, &noted, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&reopened), (WorkStatus::Ready, 0, None));

    // A bulk status change, approved in the desktop.
    let ai = TaskStore::open_as(&path, Interface::Mcp).unwrap();
    let a = task(&ai, "Synthetic bulk ready");
    let b = apply(&ai, &task(&ai, "Synthetic bulk started"), progress(20)).unwrap();
    let requested = ai
        .request_bulk(
            BulkAction::SetTaskStatus(StatusTarget::Work(WorkStatus::Completed)),
            vec![a.id.clone(), b.id.clone()],
        )
        .unwrap();
    store.approve_bulk_request(&requested.request.id).unwrap();
    let a = store.get_task(&a.id).unwrap().unwrap();
    let b = store.get_task(&b.id).unwrap().unwrap();
    assert_eq!(
        state(&a),
        (WorkStatus::Completed, 100, Some(WorkStatus::Ready))
    );
    assert_eq!(
        state(&b),
        (WorkStatus::Completed, 100, Some(WorkStatus::InProgress))
    );
    // A bulk reopen to Ready follows the same rule, Task by Task.
    let reopen = ai
        .request_bulk(
            BulkAction::SetTaskStatus(StatusTarget::Work(WorkStatus::Ready)),
            vec![a.id.clone()],
        )
        .unwrap();
    store.approve_bulk_request(&reopen.request.id).unwrap();
    assert_eq!(
        state(&store.get_task(&a.id).unwrap().unwrap()),
        (WorkStatus::Ready, 0, None)
    );

    // Listed Tasks carry it too.
    let listed = store.list_tasks(50, None).unwrap();
    let b_listed = listed.items.iter().find(|item| item.id == b.id).unwrap();
    assert_eq!(b_listed.completed_from, Some(WorkStatus::InProgress));
}

#[test]
fn wont_do_and_a_later_real_completion_keep_the_status_before_the_task_was_done() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let ready = task(&store, "Synthetic set aside");
    let aside = apply(
        &store,
        &ready,
        ChangeTask {
            wont_do: Some(true),
            ..ChangeTask::default()
        },
    )
    .unwrap();
    assert_eq!(aside.completed_from, Some(WorkStatus::Ready));
    assert_eq!(aside.wont_do_from, Some(WorkStatus::Ready));

    // Finished for real after being set aside: still completed from Ready.
    let finished = apply(&store, &aside, status(WorkStatus::Completed)).unwrap();
    assert_eq!(
        (finished.status, finished.progress, finished.wont_do),
        (WorkStatus::Completed, 100, false)
    );
    assert_eq!(finished.completed_from, Some(WorkStatus::Ready));
    let reopened = apply(&store, &finished, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&reopened), (WorkStatus::Ready, 0, None));
}

#[test]
fn a_stale_revision_cannot_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let done = apply(
        &store,
        &task(&store, "Synthetic stale"),
        status(WorkStatus::Completed),
    )
    .unwrap();
    let renamed = store
        .patch_task(tasknotes_core::tasks::PatchTask {
            id: done.id.clone(),
            expected_revision: done.revision,
            title: Some("Synthetic renamed".into()),
            description: None,
        })
        .unwrap();
    assert_eq!(renamed.completed_from, Some(WorkStatus::Ready));
    let error = apply(&store, &done, status(WorkStatus::Ready)).unwrap_err();
    assert!(matches!(error, TaskError::RevisionConflict), "{error:?}");
    let reopened = apply(&store, &renamed, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&reopened), (WorkStatus::Ready, 0, None));
}

/// A Task completed before Progress Entries existed (schema v2) has no entry
/// to read: the earlier rule stands, so it counts as started.
#[test]
fn a_task_completed_before_progress_history_keeps_the_earlier_rule() {
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
                 'Synthetic old work', '', 'completed', 'medium', 100, 7, 1000, 1001, 0
             );
             DROP TABLE IF EXISTS audit_entries; DROP TABLE IF EXISTS bulk_requests; DROP TABLE IF EXISTS retired_revisions; DROP TABLE IF EXISTS pomodoro_sessions;
             PRAGMA user_version=2;",
        )
        .unwrap();
    }
    let store = store_at(&path);
    let old = store
        .get_task("00000000-0000-4000-8000-000000000001")
        .unwrap()
        .unwrap();
    assert_eq!(state(&old), (WorkStatus::Completed, 100, None));
    let error = apply(&store, &old, status(WorkStatus::Ready)).unwrap_err();
    assert!(
        matches!(error, TaskError::InvalidTransition(_)),
        "{error:?}"
    );
    let lowered = apply(&store, &old, progress(0)).unwrap();
    assert_eq!(state(&lowered), (WorkStatus::InProgress, 0, None));
}
