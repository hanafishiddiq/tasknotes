//! Ticket 02: Pomodoro Sessions are stored in the core, and a finished one
//! tied to a Task adds one Progress Entry without changing its status.
//! Synthetic data only.

use std::path::Path;

use rusqlite::Connection;
use tasknotes_core::audit::{AuditAction, Interface};
use tasknotes_core::pomodoro::{EndedSession, SessionOutcome};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateTask, SCHEMA_VERSION, Task, TaskError, TaskStore, WorkStatus,
};

const MINUTE: i64 = 60_000;
const START: i64 = 1_800_000_000_000;

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

fn finished(task_id: Option<&str>, overtime_seconds: i64) -> EndedSession {
    EndedSession {
        task_id: task_id.map(str::to_owned),
        started_at: START,
        planned_seconds: 25 * 60,
        overtime_seconds,
        outcome: SessionOutcome::Finished,
        ended_at: START + 25 * MINUTE + overtime_seconds * 1000,
    }
}

fn stopped(task_id: Option<&str>) -> EndedSession {
    EndedSession {
        task_id: task_id.map(str::to_owned),
        started_at: START,
        planned_seconds: 25 * 60,
        overtime_seconds: 0,
        outcome: SessionOutcome::Stopped,
        ended_at: START + 10 * MINUTE,
    }
}

#[test]
fn a_session_without_a_task_is_stored_with_its_start_length_overtime_and_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let recorded = store.record_pomodoro_session(finished(None, 135)).unwrap();
    assert!(recorded.task.is_none());

    let sessions = store
        .list_pomodoro_sessions(START - MINUTE, START + 60 * MINUTE)
        .unwrap();
    assert_eq!(sessions, vec![recorded.session.clone()]);
    let session = &sessions[0];
    assert_eq!(session.task_id, None);
    assert_eq!(session.started_at, START);
    assert_eq!(session.planned_seconds, 1500);
    assert_eq!(session.overtime_seconds, 135);
    assert_eq!(session.outcome, SessionOutcome::Finished);
    assert_eq!(session.ended_at, START + 25 * MINUTE + 135_000);
}

#[test]
fn a_finished_session_for_a_task_adds_one_progress_entry_and_keeps_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Write the report");
    let started = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: created.revision,
            progress: Some(40),
            ..ChangeTask::default()
        })
        .unwrap();
    assert_eq!(started.status, WorkStatus::InProgress);

    let recorded = store
        .record_pomodoro_session(finished(Some(&started.id), 7 * 60 + 10))
        .unwrap();
    let after = recorded.task.expect("the Task it was for");
    assert_eq!(after.status, WorkStatus::InProgress);
    assert_eq!(after.progress, 40);
    assert_eq!(after.revision, started.revision + 1);

    let history = store.list_progress_entries(&started.id, 10, None).unwrap();
    assert_eq!(
        history.items.len(),
        2,
        "the progress change and the Pomodoro"
    );
    let entry = &history.items[0];
    assert_eq!(entry.note, "Pomodoro: 25 + 7 min focus");
    assert_eq!(entry.previous_status, entry.status);
    assert_eq!(entry.previous_progress, entry.progress);
}

#[test]
fn a_ready_task_stays_ready_after_a_finished_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Plan the week");
    let recorded = store
        .record_pomodoro_session(finished(Some(&created.id), 0))
        .unwrap();
    let after = recorded.task.unwrap();
    assert_eq!(after.status, WorkStatus::Ready);
    assert_eq!(after.progress, 0);
    let history = store.list_progress_entries(&created.id, 10, None).unwrap();
    assert_eq!(history.items.len(), 1);
    assert_eq!(history.items[0].note, "Pomodoro: 25 min focus");
}

