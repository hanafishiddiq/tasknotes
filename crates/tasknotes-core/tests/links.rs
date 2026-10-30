use std::path::Path;

use rusqlite::Connection;
use tasknotes_core::links::{LinkedNote, LinkedTask};
use tasknotes_core::notes::{CreateNote, Note};

use tasknotes_core::tasks::{CreateList, CreateTask, SetListRemoved, Task, TaskError, TaskStore};

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

fn note(store: &TaskStore, markdown: &str) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: None,
        })
        .unwrap()
}

fn note_titles(items: &[LinkedNote]) -> Vec<&str> {
    items.iter().map(|item| item.title.as_str()).collect()
}

fn task_titles(items: &[LinkedTask]) -> Vec<&str> {
    items.iter().map(|item| item.title.as_str()).collect()
}

#[test]
fn a_note_links_to_several_tasks_and_a_task_to_several_notes() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let hotel = task(&store, "Book the hotel");
    let plan = note(&store, "# Trip plan");
    let budget = note(&store, "# Budget");

    store.link_note(&ferry.id, &plan.id).unwrap();
    store.link_note(&hotel.id, &plan.id).unwrap();
    store.link_note(&ferry.id, &budget.id).unwrap();

    let ferry_notes = store.list_task_notes(&ferry.id, 50, None).unwrap();
    assert_eq!(note_titles(&ferry_notes.items), ["Budget", "Trip plan"]);
    let plan_tasks = store.list_note_tasks(&plan.id, 50, None).unwrap();
    assert_eq!(
        task_titles(&plan_tasks.items),
        ["Book the hotel", "Book the ferry"]
    );
    let hotel_notes = store.list_task_notes(&hotel.id, 50, None).unwrap();
    assert_eq!(note_titles(&hotel_notes.items), ["Trip plan"]);
}

#[test]
fn linking_the_same_pair_again_keeps_one_link_with_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan");

    let first = store.link_note(&ferry.id, &plan.id).unwrap();
    let again = store.link_note(&ferry.id, &plan.id).unwrap();

    assert_eq!(again, first);
    let notes = store.list_task_notes(&ferry.id, 50, None).unwrap();
    assert_eq!(notes.items.len(), 1);
    assert_eq!(notes.items[0].link_id, first.id);
}

#[test]
fn unlinking_removes_only_the_relationship_and_undo_brings_back_the_same_link() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan\n\nLeave at 9");
    let link = store.link_note(&ferry.id, &plan.id).unwrap();

    let unlinked = store.unlink_note(&link.id).unwrap();

    assert_eq!(unlinked.id, link.id);
    assert!(unlinked.unlinked_at.is_some());
    assert!(
        store
            .list_task_notes(&ferry.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .list_note_tasks(&plan.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );
    // Linking and unlinking never write the Task or the Note.
    assert_eq!(store.get_task(&ferry.id).unwrap().unwrap(), ferry);
    assert_eq!(store.get_note(&plan.id).unwrap().unwrap(), plan);

    let restored = store.restore_link(&link.id).unwrap();
    assert_eq!(restored, link, "Undo restores the same link, ID and order");
    let tasks = store.list_note_tasks(&plan.id, 50, None).unwrap();
    assert_eq!(task_titles(&tasks.items), ["Book the ferry"]);
}

#[test]
fn unlinking_a_link_that_is_not_active_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan");
    let link = store.link_note(&ferry.id, &plan.id).unwrap();
    store.unlink_note(&link.id).unwrap();

    assert!(matches!(
        store.unlink_note(&link.id),
        Err(TaskError::LinkNotFound)
    ));
    assert!(matches!(
        store.unlink_note("00000000-0000-4000-8000-000000000000"),
        Err(TaskError::LinkNotFound)
    ));
}

