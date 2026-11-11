//! Ticket 02: Pomodoro Sessions are stored in the core, and a finished one
//! tied to a Task adds one Progress Entry without changing its status.
//! Synthetic data only.

use std::path::Path;

use rusqlite::Connection;
use tasknotes_core::audit::{AuditAction, Interface};
use tasknotes_core::pomodoro::{EndedSession, SessionOutcome};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateSubList, CreateTask, SCHEMA_VERSION, SetListRemoved, Task,
    TaskError, TaskStore, WorkStatus,
};

const MINUTE: i64 = 60_000;
const START: i64 = 1_800_000_000_000;
/// The Mac's clock in Jakarta when the focus started, UTC+7.
const JAKARTA: i32 = 7 * 3600;

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
        utc_offset_seconds: JAKARTA,
        planned_seconds: 25 * 60,
        overtime_seconds,
        outcome: SessionOutcome::Finished,
        stopped_after_seconds: None,
        ended_at: START + 25 * MINUTE + overtime_seconds * 1000,
    }
}

fn stopped(task_id: Option<&str>) -> EndedSession {
    EndedSession {
        task_id: task_id.map(str::to_owned),
        started_at: START,
        utc_offset_seconds: JAKARTA,
        planned_seconds: 25 * 60,
        overtime_seconds: 0,
        outcome: SessionOutcome::Stopped,
        stopped_after_seconds: Some(10 * 60),
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
    assert_eq!(session.utc_offset_seconds, Some(JAKARTA));
    assert_eq!(session.stopped_after_seconds, None);
}

#[test]
fn a_stopped_session_keeps_its_focus_before_stop_and_its_offset() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let new_york = EndedSession {
        utc_offset_seconds: -5 * 3600,
        stopped_after_seconds: Some(746),
        ..stopped(None)
    };
    let recorded = store.record_pomodoro_session(new_york).unwrap();
    assert_eq!(recorded.session.utc_offset_seconds, Some(-5 * 3600));
    assert_eq!(recorded.session.stopped_after_seconds, Some(746));
    assert_eq!(
        store.get_pomodoro_session(&recorded.session.id).unwrap(),
        Some(recorded.session)
    );
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

/// Ticket 12 meets Ticket 02: the focus was for a Task in a sub-List whose
/// parent List went to Removed during the session. The Task is hidden by
/// Removed, so the session keeps its Task id and adds no Progress Entry.
#[test]
fn a_session_for_a_task_in_a_sub_list_of_a_removed_list_is_kept_without_a_progress_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let parent = store
        .create_list(CreateList {
            name: "Synthetic parent".into(),
        })
        .unwrap();
    let child = store
        .create_sub_list(CreateSubList {
            name: "Synthetic child".into(),
            parent_id: parent.id.clone(),
        })
        .unwrap();
    let created = store
        .create_task_in_list(
            CreateTask {
                title: "Nested focus".into(),
                description: String::new(),
            },
            Some(&child.id),
        )
        .unwrap();
    let parent_now = store
        .list_lists(10, None, false)
        .unwrap()
        .items
        .into_iter()
        .find(|list| list.id == parent.id)
        .unwrap();
    store
        .set_list_removed(SetListRemoved {
            id: parent.id.clone(),
            expected_revision: parent_now.revision,
            removed: true,
        })
        .unwrap();

    let recorded = store
        .record_pomodoro_session(finished(Some(&created.id), 0))
        .unwrap();
    assert_eq!(
        recorded.session.task_id.as_deref(),
        Some(created.id.as_str())
    );
    assert_eq!(recorded.task.unwrap().revision, created.revision);
    assert!(
        store
            .list_progress_entries(&created.id, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(store.list_pomodoro_sessions(0, i64::MAX).unwrap().len(), 1);
}

#[test]
fn a_session_whose_task_is_gone_after_a_restore_is_kept_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let backup = store.create_backup().unwrap().backup;
    // The focus was for a Task made after the backup the person then restored.
    let created = task(&store, "Made after the backup");
    store.restore_backup(&backup.id).unwrap();
    assert!(store.get_task(&created.id).unwrap().is_none());

    let recorded = store
        .record_pomodoro_session(finished(Some(&created.id), 120))
        .unwrap();
    assert_eq!(recorded.session.task_id, None);
    assert!(recorded.task.is_none());
    let sessions = store.list_pomodoro_sessions(0, i64::MAX).unwrap();
    assert_eq!(sessions, vec![recorded.session]);
    assert_eq!(sessions[0].overtime_seconds, 120);
    assert_eq!(sessions[0].outcome, SessionOutcome::Finished);
}

#[test]
fn a_session_for_an_unknown_task_is_kept_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let recorded = store
        .record_pomodoro_session(stopped(Some("no-such-task")))
        .unwrap();
    assert_eq!(recorded.session.task_id, None);
    assert_eq!(
        store.list_pomodoro_sessions(0, i64::MAX).unwrap(),
        vec![recorded.session]
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
    let stopped_without_its_time = EndedSession {
        stopped_after_seconds: None,
        ..stopped(None)
    };
    let stopped_past_its_plan = EndedSession {
        stopped_after_seconds: Some(25 * 60 + 1),
        ..stopped(None)
    };
    let stopped_before_it_started = EndedSession {
        stopped_after_seconds: Some(-1),
        ..stopped(None)
    };
    let finished_with_a_stop_time = EndedSession {
        stopped_after_seconds: Some(60),
        ..finished(None, 0)
    };
    let beyond_any_time_zone = EndedSession {
        utc_offset_seconds: 18 * 3600 + 1,
        ..finished(None, 0)
    };
    for session in [
        stopped_with_overtime,
        ends_before_start,
        negative_overtime,
        zero_length,
        stopped_without_its_time,
        stopped_past_its_plan,
        stopped_before_it_started,
        finished_with_a_stop_time,
        beyond_any_time_zone,
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
    // The file refuses what the core refuses, whoever writes it.
    let insert = |offset: i64, outcome: &str, after: Option<i64>| {
        db.execute(
            "INSERT INTO pomodoro_sessions (id, started_at, planned_seconds, outcome, ended_at,
               utc_offset_seconds, stopped_after_seconds, created_at)
             VALUES (lower(hex(randomblob(8))), 0, 1500, ?1, 0, ?2, ?3, 0)",
            rusqlite::params![outcome, offset, after],
        )
    };
    assert!(insert(64_801, "finished", None).is_err());
    assert!(insert(0, "finished", Some(60)).is_err());
    assert!(insert(0, "stopped", Some(1501)).is_err());
    assert!(insert(-64_800, "stopped", Some(1500)).is_ok());
}

#[test]
fn a_v22_database_keeps_its_sessions_without_an_offset_or_stop_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    drop(store_at(&path));
    {
        // Back to v22, as Ticket 02 left it, with one session of each kind.
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "ALTER TABLE pomodoro_sessions DROP COLUMN stopped_after_seconds;
             ALTER TABLE pomodoro_sessions DROP COLUMN utc_offset_seconds;
             PRAGMA user_version = 22;",
        )
        .unwrap();
        for (id, outcome) in [("old-finished", "finished"), ("old-stopped", "stopped")] {
            db.execute(
                "INSERT INTO pomodoro_sessions
                   (id, started_at, planned_seconds, outcome, ended_at, created_at)
                 VALUES (?1, ?2, 1500, ?3, ?2 + 600000, ?2)",
                rusqlite::params![id, START, outcome],
            )
            .unwrap();
        }
    }
    let store = store_at(&path);
    let db = Connection::open(&path).unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    const { assert!(SCHEMA_VERSION >= 23) };
    for id in ["old-finished", "old-stopped"] {
        let session = store.get_pomodoro_session(id).unwrap().unwrap();
        assert_eq!(
            (session.utc_offset_seconds, session.stopped_after_seconds),
            (None, None),
            "{id}"
        );
    }
    // Still append-only, and new sessions keep both.
    assert!(
        db.execute("UPDATE pomodoro_sessions SET utc_offset_seconds = 0", [])
            .is_err()
    );
    let recorded = store.record_pomodoro_session(stopped(None)).unwrap();
    assert_eq!(recorded.session.utc_offset_seconds, Some(JAKARTA));
    assert_eq!(recorded.session.stopped_after_seconds, Some(600));
}

