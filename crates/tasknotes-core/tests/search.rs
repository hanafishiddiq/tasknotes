use std::path::Path;

use rusqlite::Connection;
use tasknotes_core::tasks::{
    CreateList, CreateTask, RecordProgress, SetListRemoved, Task, TaskQuery, TaskScope, TaskStore,
    TaskView, UpdateTask,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn task(store: &TaskStore, title: &str, description: &str) -> Task {
    store
        .create_task(CreateTask {
            title: title.into(),
            description: description.into(),
        })
        .unwrap()
}

fn progress(store: &TaskStore, task: Task, note: &str, next_step: Option<&str>) -> Task {
    store
        .record_progress(RecordProgress {
            id: task.id,
            expected_revision: task.revision,
            status: None,
            progress: None,
            next_step: next_step.map(str::to_owned),
            note: Some(note.into()),
        })
        .unwrap()
}

fn search(store: &TaskStore, text: &str) -> Vec<String> {
    titles(
        store,
        &TaskQuery {
            text: text.into(),
            ..TaskQuery::default()
        },
    )
}

fn titles(store: &TaskStore, query: &TaskQuery) -> Vec<String> {
    store
        .query_tasks(query, 50, None)
        .unwrap()
        .items
        .into_iter()
        .map(|task| task.title)
        .collect()
}

/// Ticket 09 owns the remove/restore operation; until then a test marks the
/// disposition directly to prove the query layer already honours it.
fn mark_removed(db: &Path, id: &str) {
    Connection::open(db)
        .unwrap()
        .execute("UPDATE tasks SET removed_at=1 WHERE id=?1", [id])
        .unwrap();
}

#[test]
fn search_finds_title_description_next_step_and_older_progress_notes() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("search.db"));
    task(&store, "Quarterly budget", "");
    task(&store, "Fix the sink", "Call the plumber on Monday");
    let invoice = task(&store, "Client follow-up", "");
    progress(
        &store,
        invoice,
        "Sent a reminder",
        Some("Draft the invoice"),
    );
    let lease = task(&store, "Renew lease", "");
    let lease = progress(&store, lease, "Met the landlord", None);
    progress(&store, lease, "Signed the papers", None);

    assert_eq!(search(&store, "budget"), vec!["Quarterly budget"]);
    assert_eq!(search(&store, "plumb"), vec!["Fix the sink"]);
    assert_eq!(search(&store, "invoice"), vec!["Client follow-up"]);
    assert_eq!(search(&store, "landlord"), vec!["Renew lease"]);
    assert!(search(&store, "zebra").is_empty());
}

#[test]
fn search_ignores_case_and_accents_and_needs_every_word() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("words.db"));
    task(&store, "Café opening", "");
    let trip = task(&store, "Plan the trip", "");
    progress(&store, trip, "Booked the hotel", None);

    assert_eq!(search(&store, "cafe"), vec!["Café opening"]);
    assert_eq!(search(&store, "CAFÉ"), vec!["Café opening"]);
    assert_eq!(search(&store, "plan hotel"), vec!["Plan the trip"]);
    assert!(search(&store, "plan zebra").is_empty());
}

#[test]
fn search_text_is_never_interpreted_as_query_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("syntax.db"));
    task(&store, "Plan or postpone", "");
    task(&store, "Other task", "");

    for text in ["\"", "plan OR", "*", "NEAR(plan", "plan:", "-plan", "^plan"] {
        assert!(
            store
                .query_tasks(
                    &TaskQuery {
                        text: text.into(),
                        ..TaskQuery::default()
                    },
                    50,
                    None
                )
                .is_ok(),
            "{text:?} must be treated as plain words"
        );
    }
    assert_eq!(search(&store, "plan OR"), vec!["Plan or postpone"]);
    assert_eq!(
        search(&store, "  !!  ").len(),
        2,
        "no words means no text filter"
    );
}

#[test]
fn the_index_follows_title_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("edits.db"));
    let created = task(&store, "Old name", "");
    store
        .update_task(UpdateTask {
            id: created.id,
            title: "New name".into(),
            description: "Fresh description".into(),
            expected_revision: created.revision,
        })
        .unwrap();
    assert!(search(&store, "old").is_empty());
    assert_eq!(search(&store, "new"), vec!["New name"]);
    assert_eq!(search(&store, "fresh"), vec!["New name"]);
}

#[test]
fn removed_tasks_and_tasks_in_removed_lists_stay_out_of_ordinary_search() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("removed.db");
    let store = store_at(&db);
    task(&store, "Visible report", "");
    let removed = task(&store, "Removed report", "");
    mark_removed(&db, &removed.id);
    let list = store
        .create_list(CreateList {
            name: "Archive".into(),
        })
        .unwrap();
    store
        .create_task_in_list(
            CreateTask {
                title: "Listed report".into(),
                description: String::new(),
            },
            Some(&list.id),
        )
        .unwrap();
    store
        .set_list_removed(SetListRemoved {
            id: list.id,
            expected_revision: list.revision,
            removed: true,
        })
        .unwrap();

    assert_eq!(search(&store, "report"), vec!["Visible report"]);
    let removed_scope = TaskQuery {
        text: "report".into(),
        scope: TaskScope::Removed,
        ..TaskQuery::default()
    };
    assert_eq!(titles(&store, &removed_scope), vec!["Removed report"]);
    assert_eq!(
        titles(
            &store,
            &TaskQuery {
                view: TaskView::Inbox,
                ..TaskQuery::default()
            }
        ),
        vec!["Visible report"]
    );
}