#[test]
fn a_link_to_a_missing_task_or_note_is_refused_and_nothing_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan");
    let missing = "00000000-0000-4000-8000-000000000000";

    assert!(matches!(
        store.link_note(missing, &plan.id),
        Err(TaskError::NotFound)
    ));
    assert!(matches!(
        store.link_note(&ferry.id, missing),
        Err(TaskError::NoteNotFound)
    ));
    assert!(
        store
            .list_task_notes(&ferry.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .list_note_tasks(&plan.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );
}

/// Another writer of the same SQLite file (a later CLI, a script) uses SQL directly.
fn other_writer(db: &Path) -> Connection {
    let conn = Connection::open(db).unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    conn
}

#[test]
fn the_database_refuses_dangling_links_from_any_writer() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("links.db");
    let store = store_at(&db);
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan");
    store.link_note(&ferry.id, &plan.id).unwrap();
    let conn = other_writer(&db);

    let dangling = conn.execute(
        "INSERT INTO task_note_links (id, task_id, note_id, linked_at)
         VALUES ('00000000-0000-4000-8000-000000000001', ?1, 'no-such-note', 1)",
        [&ferry.id],
    );
    assert!(dangling.is_err(), "a link must point at a stored Note");
    let duplicate = conn.execute(
        "INSERT INTO task_note_links (id, task_id, note_id, linked_at)
         VALUES ('00000000-0000-4000-8000-000000000002', ?1, ?2, 1)",
        [&ferry.id, &plan.id],
    );
    assert!(duplicate.is_err(), "a pair is linked at most once");
    assert!(
        conn.execute("DELETE FROM notes WHERE id = ?1", [&plan.id])
            .is_err(),
        "a linked Note cannot be erased out from under its link"
    );
    assert!(
        conn.execute("DELETE FROM tasks WHERE id = ?1", [&ferry.id])
            .is_err(),
        "a linked Task cannot be erased out from under its link"
    );
    assert_eq!(
        store
            .list_task_notes(&ferry.id, 50, None)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn links_survive_reopening_and_join_tasks_in_different_lists() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("links.db");
    let store = store_at(&db);
    let travel = store
        .create_list(CreateList {
            name: "Travel".into(),
        })
        .unwrap();
    let work = store
        .create_list(CreateList {
            name: "Work".into(),
        })
        .unwrap();
    let new_task = |title: &str, list: Option<&str>| {
        store
            .create_task_in_list(
                CreateTask {
                    title: title.into(),
                    description: String::new(),
                },
                list,
            )
            .unwrap()
    };
    let ferry = new_task("Book the ferry", Some(&travel.id));
    let leave = new_task("Ask for leave", Some(&work.id));
    let pack = new_task("Pack", None);
    let plan = note(&store, "# Trip plan");
    for each in [&ferry, &leave, &pack] {
        store.link_note(&each.id, &plan.id).unwrap();
    }
    drop(store);

    let reopened = store_at(&db);
    let tasks = reopened.list_note_tasks(&plan.id, 50, None).unwrap();
    assert_eq!(
        task_titles(&tasks.items),
        ["Pack", "Ask for leave", "Book the ferry"]
    );
    let notes = reopened.list_task_notes(&leave.id, 50, None).unwrap();
    assert_eq!(note_titles(&notes.items), ["Trip plan"]);
}

#[test]
fn linked_notes_come_in_bounded_pages_without_repeats() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    for n in 1..=5 {
        let each = note(&store, &format!("Note {n}"));
        store.link_note(&ferry.id, &each.id).unwrap();
    }

    let first = store.list_task_notes(&ferry.id, 2, None).unwrap();
    let second = store
        .list_task_notes(&ferry.id, 2, first.next_cursor.as_deref())
        .unwrap();
    let third = store
        .list_task_notes(&ferry.id, 2, second.next_cursor.as_deref())
        .unwrap();

    assert_eq!(note_titles(&first.items), ["Note 5", "Note 4"]);
    assert_eq!(note_titles(&second.items), ["Note 3", "Note 2"]);
    assert_eq!(note_titles(&third.items), ["Note 1"]);
    assert_eq!(third.next_cursor, None);
    assert!(matches!(
        store.list_task_notes(&ferry.id, 0, None),
        Err(TaskError::InvalidPageSize)
    ));
    assert!(matches!(
        store.list_note_tasks(&ferry.id, 10, Some("n1|5|nope")),
        Err(TaskError::InvalidLinkCursor)
    ));
}

