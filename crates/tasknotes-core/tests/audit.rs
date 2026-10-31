//! Ticket 10: append-only Audit Entries (ADR-0005), written by the core for
//! every interface. Synthetic data only.

use std::path::Path;

use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, MoveTask, Priority, RecordProgress, RenameList,
    SetListRemoved, TaskError, TaskStore, UpdateTask, UpdateTaskDetails,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

#[test]
fn a_desktop_edit_is_audited_and_a_stale_edit_is_refused_and_audited() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let task = store
        .create_task(CreateTask {
            title: "Draft the quarterly summary".into(),
            description: "Synthetic secret-free text".into(),
        })
        .unwrap();
    let edited = store
        .update_task(UpdateTask {
            id: task.id.clone(),
            title: "Draft the yearly summary".into(),
            description: String::new(),
            expected_revision: task.revision,
        })
        .unwrap();

    let stale = store.update_task(UpdateTask {
        id: task.id.clone(),
        title: "Overwrite attempt".into(),
        description: String::new(),
        expected_revision: task.revision,
    });
    assert!(matches!(stale, Err(TaskError::RevisionConflict)));
    assert_eq!(
        store.get_task(&task.id).unwrap().unwrap().title,
        "Draft the yearly summary"
    );

    let entries = store.list_audit_entries(None, 50, None).unwrap().items;
    let seen: Vec<_> = entries
        .iter()
        .map(|e| {
            (
                e.interface,
                e.action,
                e.entity_id.as_str(),
                e.outcome,
                e.error_code.as_deref(),
                e.revision,
            )
        })
        .collect();
    // Newest first.
    assert_eq!(
        seen,
        vec![
            (
                Interface::Desktop,
                AuditAction::TaskUpdate,
                task.id.as_str(),
                AuditOutcome::Rejected,
                Some("revision_conflict"),
                Some(1),
            ),
            (
                Interface::Desktop,
                AuditAction::TaskUpdate,
                task.id.as_str(),
                AuditOutcome::Applied,
                None,
                Some(edited.revision),
            ),
            (
                Interface::Desktop,
                AuditAction::TaskCreate,
                task.id.as_str(),
                AuditOutcome::Applied,
                None,
                Some(1),
            ),
        ]
    );
    let raw = format!("{entries:?}");
    assert!(!raw.contains("quarterly") && !raw.contains("Overwrite"));
}

