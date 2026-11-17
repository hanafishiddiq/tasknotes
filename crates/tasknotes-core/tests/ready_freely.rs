//! Owner decision (2026-11-18, ADR 0006 addendum): the person may set any
//! Task back to Ready, from In Progress, Completed or Won't Do, and it keeps
//! the progress it had. Only explicitly: lowering a started Task to 0% still
//! leaves it In Progress (ADR 0006), and any new percentage on a Ready Task
//! starts it again. Synthetic data only.

use std::path::Path;

use tasknotes_core::tasks::{ChangeTask, CreateTask, Task, TaskError, TaskStore, WorkStatus};

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

fn state(task: &Task) -> (WorkStatus, u8, bool) {
    (task.status, task.progress, task.wont_do)
}

#[test]
fn an_in_progress_task_goes_back_to_ready_and_keeps_its_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = apply(&store, &task(&store, "Synthetic forty"), progress(40)).unwrap();
    assert_eq!(state(&started), (WorkStatus::InProgress, 40, false));

    let ready = apply(&store, &started, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&ready), (WorkStatus::Ready, 40, false));

    // The move is in its history, like every status change.
    let history = store.list_progress_entries(&ready.id, 50, None).unwrap();
    let latest = &history.items[0];
    assert_eq!(
        (latest.previous_status, latest.status, latest.progress),
        (WorkStatus::InProgress, WorkStatus::Ready, 40)
    );

    // Choosing In Progress again picks up where it was.
    let again = apply(&store, &ready, status(WorkStatus::InProgress)).unwrap();
    assert_eq!(state(&again), (WorkStatus::InProgress, 40, false));
}

#[test]
fn a_started_task_idle_at_zero_can_go_back_to_ready_but_never_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = apply(&store, &task(&store, "Synthetic idle"), progress(30)).unwrap();
    // Lowering to 0% is not a choice of Ready (ADR 0006).
    let lowered = apply(&store, &started, progress(0)).unwrap();
    assert_eq!(state(&lowered), (WorkStatus::InProgress, 0, false));

    let ready = apply(&store, &lowered, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&ready), (WorkStatus::Ready, 0, false));
}

#[test]
fn a_new_percentage_on_a_ready_task_starts_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = apply(&store, &task(&store, "Synthetic resumed"), progress(40)).unwrap();
    let ready = apply(&store, &started, status(WorkStatus::Ready)).unwrap();

    let moved = apply(&store, &ready, progress(50)).unwrap();
    assert_eq!(state(&moved), (WorkStatus::InProgress, 50, false));
    // Lowering a Ready Task to 0% keeps it Ready.
    let ready = apply(&store, &moved, status(WorkStatus::Ready)).unwrap();
    let zero = apply(&store, &ready, progress(0)).unwrap();
    assert_eq!(state(&zero), (WorkStatus::Ready, 0, false));
}

#[test]
fn ready_may_be_given_a_percentage_but_never_100() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let fresh = task(&store, "Synthetic set by hand");
    let both = ChangeTask {
        status: Some(WorkStatus::Ready),
        progress: Some(20),
        ..ChangeTask::default()
    };
    let ready = apply(&store, &fresh, both).unwrap();
    assert_eq!(state(&ready), (WorkStatus::Ready, 20, false));

    let full = ChangeTask {
        status: Some(WorkStatus::Ready),
        progress: Some(100),
        ..ChangeTask::default()
    };
    let refused = apply(&store, &ready, full).unwrap_err();
    assert!(
        matches!(refused, TaskError::InvalidTransition(_)),
        "{refused:?}"
    );
}

#[test]
fn a_completed_task_reopens_to_ready_at_the_progress_it_had() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = apply(&store, &task(&store, "Synthetic finished"), progress(40)).unwrap();
    let done = apply(&store, &started, status(WorkStatus::Completed)).unwrap();
    let ready = apply(&store, &done, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&ready), (WorkStatus::Ready, 40, false));
    assert_eq!(ready.completed_from, None);

    // Completed from that Ready, the check reopens it to Ready, still at 40%.
    let done = apply(&store, &ready, status(WorkStatus::Completed)).unwrap();
    assert_eq!(done.completed_from, Some(WorkStatus::Ready));
    let reopened = apply(&store, &done, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&reopened), (WorkStatus::Ready, 40, false));
}

#[test]
fn a_wont_do_task_goes_back_to_ready_and_keeps_its_progress() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = apply(&store, &task(&store, "Synthetic set aside"), progress(30)).unwrap();
    let set_aside = apply(
        &store,
        &started,
        ChangeTask {
            wont_do: Some(true),
            ..ChangeTask::default()
        },
    )
    .unwrap();
    assert_eq!(state(&set_aside), (WorkStatus::Completed, 30, true));

    let ready = apply(&store, &set_aside, status(WorkStatus::Ready)).unwrap();
    assert_eq!(state(&ready), (WorkStatus::Ready, 30, false));
}