#[test]
fn a_stopped_session_is_stored_but_adds_no_progress_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Read the paper");
    let recorded = store
        .record_pomodoro_session(stopped(Some(&created.id)))
        .unwrap();
    assert_eq!(recorded.session.outcome, SessionOutcome::Stopped);
    assert_eq!(recorded.task.unwrap().revision, created.revision);
    let history = store.list_progress_entries(&created.id, 10, None).unwrap();
    assert!(history.items.is_empty());
    assert_eq!(
        store
            .list_pomodoro_sessions(START, START + 60 * MINUTE)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_session_for_a_task_in_removed_is_kept_without_a_progress_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Old idea");
    let removed = store
        .set_task_removed(SetRemoved {
            id: created.id.clone(),
            expected_revision: created.revision,
            removed: true,
        })
        .unwrap();
    let recorded = store
        .record_pomodoro_session(finished(Some(&created.id), 60))
        .unwrap();
    assert_eq!(
        recorded.session.task_id.as_deref(),
        Some(created.id.as_str())
    );
    assert_eq!(recorded.task.unwrap().revision, removed.revision);
    assert!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn a_session_for_an_unknown_task_is_refused_and_nothing_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let error = store
        .record_pomodoro_session(finished(Some("no-such-task"), 0))
        .unwrap_err();
    assert!(matches!(error, TaskError::NotFound));
    assert!(
        store
            .list_pomodoro_sessions(0, i64::MAX)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn impossible_sessions_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let stopped_with_overtime = EndedSession {
        overtime_seconds: 30,
        ..stopped(None)
    };
    let ends_before_start = EndedSession {
        ended_at: START - 1,
        ..finished(None, 0)
    };
    let negative_overtime = finished(None, -1);
    let zero_length = EndedSession {
        planned_seconds: 0,
        ..finished(None, 0)
    };
    for session in [
        stopped_with_overtime,
        ends_before_start,
        negative_overtime,
        zero_length,
    ] {
        let error = store.record_pomodoro_session(session).unwrap_err();
        assert_eq!(error.code(), "invalid_input", "{error}");
    }
    assert!(
        store
            .list_pomodoro_sessions(0, i64::MAX)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn recording_is_audited_without_user_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Audited");
    let recorded = store
        .record_pomodoro_session(finished(Some(&created.id), 0))
        .unwrap();
    let audit = store.list_audit_entries(None, 50, None).unwrap();
    let actions: Vec<_> = audit.items.iter().map(|entry| entry.action).collect();
    assert!(actions.contains(&AuditAction::PomodoroRecord));
    assert!(actions.contains(&AuditAction::TaskProgress));
    let entry = audit
        .items
        .iter()
        .find(|entry| entry.action == AuditAction::PomodoroRecord)
        .unwrap();
    assert_eq!(entry.entity_id, recorded.session.id);
}

#[test]
fn only_the_desktop_records_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    store_at(&path);
    for interface in [Interface::Cli, Interface::Mcp] {
        let store = TaskStore::open_as(&path, interface).unwrap();
        let error = store
            .record_pomodoro_session(finished(None, 0))
            .unwrap_err();
        assert!(matches!(error, TaskError::DesktopOnly));
        // Reading stays open to every interface for the history (Ticket 14).
        assert!(store.list_pomodoro_sessions(0, i64::MAX).is_ok());
    }
}

#[test]
fn sessions_are_listed_by_start_within_the_asked_range() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    for offset in [0, 60, 30] {
        let session = EndedSession {
            started_at: START + offset * MINUTE,
            ended_at: START + (offset + 25) * MINUTE,
            ..finished(None, 0)
        };
        store.record_pomodoro_session(session).unwrap();
    }
    let starts: Vec<_> = store
        .list_pomodoro_sessions(START, START + 59 * MINUTE)
        .unwrap()
        .iter()
        .map(|session| session.started_at)
        .collect();
    assert_eq!(starts, vec![START, START + 30 * MINUTE]);
}

#[test]
fn stored_sessions_cannot_be_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store = store_at(&path);
    store.record_pomodoro_session(finished(None, 0)).unwrap();
    let db = Connection::open(&path).unwrap();
    assert!(
        db.execute("UPDATE pomodoro_sessions SET overtime_seconds = 999", [])
            .is_err()
    );
    assert!(db.execute("DELETE FROM pomodoro_sessions", []).is_err());
}

#[test]
fn a_v19_database_gains_the_session_table_and_keeps_its_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store = store_at(&path);
    let created = task(&store, "Before the upgrade");
    drop(store);
    {
        // Back to v19, as a database from before Ticket 02 has it.
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "DROP TABLE pomodoro_sessions;
             PRAGMA user_version = 19;",
        )
        .unwrap();
    }
    let store = store_at(&path);
    let db = Connection::open(&path).unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    const { assert!(SCHEMA_VERSION >= 22) };
    assert!(store.get_task(&created.id).unwrap().is_some());
    store.record_pomodoro_session(finished(None, 0)).unwrap();
}
