use std::path::Path;

use rusqlite::Connection;
use tasknotes_core::tasks::{
    CreateList, CreateTask, MoveTask, RenameList, SetListRemoved, SortMode, TaskError, TaskStore,
    TaskView, UpdateTaskDetails,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

#[test]
fn lists_can_be_created_renamed_and_reopened_without_losing_tasks() {
    let folder = tempfile::tempdir().unwrap();
    let db = folder.path().join("ticket04.db");
    let store = store_at(&db);
    let first = store
        .create_list(CreateList {
            name: "Projects".into(),
        })
        .unwrap();
    assert_eq!(first.name, "Projects");
    assert_eq!(first.revision, 1);
    let task = store
        .create_task(CreateTask {
            title: "Old Task remains".into(),
            description: String::new(),
        })
        .unwrap();
    assert_eq!(store.list_lists(10, None, false).unwrap().items.len(), 1);

    let second = store
        .rename_list(RenameList {
            id: first.id.clone(),
            expected_revision: first.revision,
            name: "Work".into(),
        })
        .unwrap();
    assert_eq!(second.name, "Work");
    assert_eq!(second.revision, 2);
    assert!(
        store
            .rename_list(RenameList {
                id: first.id.clone(),
                expected_revision: first.revision,
                name: "Overwritten".into(),
            })
            .is_err()
    );

    drop(store);
    let store = store_at(&db);
    assert_eq!(
        store.get_task(&task.id).unwrap().unwrap().title,
        "Old Task remains"
    );
    assert_eq!(
        store.list_lists(10, None, false).unwrap().items[0].name,
        "Work"
    );
    assert_eq!(
        Connection::open(&db)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
}

#[test]
fn inbox_and_scoped_lists_return_bounded_unique_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("view.db"));
    let list = store
        .create_list(CreateList {
            name: "Work".into(),
        })
        .unwrap();
    let a = store
        .create_task(CreateTask {
            title: "Inbox".into(),
            description: String::new(),
        })
        .unwrap();
    let b = store
        .create_task_in_list(
            CreateTask {
                title: "List".into(),
                description: String::new(),
            },
            Some(&list.id),
        )
        .unwrap();
    assert_eq!(a.list_id, None);
    assert_eq!(b.list_id, Some(list.id.clone()));
    assert_eq!(
        store
            .list_tasks_in_view(50, None, SortMode::Priority, TaskView::Inbox)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        store
            .list_tasks_in_view(
                50,
                None,
                SortMode::Priority,
                TaskView::List(list.id.clone())
            )
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        store
            .list_tasks_in_view(50, None, SortMode::Priority, TaskView::All)
            .unwrap()
            .items
            .len(),
        2
    );
    let moved = store
        .move_task(MoveTask {
            id: a.id.clone(),
            expected_revision: a.revision,
            list_id: Some(list.id.clone()),
        })
        .unwrap();
    assert_eq!(moved.list_id, Some(list.id.clone()));
    assert!(
        store
            .list_tasks_in_view(50, None, SortMode::Priority, TaskView::Inbox)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .move_task(MoveTask {
                id: moved.id.clone(),
                expected_revision: a.revision,
                list_id: None
            })
            .is_err()
    );
    assert!(
        store
            .move_task(MoveTask {
                id: moved.id.clone(),
                expected_revision: moved.revision,
                list_id: Some("bad-list".into())
            })
            .is_err()
    );
    let gone = store
        .set_list_removed(SetListRemoved {
            id: list.id.clone(),
            expected_revision: list.revision,
            removed: true,
        })
        .unwrap();
    assert!(
        store
            .list_tasks_in_view(50, None, SortMode::Priority, TaskView::All)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .list_tasks_in_view(
                50,
                None,
                SortMode::Priority,
                TaskView::List(list.id.clone())
            )
            .is_err()
    );
    assert!(
        store
            .create_task_in_list(
                CreateTask {
                    title: "Rejected".into(),
                    description: String::new()
                },
                Some(&list.id)
            )
            .is_err()
    );
    assert_eq!(store.get_task(&b.id).unwrap().unwrap().title, "List");
    store
        .set_list_removed(SetListRemoved {
            id: gone.id,
            expected_revision: gone.revision,
            removed: false,
        })
        .unwrap();
    assert_eq!(
        store
            .list_tasks_in_view(50, None, SortMode::Priority, TaskView::All)
            .unwrap()
            .items
            .len(),
        2
    );
}

