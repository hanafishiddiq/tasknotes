//! Ticket 09: an optional Difficulty beside Priority. Synthetic data only.

use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::Value;
use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::tasks::{
    ChangeTask, CreateTask, Difficulty, DifficultyFilter, SCHEMA_VERSION, SortMode, Task,
    TaskError, TaskQuery, TaskStore, WorkStatus,
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

fn change(
    store: &TaskStore,
    task: &Task,
    difficulty: Option<Difficulty>,
) -> Result<Task, TaskError> {
    store.change_task(ChangeTask {
        id: task.id.clone(),
        expected_revision: task.revision,
        difficulty: Some(difficulty),
        ..Default::default()
    })
}

fn titles(store: &TaskStore, query: &TaskQuery, limit: u32) -> Vec<String> {
    let mut titles = Vec::new();
    let mut cursor = None;
    loop {
        let page = store.query_tasks(query, limit, cursor.as_deref()).unwrap();
        titles.extend(page.items.into_iter().map(|task| task.title));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return titles,
        }
    }
}

#[test]
fn a_new_task_has_no_difficulty_and_every_level_serializes_by_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Synthetic chore");
    assert_eq!(created.difficulty, None);
    assert_eq!(
        store.get_task(&created.id).unwrap().unwrap().difficulty,
        None
    );
    let names: Vec<Value> = Difficulty::ALL
        .iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect();
    assert_eq!(
        names,
        ["very_easy", "easy", "medium", "hard", "very_hard"].map(Value::from)
    );
}

#[test]
fn setting_and_clearing_difficulty_is_a_revisioned_audited_change() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Synthetic report");

    let hard = change(&store, &created, Some(Difficulty::Hard)).unwrap();
    assert_eq!(hard.difficulty, Some(Difficulty::Hard));
    assert_eq!(hard.revision, created.revision + 1);
    assert_eq!(
        store.get_task(&created.id).unwrap().unwrap().difficulty,
        Some(Difficulty::Hard)
    );

    // A stale revision is refused and leaves the Difficulty alone.
    let stale = change(&store, &created, Some(Difficulty::Easy)).unwrap_err();
    assert!(matches!(stale, TaskError::RevisionConflict), "{stale:?}");

    // Setting the same Difficulty again changes nothing and is refused.
    let same = change(&store, &hard, Some(Difficulty::Hard)).unwrap_err();
    assert!(matches!(same, TaskError::EmptyProgressUpdate), "{same:?}");

    let cleared = change(&store, &hard, None).unwrap();
    assert_eq!(cleared.difficulty, None);
    assert_eq!(cleared.revision, hard.revision + 1);

    // Like Priority, Difficulty writes no Progress Entry.
    let history = store.list_progress_entries(&created.id, 50, None).unwrap();
    assert!(history.items.is_empty());

    let audit = store
        .list_audit_entries(Some(&created.id), 10, None)
        .unwrap()
        .items;
    let applied: Vec<_> = audit
        .iter()
        .filter(|e| e.outcome == AuditOutcome::Applied && e.action == AuditAction::TaskChange)
        .map(|e| e.revision)
        .collect();
    assert_eq!(applied, [Some(cleared.revision), Some(hard.revision)]);
    assert!(audit.iter().any(|e| e.outcome == AuditOutcome::Rejected
        && e.error_code.as_deref() == Some("revision_conflict")));
}

#[test]
fn difficulty_and_priority_change_together_in_one_revision() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Synthetic migration");
    let changed = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: created.revision,
            priority: Some(tasknotes_core::tasks::Priority::High),
            difficulty: Some(Some(Difficulty::VeryHard)),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(changed.revision, created.revision + 1);
    assert_eq!(changed.priority, tasknotes_core::tasks::Priority::High);
    assert_eq!(changed.difficulty, Some(Difficulty::VeryHard));
}

#[test]
fn a_removed_task_cannot_change_difficulty() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let created = task(&store, "Synthetic removed");
    let removed = store
        .set_task_removed(tasknotes_core::removed::SetRemoved {
            id: created.id.clone(),
            expected_revision: created.revision,
            removed: true,
        })
        .unwrap();
    let refused = change(&store, &removed, Some(Difficulty::Easy)).unwrap_err();
    assert_eq!(refused.code(), "removed");
}