fn version(db: &Path) -> i64 {
    Connection::open(db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// A database exactly as schema v6 left it, holding one Task and one Note.
fn v6_database(db: &Path) -> (Task, Note) {
    let store = store_at(db);
    let kept = (
        task(&store, "Existing Task"),
        note(&store, "# Existing Note"),
    );
    Connection::open(db)
        .unwrap()
        // Undo v7 (links), v8 (folders, pins, Note search) and v9 (Removed
        // indexes): notes back in its v6 shape.
        .execute_batch(
            "DROP INDEX tasks_removed_order; DROP INDEX lists_removed_order;
             DROP TABLE task_note_links; DROP TABLE notes_fts;
             CREATE TABLE notes_v6 (
                id TEXT PRIMARY KEY NOT NULL,
                markdown TEXT NOT NULL,
                title TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             INSERT INTO notes_v6 SELECT id, markdown, title, revision, created_at, updated_at FROM notes;
             DROP TABLE notes; ALTER TABLE notes_v6 RENAME TO notes;
             CREATE INDEX notes_updated_order ON notes(updated_at DESC, id DESC);
             DROP TABLE note_folders;
             DROP TABLE IF EXISTS audit_entries; DROP TABLE IF EXISTS bulk_requests;
             PRAGMA user_version = 6;",
        )
        .unwrap();
    kept
}

#[test]
fn a_v6_database_gains_links_without_touching_its_tasks_or_notes() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v6.db");
    let (old_task, old_note) = v6_database(&db);

    let store = store_at(&db);

    assert_eq!(version(&db), tasknotes_core::tasks::SCHEMA_VERSION);
    assert_eq!(store.get_task(&old_task.id).unwrap().unwrap(), old_task);
    assert_eq!(store.get_note(&old_note.id).unwrap().unwrap(), old_note);
    store.link_note(&old_task.id, &old_note.id).unwrap();
    let notes = store.list_task_notes(&old_task.id, 50, None).unwrap();
    assert_eq!(note_titles(&notes.items), ["Existing Note"]);
}

#[test]
fn a_failed_v7_migration_rolls_back_and_keeps_the_v6_database_usable() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("collision.db");
    let (old_task, old_note) = v6_database(&db);
    // The links table is created first; the second index name then collides.
    Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE task_note_links_note_order(collision INTEGER);")
        .unwrap();

    assert!(TaskStore::open(&db).is_err());
    assert_eq!(version(&db), 6);
    let half_done: bool = Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'task_note_links')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !half_done,
        "the links table must roll back with the failed migration"
    );

    Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TABLE task_note_links_note_order;")
        .unwrap();
    let store = store_at(&db);
    assert_eq!(version(&db), tasknotes_core::tasks::SCHEMA_VERSION);
    assert_eq!(store.get_task(&old_task.id).unwrap().unwrap(), old_task);
    assert_eq!(store.get_note(&old_note.id).unwrap().unwrap(), old_note);
}

#[test]
fn linking_an_unlinked_pair_again_is_a_fresh_link_first_in_the_list() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan");
    let budget = note(&store, "# Budget");
    let old = store.link_note(&ferry.id, &plan.id).unwrap();
    store.link_note(&ferry.id, &budget.id).unwrap();
    store.unlink_note(&old.id).unwrap();

    let again = store.link_note(&ferry.id, &plan.id).unwrap();

    assert_eq!(again.id, old.id, "one row per pair, ever");
    assert!(again.linked_at > old.linked_at);
    let notes = store.list_task_notes(&ferry.id, 50, None).unwrap();
    assert_eq!(note_titles(&notes.items), ["Trip plan", "Budget"]);
}

#[test]
fn undo_restores_the_original_place_and_refuses_unknown_links() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("links.db"));
    let ferry = task(&store, "Book the ferry");
    let plan = note(&store, "# Trip plan");
    let budget = note(&store, "# Budget");
    let old = store.link_note(&ferry.id, &plan.id).unwrap();
    store.link_note(&ferry.id, &budget.id).unwrap();
    store.unlink_note(&old.id).unwrap();

    assert_eq!(store.restore_link(&old.id).unwrap(), old);
    let notes = store.list_task_notes(&ferry.id, 50, None).unwrap();
    assert_eq!(note_titles(&notes.items), ["Budget", "Trip plan"]);
    // Restoring a link that is already active changes nothing.
    assert_eq!(store.restore_link(&old.id).unwrap(), old);
    assert!(matches!(
        store.restore_link("00000000-0000-4000-8000-000000000000"),
        Err(TaskError::LinkNotFound)
    ));
}

#[test]
fn a_note_lists_no_task_hidden_by_removed_and_shows_it_again_on_restore() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("links.db");
    let store = store_at(&db);
    let travel = store
        .create_list(CreateList {
            name: "Travel".into(),
        })
        .unwrap();
    let ferry = store
        .create_task_in_list(
            CreateTask {
                title: "Book the ferry".into(),
                description: String::new(),
            },
            Some(&travel.id),
        )
        .unwrap();
    let pack = task(&store, "Pack");
    let plan = note(&store, "# Trip plan");
    let link = store.link_note(&ferry.id, &plan.id).unwrap();
    store.link_note(&pack.id, &plan.id).unwrap();

    let removed = store
        .set_list_removed(SetListRemoved {
            id: travel.id.clone(),
            expected_revision: travel.revision,
            removed: true,
        })
        .unwrap();
    let tasks = store.list_note_tasks(&plan.id, 50, None).unwrap();
    assert_eq!(task_titles(&tasks.items), ["Pack"]);
    assert_eq!(
        store.get_link(&link.id).unwrap(),
        Some(link.clone()),
        "the link row stays"
    );

    // A Task Removed on its own (Ticket 09) is left out the same way.
    other_writer(&db)
        .execute("UPDATE tasks SET removed_at = 1 WHERE id = ?1", [&pack.id])
        .unwrap();
    assert!(
        store
            .list_note_tasks(&plan.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );

    store
        .set_list_removed(SetListRemoved {
            id: travel.id,
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();
    let tasks = store.list_note_tasks(&plan.id, 50, None).unwrap();
    assert_eq!(task_titles(&tasks.items), ["Book the ferry"]);
}