#[test]
fn scoped_cursor_rejects_cross_view_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("page.db"));
    let one = store
        .create_list(CreateList { name: "One".into() })
        .unwrap();
    let other = store
        .create_list(CreateList {
            name: "Other".into(),
        })
        .unwrap();
    for i in 0..9 {
        store
            .create_task_in_list(
                CreateTask {
                    title: format!("Task {i}"),
                    description: String::new(),
                },
                Some(&one.id),
            )
            .unwrap();
    }
    let a = store
        .list_tasks_in_view(3, None, SortMode::Priority, TaskView::List(one.id.clone()))
        .unwrap();
    let cur = a.next_cursor.as_deref().unwrap();
    assert!(
        store
            .list_tasks_in_view(3, Some(cur), SortMode::Priority, TaskView::List(other.id))
            .is_err()
    );
    assert!(
        store
            .list_tasks_in_view(
                3,
                Some(cur),
                SortMode::Newest,
                TaskView::List(one.id.clone())
            )
            .is_err()
    );
    let b = store
        .list_tasks_in_view(3, Some(cur), SortMode::Priority, TaskView::List(one.id))
        .unwrap();
    assert_eq!(b.items.len(), 3);
    assert!(a.items.iter().all(|x| b.items.iter().all(|y| x.id != y.id)));
}

#[test]
fn tags_and_deadline_roundtrip_and_can_be_cleared_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("metadata.db");
    let store = store_at(&db);
    let created = store
        .create_task(CreateTask {
            title: "Plan".into(),
            description: String::new(),
        })
        .unwrap();
    let tagged = store
        .update_task_details(UpdateTaskDetails {
            id: created.id.clone(),
            expected_revision: created.revision,
            tags: vec![" Work ".into(), "work".into(), "Focus".into()],
            due_date: Some("2028-02-29".into()),
        })
        .unwrap();
    assert_eq!(tagged.due_date.as_deref(), Some("2028-02-29"));
    assert_eq!(tagged.tags.len(), 2);
    assert!(tagged.tags.iter().any(|t| t == "Work"));
    assert!(tagged.tags.iter().any(|t| t == "Focus"));
    assert!(
        store
            .update_task_details(UpdateTaskDetails {
                id: tagged.id.clone(),
                expected_revision: created.revision,
                tags: vec![],
                due_date: None
            })
            .is_err()
    );
    drop(store);
    let reopened = store_at(&db);
    assert_eq!(
        reopened.get_task(&tagged.id).unwrap().unwrap().tags.len(),
        2
    );
    let cleared = reopened
        .update_task_details(UpdateTaskDetails {
            id: tagged.id.clone(),
            expected_revision: tagged.revision,
            tags: vec!["Focus".into()],
            due_date: None,
        })
        .unwrap();
    assert_eq!(cleared.tags, vec!["Focus"]);
    assert_eq!(cleared.due_date, None);
    assert_eq!(
        reopened.list_tasks(50, None).unwrap().items[0].tags,
        vec!["Focus"]
    );
}

#[test]
fn invalid_tag_and_impossible_local_deadline_do_not_mutate_a_task() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("invalid.db"));
    let task = store
        .create_task(CreateTask {
            title: "Keep".into(),
            description: String::new(),
        })
        .unwrap();
    for date in [
        "2027-02-29",
        "2026-13-01",
        "2026-04-31",
        "01/02/2026",
        "2026-1-02",
    ] {
        assert!(
            store
                .update_task_details(UpdateTaskDetails {
                    id: task.id.clone(),
                    expected_revision: task.revision,
                    tags: vec!["Okay".into()],
                    due_date: Some(date.into()),
                })
                .is_err(),
            "{date}"
        );
    }
    for tag in ["  ", "\n", &"x".repeat(41)] {
        assert!(
            store
                .update_task_details(UpdateTaskDetails {
                    id: task.id.clone(),
                    expected_revision: task.revision,
                    tags: vec![tag.into()],
                    due_date: Some("2028-02-29".into())
                })
                .is_err()
        );
    }
    let old = store.get_task(&task.id).unwrap().unwrap();
    assert_eq!(old.revision, task.revision);
    assert!(old.tags.is_empty());
    assert!(old.due_date.is_none());
}