#[test]
fn every_task_and_list_change_is_audited_under_the_interface_that_made_it() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let desktop = store_at(&db);
    let cli = TaskStore::open_as(&db, Interface::Cli).unwrap();
    assert_eq!(cli.interface(), Interface::Cli);

    let list = cli
        .create_list(CreateList {
            name: "Errands".into(),
        })
        .unwrap();
    let list = cli
        .rename_list(RenameList {
            id: list.id,
            expected_revision: list.revision,
            name: "Weekend errands".into(),
        })
        .unwrap();
    let t = cli
        .create_task(CreateTask {
            title: "Buy stamps".into(),
            description: String::new(),
        })
        .unwrap();
    let t = cli
        .change_task(ChangeTask {
            id: t.id,
            expected_revision: t.revision,
            status: None,
            priority: Some(Priority::High),
            progress: None,
        })
        .unwrap();
    let t = desktop
        .record_progress(RecordProgress {
            id: t.id,
            expected_revision: t.revision,
            status: None,
            progress: Some(40),
            next_step: Some("Find the post office".into()),
            note: None,
        })
        .unwrap();
    let t = cli
        .update_task_details(UpdateTaskDetails {
            id: t.id,
            expected_revision: t.revision,
            tags: vec!["post".into()],
            due_date: Some("2030-01-15".into()),
        })
        .unwrap();
    let t = cli
        .move_task(MoveTask {
            id: t.id,
            expected_revision: t.revision,
            list_id: Some(list.id.clone()),
        })
        .unwrap();
    let t = cli
        .set_task_removed(SetRemoved {
            id: t.id,
            expected_revision: t.revision,
            removed: true,
        })
        .unwrap();
    let t = cli
        .set_task_removed(SetRemoved {
            id: t.id,
            expected_revision: t.revision,
            removed: false,
        })
        .unwrap();
    let list = desktop
        .set_list_removed(SetListRemoved {
            id: list.id,
            expected_revision: list.revision,
            removed: true,
        })
        .unwrap();
    // Changing a Task while its List is in Removed is refused and recorded.
    assert!(matches!(
        cli.update_task(UpdateTask {
            id: t.id.clone(),
            title: "Buy more stamps".into(),
            description: String::new(),
            expected_revision: t.revision,
        }),
        Err(TaskError::TaskListRemoved)
    ));
    let list = cli
        .set_list_removed(SetListRemoved {
            id: list.id,
            expected_revision: list.revision,
            removed: false,
        })
        .unwrap();
    // An input mistake is not a consequential change and is not recorded.
    assert!(
        cli.create_task(CreateTask {
            title: "   ".into(),
            description: String::new(),
        })
        .is_err()
    );

    let mut entries = cli.list_audit_entries(None, 100, None).unwrap().items;
    entries.reverse();
    let seen: Vec<_> = entries
        .iter()
        .map(|e| {
            (
                e.interface.as_str(),
                e.action.as_str(),
                e.outcome,
                e.revision,
            )
        })
        .collect();
    use AuditOutcome::{Applied, Rejected};
    assert_eq!(
        seen,
        vec![
            ("cli", "list.create", Applied, Some(1)),
            ("cli", "list.rename", Applied, Some(2)),
            ("cli", "task.create", Applied, Some(1)),
            ("cli", "task.change", Applied, Some(2)),
            ("desktop", "task.progress", Applied, Some(3)),
            ("cli", "task.details", Applied, Some(4)),
            ("cli", "task.move", Applied, Some(5)),
            ("cli", "task.remove", Applied, Some(6)),
            ("cli", "task.restore", Applied, Some(7)),
            ("desktop", "list.remove", Applied, Some(3)),
            ("cli", "task.update", Rejected, Some(7)),
            ("cli", "list.restore", Applied, Some(list.revision)),
        ]
    );
    assert_eq!(entries[10].error_code.as_deref(), Some("removed"));
    assert_eq!(entries[2].entity_id, t.id);
    assert_eq!(entries[0].entity_id, list.id);

    // One entity's history, paged.
    let first = cli.list_audit_entries(Some(&t.id), 3, None).unwrap();
    assert_eq!(first.items.len(), 3);
    let rest = cli
        .list_audit_entries(Some(&t.id), 100, first.next_cursor.as_deref())
        .unwrap();
    assert_eq!(first.items.len() + rest.items.len(), 8);
    assert!(rest.next_cursor.is_none());
    assert!(matches!(
        cli.list_audit_entries(None, 10, Some("a1|zero")),
        Err(TaskError::InvalidAuditCursor)
    ));
}

#[test]
fn audit_entries_cannot_be_rewritten_or_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let store = store_at(&db);
    store
        .create_task(CreateTask {
            title: "Water the plants".into(),
            description: String::new(),
        })
        .unwrap();
    let conn = rusqlite::Connection::open(&db).unwrap();
    assert!(
        conn.execute("UPDATE audit_entries SET outcome = 'rejected'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM audit_entries", []).is_err());
    assert_eq!(
        store
            .list_audit_entries(None, 10, None)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn errors_carry_stable_codes() {
    assert_eq!(TaskError::RevisionConflict.code(), "revision_conflict");
    assert_eq!(TaskError::NotFound.code(), "not_found");
    assert_eq!(TaskError::EmptyTitle.code(), "invalid_input");
    assert_eq!(TaskError::InvalidCursor.code(), "invalid_cursor");
    assert_eq!(TaskError::TaskRemoved.code(), "removed");
    assert_eq!(TaskError::ListNameConflict.code(), "name_conflict");
    assert_eq!(
        TaskError::UnsupportedSchema(99).code(),
        "unsupported_schema"
    );
}

fn version(db: &Path) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// A database exactly as schema v9 left it, holding one Task.
fn v9_database(db: &Path) -> String {
    let store = store_at(db);
    let task = store
        .create_task(CreateTask {
            title: "Renew library card".into(),
            description: String::new(),
        })
        .unwrap();
    drop(store);
    rusqlite::Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP TRIGGER audit_entries_append_only_update;
             DROP TRIGGER audit_entries_append_only_delete;
             DROP TABLE audit_entries;
             DROP TABLE retired_revisions;
             DROP TABLE bulk_requests;
             ALTER TABLE notes DROP COLUMN exclude_from_ai;
             DROP INDEX lists_manual_order; ALTER TABLE lists DROP COLUMN position;
             CREATE INDEX lists_created_order ON lists(created_at DESC, id DESC);
             DROP INDEX note_folders_manual_order; ALTER TABLE note_folders DROP COLUMN position;
             PRAGMA user_version = 9;",
        )
        .unwrap();
    task.id
}