/// Five Ready Tasks and one In Progress, created oldest first.
fn sorted_world(store: &TaskStore) {
    for (title, difficulty) in [
        ("none-old", None),
        ("easy", Some(Difficulty::Easy)),
        ("very-hard", Some(Difficulty::VeryHard)),
        ("none-new", None),
        ("hard-old", Some(Difficulty::Hard)),
        ("hard-new", Some(Difficulty::Hard)),
    ] {
        let created = task(store, title);
        if let Some(level) = difficulty {
            change(store, &created, Some(level)).unwrap();
        }
    }
    let started = task(store, "started-none");
    store
        .change_task(ChangeTask {
            id: started.id,
            expected_revision: started.revision,
            progress: Some(10),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn sorting_by_difficulty_puts_very_hard_first_and_no_difficulty_last_within_each_group() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    sorted_world(&store);
    let query = TaskQuery {
        sort: SortMode::Difficulty,
        ..TaskQuery::default()
    };
    let expected = [
        "started-none",
        "very-hard",
        "hard-new",
        "hard-old",
        "easy",
        "none-new",
        "none-old",
    ];
    assert_eq!(titles(&store, &query, 100), expected);
    // Pages of two walk the same order without repeats or gaps.
    assert_eq!(titles(&store, &query, 2), expected);
    assert_eq!(titles(&store, &query, 1), expected);
}

#[test]
fn a_cursor_from_another_sort_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    sorted_world(&store);
    let priority_page = store.query_tasks(&TaskQuery::default(), 2, None).unwrap();
    let cursor = priority_page.next_cursor.unwrap();
    let refused = store
        .query_tasks(
            &TaskQuery {
                sort: SortMode::Difficulty,
                ..TaskQuery::default()
            },
            2,
            Some(&cursor),
        )
        .unwrap_err();
    assert!(matches!(refused, TaskError::InvalidCursor), "{refused:?}");
}

#[test]
fn filtering_by_difficulty_keeps_only_that_level() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    sorted_world(&store);
    let query = TaskQuery {
        difficulty: Some(DifficultyFilter::Is(Difficulty::Hard)),
        ..TaskQuery::default()
    };
    assert_eq!(titles(&store, &query, 100), ["hard-new", "hard-old"]);
    let combined = TaskQuery {
        difficulty: Some(DifficultyFilter::Is(Difficulty::Hard)),
        status: Some(WorkStatus::InProgress),
        ..TaskQuery::default()
    };
    assert!(titles(&store, &combined, 100).is_empty());
}

#[test]
fn filtering_by_no_difficulty_keeps_only_tasks_without_one_and_any_keeps_all() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    sorted_world(&store);
    let none = TaskQuery {
        difficulty: Some(DifficultyFilter::NoDifficulty),
        ..TaskQuery::default()
    };
    let expected = ["started-none", "none-new", "none-old"];
    assert_eq!(titles(&store, &none, 100), expected);
    assert_eq!(titles(&store, &none, 1), expected);
    // Combined with the Difficulty sort, the cursor walks the same rows.
    let sorted = TaskQuery {
        sort: SortMode::Difficulty,
        ..none.clone()
    };
    assert_eq!(titles(&store, &sorted, 1), expected);
    // A cursor from the No Difficulty filter is refused by an unfiltered query.
    let cursor = store
        .query_tasks(&none, 1, None)
        .unwrap()
        .next_cursor
        .unwrap();
    let refused = store
        .query_tasks(&TaskQuery::default(), 1, Some(&cursor))
        .unwrap_err();
    assert!(matches!(refused, TaskError::InvalidCursor), "{refused:?}");
    // Any Difficulty is no filter at all.
    assert_eq!(titles(&store, &TaskQuery::default(), 100).len(), 7);
}

#[test]
fn a_difficulty_filter_reads_and_writes_a_level_name_or_none() {
    let names: Vec<Value> = DifficultyFilter::ALL
        .iter()
        .map(|f| serde_json::to_value(f).unwrap())
        .collect();
    assert_eq!(
        names,
        ["very_easy", "easy", "medium", "hard", "very_hard", "none"].map(Value::from)
    );
    for filter in DifficultyFilter::ALL {
        let name = serde_json::to_value(filter).unwrap();
        assert_eq!(
            serde_json::from_value::<DifficultyFilter>(name).unwrap(),
            filter
        );
    }
    assert!(serde_json::from_value::<DifficultyFilter>(Value::from("brutal")).is_err());
    assert!(serde_json::from_value::<DifficultyFilter>(Value::Null).is_err());
    // The desktop sends the filter inside a camelCase query.
    let query: TaskQuery =
        serde_json::from_value(serde_json::json!({"difficulty": "none"})).unwrap();
    assert_eq!(query.difficulty, Some(DifficultyFilter::NoDifficulty));
}