#[test]
fn a_v19_database_gains_the_session_table_and_keeps_its_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store = store_at(&path);
    let created = task(&store, "Before the upgrade");
    drop(store);
    {
        // Back to v19, as a database from before Tickets 13, 12 and 02 has
        // it: no Custom Order (v20), no nested Lists (v21), no sessions (v22).
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "DROP TABLE pomodoro_sessions;
             DROP INDEX lists_parent; DROP INDEX lists_manual_order;
             DROP INDEX lists_sibling_name; ALTER TABLE lists DROP COLUMN parent_id;
             CREATE UNIQUE INDEX lists_active_name ON lists(name COLLATE NOCASE)
               WHERE removed_at IS NULL;
             CREATE INDEX lists_manual_order ON lists(position, id);
             DROP INDEX tasks_custom_order; ALTER TABLE tasks DROP COLUMN position;
             DROP TABLE view_sorts;
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

#[test]
fn a_task_completed_during_the_session_gets_the_entry_and_stays_completed() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Finished mid-session");
    let done = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: created.revision,
            status: Some(WorkStatus::Completed),
            ..ChangeTask::default()
        })
        .unwrap();
    let recorded = store
        .record_pomodoro_session(finished(Some(&created.id), 0))
        .unwrap();
    let after = recorded.task.unwrap();
    assert_eq!(after.status, WorkStatus::Completed);
    assert_eq!(after.progress, done.progress);
    let history = store.list_progress_entries(&created.id, 10, None).unwrap();
    assert_eq!(history.items[0].note, "Pomodoro: 25 min focus");
}