#[test]
fn list_name_collision_and_reversible_removal_are_safe() {
    let folder = tempfile::tempdir().unwrap();
    let store = store_at(&folder.path().join("ticket04.db"));
    let list = store
        .create_list(CreateList {
            name: "Work".into(),
        })
        .unwrap();
    assert!(
        store
            .create_list(CreateList {
                name: " work ".into()
            })
            .is_err()
    );
    assert!(store.create_list(CreateList { name: " ".into() }).is_err());
    let removed = store
        .set_list_removed(SetListRemoved {
            id: list.id.clone(),
            expected_revision: list.revision,
            removed: true,
        })
        .unwrap();
    assert!(removed.removed);
    assert!(store.list_lists(10, None, false).unwrap().items.is_empty());
    assert_eq!(store.list_lists(10, None, true).unwrap().items.len(), 1);
    let restored = store
        .set_list_removed(SetListRemoved {
            id: removed.id,
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();
    assert!(!restored.removed);
}

#[test]
fn a_v3_database_upgrades_to_v4_without_changing_task_history() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("historical-v3.db");
    let id = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    {
        let c = Connection::open(&db).unwrap();
        c.execute_batch(&format!(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE tasks(
               id TEXT PRIMARY KEY NOT NULL,
               title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
               status TEXT NOT NULL, priority TEXT NOT NULL, progress INTEGER NOT NULL,
               revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL,
               previous_incomplete_progress INTEGER NOT NULL DEFAULT 0,
               next_step TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE progress_entries(
               sequence INTEGER PRIMARY KEY AUTOINCREMENT,
               id TEXT NOT NULL UNIQUE, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
               previous_status TEXT NOT NULL, status TEXT NOT NULL,
               previous_progress INTEGER NOT NULL, progress INTEGER NOT NULL,
               note TEXT NOT NULL DEFAULT '', next_step TEXT NOT NULL DEFAULT '',
               created_at INTEGER NOT NULL
             );
             INSERT INTO tasks(id,title,description,status,priority,progress,revision,
                  created_at,updated_at,previous_incomplete_progress,next_step)
             VALUES('{id}','Historic Task','Retain description','in_progress','high',42,7,123,124,42,'Finish audit');
             INSERT INTO progress_entries(id,task_id,previous_status,status,
                    previous_progress,progress,note,next_step,created_at)
             VALUES('bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb','{id}','ready','in_progress',
                    0,42,'Historic note','Finish audit',123);
             PRAGMA user_version=3;"
        )).unwrap();
    }
    let store = store_at(&db);
    let task = store.get_task(id).unwrap().unwrap();
    assert_eq!(task.progress, 42);
    assert_eq!(task.revision, 7);
    assert_eq!(task.next_step, "Finish audit");
    assert_eq!(task.description, "Retain description");
    assert_eq!(task.list_id, None);
    assert_eq!(task.due_date, None);
    assert!(task.tags.is_empty());
    let notes = store.list_progress_entries(id, 10, None).unwrap();
    assert_eq!(notes.items.len(), 1);
    assert_eq!(notes.items[0].note, "Historic note");
    drop(store);
    assert_eq!(
        Connection::open(&db)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
}

#[test]
fn tag_and_due_metadata_survive_title_edit_progress_change_and_list_move() {
    use tasknotes_core::tasks::{ChangeTask, UpdateTask, WorkStatus};
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("mutations.db"));
    let list = store
        .create_list(CreateList {
            name: "Group".into(),
        })
        .unwrap();
    let created = store
        .create_task(CreateTask {
            title: "One".into(),
            description: String::new(),
        })
        .unwrap();
    let tagged = store
        .update_task_details(UpdateTaskDetails {
            id: created.id.clone(),
            expected_revision: created.revision,
            tags: vec!["Work".into()],
            due_date: Some("2026-12-30".into()),
        })
        .unwrap();
    let edited = store
        .update_task(UpdateTask {
            id: created.id.clone(),
            title: "Two".into(),
            description: "new details".into(),
            expected_revision: tagged.revision,
        })
        .unwrap();
    assert_eq!(edited.tags, vec!["Work"]);
    let progressed = store
        .change_task(ChangeTask {
            id: created.id.clone(),
            expected_revision: edited.revision,
            progress: Some(45),
            status: Some(WorkStatus::InProgress),
            priority: None,
        })
        .unwrap();
    assert_eq!(progressed.tags, vec!["Work"]);
    let moved = store
        .move_task(MoveTask {
            id: created.id.clone(),
            expected_revision: progressed.revision,
            list_id: Some(list.id),
        })
        .unwrap();
    assert_eq!(moved.tags, vec!["Work"]);
    assert_eq!(moved.due_date.as_deref(), Some("2026-12-30"));
}

#[test]
fn list_cursor_is_bound_to_removed_visibility() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("lists-page.db"));
    for i in 0..6 {
        let list = store
            .create_list(CreateList {
                name: format!("List {i}"),
            })
            .unwrap();
        if i % 2 == 0 {
            store
                .set_list_removed(SetListRemoved {
                    id: list.id,
                    expected_revision: list.revision,
                    removed: true,
                })
                .unwrap();
        }
    }
    let page = store.list_lists(2, None, true).unwrap();
    assert_eq!(page.items.len(), 2);
    let after = page.next_cursor.as_deref().unwrap();
    assert!(
        store.list_lists(2, Some(after), false).is_err(),
        "A cursor from Removed-inclusive pagination must not skip active Lists"
    );
    assert_eq!(
        store.list_lists(2, Some(after), true).unwrap().items.len(),
        2
    );
}