/// Puts a database back exactly as schema v16 left it.
fn back_to_v16(db: &Path) {
    let conn = Connection::open(db).unwrap();
    conn.execute_batch(
        "DROP INDEX lists_parent; DROP INDEX lists_manual_order;
         DROP INDEX lists_sibling_name; ALTER TABLE lists DROP COLUMN parent_id;
         CREATE INDEX lists_manual_order ON lists(position, id);
         DROP INDEX tasks_status_difficulty_order;
         ALTER TABLE tasks DROP COLUMN difficulty;
         ALTER TABLE lists DROP COLUMN icon; ALTER TABLE lists DROP COLUMN color;
         ALTER TABLE note_folders DROP COLUMN icon; ALTER TABLE note_folders DROP COLUMN color;
         DROP INDEX tasks_custom_order; ALTER TABLE tasks DROP COLUMN position; DROP TABLE view_sorts; ALTER TABLE tasks DROP COLUMN wont_do_from; ALTER TABLE tasks DROP COLUMN wont_do; ALTER TABLE progress_entries DROP COLUMN previous_wont_do;
         ALTER TABLE progress_entries DROP COLUMN wont_do;
         PRAGMA user_version = 16;",
    )
    .unwrap();
}

#[test]
fn upgrading_from_v16_gives_every_task_no_difficulty_and_changes_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade.db");
    let store = store_at(&db);
    let a = task(&store, "Synthetic one");
    let b = task(&store, "Synthetic two");
    drop(store);
    back_to_v16(&db);
    let before: Vec<(String, i64, i64)> = {
        let conn = Connection::open(&db).unwrap();
        let mut statement = conn
            .prepare("SELECT id, revision, updated_at FROM tasks ORDER BY id")
            .unwrap();
        statement
            .query_map(params![], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };

    let store = store_at(&db);
    const { assert!(SCHEMA_VERSION >= 17) };
    for id in [&a.id, &b.id] {
        assert_eq!(store.get_task(id).unwrap().unwrap().difficulty, None);
    }
    let conn = Connection::open(&db).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let mut statement = conn
        .prepare("SELECT id, revision, updated_at FROM tasks ORDER BY id")
        .unwrap();
    let after: Vec<(String, i64, i64)> = statement
        .query_map(params![], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(after, before);
    // The column refuses anything but the five names.
    let invalid = conn.execute(
        "UPDATE tasks SET difficulty = 'impossible' WHERE id = ?1",
        params![a.id],
    );
    assert!(invalid.is_err());
}

#[test]
fn exports_include_difficulty() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plain = task(&store, "Synthetic plain");
    let hard = task(&store, "Synthetic hard");
    change(&store, &hard, Some(Difficulty::VeryHard)).unwrap();
    let ai = TaskStore::open_as(&dir.path().join("t.db"), Interface::Desktop).unwrap();

    let json = dir.path().join("export.json");
    ai.export_data(ExportRequest {
        format: ExportFormat::Json,
        destination: json.clone(),
        include_private_notes: false,
    })
    .unwrap();
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    let difficulty = |id: &str| {
        doc["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .unwrap()["difficulty"]
            .clone()
    };
    assert_eq!(difficulty(&plain.id), Value::Null);
    assert_eq!(difficulty(&hard.id), Value::from("very_hard"));

    let markdown = dir.path().join("Markdown export");
    ai.export_data(ExportRequest {
        format: ExportFormat::Markdown,
        destination: markdown.clone(),
        include_private_notes: false,
    })
    .unwrap();
    let inbox = std::fs::read_to_string(markdown.join("Tasks/Inbox.md")).unwrap();
    assert!(inbox.contains("Difficulty: Very Hard"), "{inbox}");
    assert_eq!(inbox.matches("Difficulty:").count(), 1, "{inbox}");
}