#[test]
fn a_v4_database_upgrades_to_v5_and_indexes_existing_history() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("historical-v4.db");
    let id = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
    {
        let c = Connection::open(&db).unwrap();
        c.execute_batch(&format!(
            "CREATE TABLE tasks(
               id TEXT PRIMARY KEY NOT NULL,
               title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
               status TEXT NOT NULL, priority TEXT NOT NULL, progress INTEGER NOT NULL,
               revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL,
               previous_incomplete_progress INTEGER NOT NULL DEFAULT 0,
               next_step TEXT NOT NULL DEFAULT '',
               list_id TEXT, due_date TEXT
             );
             CREATE TABLE progress_entries(
               sequence INTEGER PRIMARY KEY AUTOINCREMENT,
               id TEXT NOT NULL UNIQUE, task_id TEXT NOT NULL REFERENCES tasks(id),
               previous_status TEXT NOT NULL, status TEXT NOT NULL,
               previous_progress INTEGER NOT NULL, progress INTEGER NOT NULL,
               note TEXT NOT NULL DEFAULT '', next_step TEXT NOT NULL DEFAULT '',
               created_at INTEGER NOT NULL
             );
             CREATE TABLE lists(
               id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL,
               revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL, removed_at INTEGER
             );
             CREATE TABLE tags(
               name TEXT PRIMARY KEY COLLATE NOCASE NOT NULL, fold TEXT NOT NULL UNIQUE
             );
             CREATE TABLE task_tags(
               task_id TEXT NOT NULL REFERENCES tasks(id),
               tag_name TEXT NOT NULL REFERENCES tags(name),
               PRIMARY KEY(task_id, tag_name)
             );
             INSERT INTO tasks(id,title,description,status,priority,progress,revision,
                  created_at,updated_at,previous_incomplete_progress,next_step)
             VALUES('{id}','Historic Task','Retain description','in_progress','high',42,7,123,124,42,'Finish audit');
             INSERT INTO progress_entries(id,task_id,previous_status,status,
                    previous_progress,progress,note,next_step,created_at)
             VALUES('bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb','{id}','ready','in_progress',
                    0,42,'Historic landlord note','Finish audit',123);
             PRAGMA user_version=4;"
        ))
        .unwrap();
    }
    let store = store_at(&db);
    assert_eq!(search(&store, "landlord"), vec!["Historic Task"]);
    assert_eq!(search(&store, "audit"), vec!["Historic Task"]);
    let task = store.get_task(id).unwrap().unwrap();
    assert_eq!((task.revision, task.progress), (7, 42));
    drop(store);
    assert_eq!(
        Connection::open(&db)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        7
    );
}

#[test]
fn a_failed_v5_migration_rolls_back_and_keeps_the_v4_database_usable() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("collision.db");
    let created = {
        let store = store_at(&db);
        task(&store, "Preserved Task", "")
    };
    {
        // Simulate a pre-v5 database whose migration cannot complete.
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "DROP TABLE notes_fts; DROP TABLE notes; DROP TABLE note_folders;
             DROP TRIGGER tasks_fts_insert; DROP TRIGGER tasks_fts_update;
             DROP TRIGGER tasks_fts_delete; DROP TRIGGER progress_fts_insert;
             DROP TABLE task_fields_fts; DROP TABLE progress_fts;
             ALTER TABLE tasks DROP COLUMN removed_at;
             CREATE TABLE progress_fts(collision INTEGER);
             PRAGMA user_version=4;",
        )
        .unwrap();
    }
    assert!(TaskStore::open(&db).is_err());
    {
        let c = Connection::open(&db).unwrap();
        let version: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
        let mut stmt = c.prepare("PRAGMA table_info(tasks)").unwrap();
        let columns = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(!columns.iter().any(|name| name == "removed_at"));
        c.execute("DROP TABLE progress_fts", []).unwrap();
    }
    let store = store_at(&db);
    assert_eq!(search(&store, "preserved"), vec!["Preserved Task"]);
    assert_eq!(
        store.get_task(&created.id).unwrap().unwrap().revision,
        created.revision
    );
}

#[test]
fn every_search_word_counts_and_too_many_words_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("long.db"));
    let words: Vec<String> = (1..=16).map(|n| format!("word{n:02}")).collect();
    task(&store, "All sixteen", &words.join(" "));
    task(&store, "Missing the last", &words[..15].join(" "));

    // The 16th word still narrows the result instead of being dropped.
    assert_eq!(search(&store, &words.join(" ")), vec!["All sixteen"]);

    let too_long = format!("{} word17", words.join(" "));
    let error = store
        .query_tasks(
            &TaskQuery {
                text: too_long,
                ..TaskQuery::default()
            },
            50,
            None,
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "Search can use at most 16 words");
}