#[test]
fn large_list_pages_are_bounded_without_cross_list_leakage() {
    use std::collections::HashSet;
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("large-list.db"));
    let target = store
        .create_list(CreateList {
            name: "Target".into(),
        })
        .unwrap();
    let neighbor = store
        .create_list(CreateList {
            name: "Neighbor".into(),
        })
        .unwrap();
    for i in 0..135 {
        store
            .create_task_in_list(
                CreateTask {
                    title: format!("Bounded Task {i}"),
                    description: String::new(),
                },
                Some(&target.id),
            )
            .unwrap();
    }
    for i in 0..20 {
        store
            .create_task_in_list(
                CreateTask {
                    title: format!("Other Task {i}"),
                    description: String::new(),
                },
                Some(&neighbor.id),
            )
            .unwrap();
    }
    assert!(
        store
            .list_tasks_in_view(
                101,
                None,
                SortMode::Newest,
                TaskView::List(target.id.clone())
            )
            .is_err()
    );
    let mut cursor = None;
    let mut seen = HashSet::new();
    let mut pages = 0;
    loop {
        let page = store
            .list_tasks_in_view(
                50,
                cursor.as_deref(),
                SortMode::Newest,
                TaskView::List(target.id.clone()),
            )
            .unwrap();
        assert!(page.items.len() <= 50);
        pages += 1;
        for item in page.items {
            assert_eq!(item.list_id.as_deref(), Some(target.id.as_str()));
            assert!(seen.insert(item.id), "cursor returned an ID twice");
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
        assert!(pages < 6, "unbounded pagination");
    }
    assert_eq!(seen.len(), 135);
    assert_eq!(pages, 3);
    assert_eq!(
        store
            .list_tasks_in_view(50, None, SortMode::Newest, TaskView::List(neighbor.id))
            .unwrap()
            .items
            .len(),
        20
    );
}