#[test]
fn a_v9_database_keeps_its_tasks_and_gains_the_audit_log() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v9.db");
    let id = v9_database(&db);
    assert_eq!(version(&db), 9);
    let store = store_at(&db);
    assert_eq!(version(&db), tasknotes_core::tasks::SCHEMA_VERSION);
    let kept = store.get_task(&id).unwrap().unwrap();
    assert_eq!(kept.title, "Renew library card");
    assert!(
        store
            .list_audit_entries(None, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
    store
        .update_task(UpdateTask {
            id,
            title: "Renew the library card".into(),
            description: String::new(),
            expected_revision: kept.revision,
        })
        .unwrap();
    assert_eq!(
        store
            .list_audit_entries(None, 10, None)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn a_failed_v10_migration_rolls_back_and_keeps_the_database_usable() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("collision.db");
    let id = v9_database(&db);
    // The table is created first; the index name then collides.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE audit_entries_entity(collision INTEGER);")
        .unwrap();

    assert!(TaskStore::open(&db).is_err());
    assert_eq!(version(&db), 9);
    let half_done: bool = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'audit_entries')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !half_done,
        "audit_entries must roll back with the migration"
    );

    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TABLE audit_entries_entity;")
        .unwrap();
    let store = store_at(&db);
    assert_eq!(version(&db), tasknotes_core::tasks::SCHEMA_VERSION);
    assert!(store.get_task(&id).unwrap().is_some());
}

#[test]
fn a_refused_write_naming_no_existing_row_stores_no_caller_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open_as(&dir.path().join("t.db"), Interface::Cli).unwrap();
    let invented = "caller-chosen text, not an id";
    let results = [
        store
            .update_task(UpdateTask {
                id: invented.into(),
                title: "Anything".into(),
                description: String::new(),
                expected_revision: 1,
            })
            .err(),
        store
            .change_task(ChangeTask {
                id: invented.into(),
                expected_revision: 1,
                status: None,
                priority: Some(Priority::High),
                progress: None,
            })
            .err(),
        store
            .record_progress(RecordProgress {
                id: invented.into(),
                expected_revision: 1,
                status: None,
                progress: Some(10),
                next_step: None,
                note: None,
            })
            .err(),
        store
            .update_task_details(UpdateTaskDetails {
                id: invented.into(),
                expected_revision: 1,
                tags: vec!["x".into()],
                due_date: None,
            })
            .err(),
        store
            .move_task(MoveTask {
                id: invented.into(),
                expected_revision: 1,
                list_id: None,
            })
            .err(),
        store
            .set_task_removed(SetRemoved {
                id: invented.into(),
                expected_revision: 1,
                removed: true,
            })
            .err(),
        store
            .rename_list(RenameList {
                id: invented.into(),
                expected_revision: 1,
                name: "Anything".into(),
            })
            .err(),
        store
            .set_list_removed(SetListRemoved {
                id: invented.into(),
                expected_revision: 1,
                removed: true,
            })
            .err(),
    ];
    assert!(results.iter().all(Option::is_some), "{results:?}");
    assert!(
        store
            .list_audit_entries(None, 10, None)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn a_refusal_that_cannot_be_audited_says_so_and_keeps_its_code() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let store = store_at(&db);
    let task = store
        .create_task(CreateTask {
            title: "Book the dentist".into(),
            description: String::new(),
        })
        .unwrap();
    // Stand-in for a storage failure that outlasts the busy timeout.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_rejections BEFORE INSERT ON audit_entries
             WHEN NEW.outcome = 'rejected'
             BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
        )
        .unwrap();
    let stale = |revision| {
        store.update_task(UpdateTask {
            id: task.id.clone(),
            title: "Book the dentist today".into(),
            description: String::new(),
            expected_revision: revision,
        })
    };
    store
        .update_task(UpdateTask {
            id: task.id.clone(),
            title: "Book the dentist soon".into(),
            description: String::new(),
            expected_revision: task.revision,
        })
        .unwrap();
    let error = stale(task.revision).unwrap_err();
    assert_eq!(error.code(), "revision_conflict");
    assert!(error.audit_unrecorded(), "{error:?}");
    assert_eq!(
        error.to_string(),
        TaskError::RevisionConflict.to_string(),
        "the person still sees the original refusal"
    );

    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TRIGGER refuse_rejections;")
        .unwrap();
    let recorded = stale(task.revision).unwrap_err();
    assert!(matches!(recorded, TaskError::RevisionConflict));
    assert!(!recorded.audit_unrecorded());
}