#[test]
fn v3_to_v4_migration_failure_rolls_back_ddl_and_preserves_task_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("migration-fault.db");
    let id = "00000000-0000-4000-8000-000000000004";
    let note_id = "00000000-0000-4000-8000-000000000005";

    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE tasks(
                 id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL,
                 status TEXT NOT NULL, priority TEXT NOT NULL, progress INTEGER NOT NULL,
                 revision INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                 previous_incomplete_progress INTEGER NOT NULL, next_step TEXT NOT NULL
             );
             CREATE TABLE progress_entries(
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL,
                 task_id TEXT NOT NULL REFERENCES tasks(id), previous_status TEXT NOT NULL,
                 status TEXT NOT NULL, previous_progress INTEGER NOT NULL,
                 progress INTEGER NOT NULL, note TEXT NOT NULL, next_step TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO tasks VALUES(
                 '{id}', 'Preserved Task', 'Before migration', 'in_progress',
                 'medium', 42, 7, 100, 101, 42, 'Restore carefully'
             );
             INSERT INTO progress_entries(id,task_id,previous_status,status,previous_progress,
                 progress,note,next_step,created_at)
             VALUES('{note_id}','{id}','ready','in_progress',0,42,
                 'Immutable Progress Note','Restore carefully',101);
             -- Deliberate synthetic name collision encountered *after* v4 List DDL
             -- and ALTER TABLE in the transaction, to prove that partial writes roll back.
             CREATE TABLE tags(name TEXT PRIMARY KEY);
             PRAGMA user_version = 3;"
        )).unwrap();
    }

    assert!(
        TaskStore::open(&path).is_err(),
        "collision must reject the migration"
    );
    {
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 3, "schema version must not advance on failure");
        let lists_exist: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lists'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lists_exist, 0, "the earlier v4 DDL must have rolled back");
        let mut stmt = conn.prepare("PRAGMA table_info(tasks)").unwrap();
        let columns = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            !columns
                .iter()
                .any(|name| name == "list_id" || name == "due_date")
        );
        let note: String = conn
            .query_row(
                "SELECT note FROM progress_entries WHERE id=?1",
                [note_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(note, "Immutable Progress Note");
        let progress: i64 = conn
            .query_row("SELECT progress FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(progress, 42);
        conn.execute("DROP TABLE tags", []).unwrap();
    }

    let reopened =
        TaskStore::open(&path).expect("retry should succeed once fixture collision removed");
    let saved = reopened.get_task(id).unwrap().unwrap();
    assert_eq!(saved.title, "Preserved Task");
    assert_eq!(saved.progress, 42);
    assert_eq!(saved.revision, 7);
    assert_eq!(saved.list_id, None);
    assert_eq!(saved.due_date, None);
    let entries = reopened.list_progress_entries(id, 10, None).unwrap();
    assert_eq!(entries.items.len(), 1);
    assert_eq!(entries.items[0].note, "Immutable Progress Note");
    let final_version: i64 = Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(final_version, 8);
}

fn tag_task(store: &TaskStore, title: &str, tags: &[&str]) -> tasknotes_core::tasks::Task {
    let created = store
        .create_task(CreateTask {
            title: title.into(),
            description: String::new(),
        })
        .unwrap();
    store
        .update_task_details(UpdateTaskDetails {
            id: created.id,
            expected_revision: created.revision,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            due_date: None,
        })
        .unwrap()
}

#[test]
fn first_tag_spelling_is_canonical_across_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("canonical-tags.db"));
    let first = tag_task(&store, "First", &["Work"]);
    let second = tag_task(&store, "Second", &["work", "iOS"]);
    assert_eq!(first.tags, vec!["Work"]);
    assert_eq!(second.tags, vec!["iOS", "Work"]);
    assert_eq!(
        store.get_task(&second.id).unwrap().unwrap().tags,
        vec!["iOS", "Work"]
    );
}

#[test]
fn case_only_tag_edit_and_same_list_move_report_no_change() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("no-change.db"));
    let task = tag_task(&store, "Plan", &["Work"]);
    let recased = store.update_task_details(UpdateTaskDetails {
        id: task.id.clone(),
        expected_revision: task.revision,
        tags: vec!["WORK".into()],
        due_date: None,
    });
    assert!(
        matches!(recased, Err(TaskError::TaskUnchanged)),
        "{recased:?}"
    );
    let same_list = store.move_task(MoveTask {
        id: task.id.clone(),
        expected_revision: task.revision,
        list_id: None,
    });
    assert!(
        matches!(same_list, Err(TaskError::TaskUnchanged)),
        "{same_list:?}"
    );
    assert!(!TaskError::TaskUnchanged.to_string().contains("Progress"));
    assert_eq!(
        store.get_task(&task.id).unwrap().unwrap().revision,
        task.revision
    );
}

#[test]
fn non_ascii_tags_share_one_canonical_spelling_and_order() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("unicode-tags.db");
    let store = store_at(&db);
    tag_task(&store, "First", &["Ärger"]);
    let second = tag_task(&store, "Second", &["ärger", "Äb", "äa"]);
    assert_eq!(second.tags, vec!["äa", "Äb", "Ärger"]);
    assert_eq!(
        store.get_task(&second.id).unwrap().unwrap().tags,
        second.tags
    );
    let tag_rows: i64 = Connection::open(&db)
        .unwrap()
        .query_row("SELECT count(*) FROM tags", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tag_rows, 3);
}

#[test]
fn reads_show_the_canonical_spelling_for_older_task_tag_rows() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("legacy-tag-row.db");
    let store = store_at(&db);
    let task = tag_task(&store, "Plan", &["Work"]);
    // A pre-canonical build could store the typed spelling on the association.
    Connection::open(&db)
        .unwrap()
        .execute("UPDATE task_tags SET tag_name='work'", [])
        .unwrap();
    assert_eq!(
        store.get_task(&task.id).unwrap().unwrap().tags,
        vec!["Work"]
    );
}

#[test]
fn tags_match_with_full_case_folding_in_any_unicode_form() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("fold-tags.db"));
    tag_task(&store, "Street names", &["Straße", "école"]);
    let later = tag_task(&store, "Shouted", &["STRASSE", "e\u{301}cole"]);
    // ß folds to ss; a precomposed é and e + combining accent are one Tag.
    assert_eq!(later.tags, vec!["Straße", "école"]);
    // Final and medial sigma fold alike, so these are one Tag on one Task.
    let greek = tag_task(&store, "Greek", &["ΣΑΣ", "σας"]);
    assert_eq!(greek.tags, vec!["ΣΑΣ"]);

    // The Tag filter folds the same way.
    let query = tasknotes_core::tasks::TaskQuery {
        tag: Some("strasse".into()),
        ..Default::default()
    };
    assert_eq!(store.query_tasks(&query, 10, None).unwrap().items.len(), 2);
}

#[test]
fn opening_a_v7_database_merges_tags_that_now_fold_alike() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v7-tags.db");
    let store = store_at(&db);
    let first = tag_task(&store, "First", &["Straße"]);
    let second = store
        .create_task(CreateTask {
            title: "Second".into(),
            description: String::new(),
        })
        .unwrap();
    let both = tag_task(&store, "Both", &["Straße"]);
    drop(store);
    // As v7 stored them: lower-cased only, so "STRASSE" was a Tag of its own.
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(&format!(
        "UPDATE tags SET fold = 'straße' WHERE name = 'Straße';
         INSERT INTO tags(name, fold) VALUES ('STRASSE', 'strasse');
         INSERT INTO task_tags(task_id, tag_name) VALUES ('{}', 'STRASSE'), ('{}', 'STRASSE');
         DROP TABLE notes_fts;
         CREATE TABLE notes_v6 (
            id TEXT PRIMARY KEY NOT NULL, markdown TEXT NOT NULL, title TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision >= 1),
            created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
         );
         DROP TABLE task_note_links; DROP TABLE notes; ALTER TABLE notes_v6 RENAME TO notes;
         CREATE INDEX notes_updated_order ON notes(updated_at DESC, id DESC);
         CREATE TABLE task_note_links (
            id TEXT PRIMARY KEY NOT NULL,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE RESTRICT,
            linked_at INTEGER NOT NULL, unlinked_at INTEGER
         );
         DROP TABLE note_folders;
         PRAGMA user_version = 7;",
        second.id, both.id
    ))
    .unwrap();
    drop(conn);

    let store = store_at(&db);
    // The first stored spelling stays; every Task keeps its Tag, once.
    for (task, expected) in [
        (&first, vec!["Straße"]),
        (&second, vec!["Straße"]),
        (&both, vec!["Straße"]),
    ] {
        assert_eq!(
            store.get_task(&task.id).unwrap().unwrap().tags,
            expected,
            "{}",
            task.title
        );
    }
    let query = tasknotes_core::tasks::TaskQuery {
        tag: Some("STRASSE".into()),
        ..Default::default()
    };
    assert_eq!(store.query_tasks(&query, 10, None).unwrap().items.len(), 3);
}
