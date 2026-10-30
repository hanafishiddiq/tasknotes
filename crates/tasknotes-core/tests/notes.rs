use std::path::Path;

use rusqlite::Connection;

use tasknotes_core::notes::{CreateNote, Note, UpdateNote};
use tasknotes_core::tasks::{CreateTask, Task, TaskError, TaskStore};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn note(store: &TaskStore, markdown: &str) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: None,
        })
        .unwrap()
}

#[test]
fn a_standalone_note_survives_reopening_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("notes.db");
    let markdown = "# Release plan\n\n- [ ] Draft schema\n- [x] Agree table shape";
    let created = note(&store_at(&db), markdown);
    assert_eq!(created.revision, 1);

    let reopened = store_at(&db).get_note(&created.id).unwrap().unwrap();
    assert_eq!(reopened.markdown, markdown);
    assert_eq!(reopened.title, "Release plan");
    assert_eq!(reopened.revision, 1);
    assert_eq!(reopened.created_at, created.created_at);
}

#[test]
fn a_stale_revision_is_a_conflict_and_never_overwrites_the_newer_edit() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("conflict.db"));
    let created = note(&store, "Draft");
    let saved = store
        .update_note(UpdateNote {
            id: created.id.clone(),
            markdown: "Draft from the desk".into(),
            expected_revision: created.revision,
        })
        .unwrap();
    assert_eq!(
        (saved.revision, saved.title.as_str()),
        (2, "Draft from the desk")
    );
    assert!(saved.updated_at >= created.updated_at);

    // A second editor still holds revision 1.
    let stale = store.update_note(UpdateNote {
        id: created.id.clone(),
        markdown: "Draft from the laptop".into(),
        expected_revision: created.revision,
    });
    assert!(matches!(stale, Err(TaskError::NoteRevisionConflict)));
    assert_eq!(
        store.get_note(&created.id).unwrap().unwrap().markdown,
        "Draft from the desk"
    );

    let missing = store.update_note(UpdateNote {
        id: "00000000-0000-4000-8000-000000000000".into(),
        markdown: "Anything".into(),
        expected_revision: 1,
    });
    assert!(matches!(missing, Err(TaskError::NoteNotFound)));
}

fn titles(page: &tasknotes_core::notes::NotePage) -> Vec<&str> {
    page.items.iter().map(|item| item.title.as_str()).collect()
}

#[test]
fn notes_list_most_recently_edited_first_in_bounded_pages() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("list.db"));
    let groceries = note(&store, "Groceries");
    note(&store, "Meeting notes");
    note(&store, "Ideas");
    // Created back to back, often within one millisecond: order must still follow edits.
    store
        .update_note(UpdateNote {
            id: groceries.id.clone(),
            markdown: "Groceries for Sunday".into(),
            expected_revision: groceries.revision,
        })
        .unwrap();

    let first = store.list_notes(2, None).unwrap();
    assert_eq!(titles(&first), vec!["Groceries for Sunday", "Ideas"]);
    let second = store.list_notes(2, first.next_cursor.as_deref()).unwrap();
    assert_eq!(titles(&second), vec!["Meeting notes"]);
    assert!(second.next_cursor.is_none());

    assert!(matches!(
        store.list_notes(0, None),
        Err(TaskError::InvalidPageSize)
    ));
    assert!(matches!(
        store.list_notes(101, None),
        Err(TaskError::InvalidPageSize)
    ));
    assert!(matches!(
        store.list_notes(2, Some("not-a-cursor")),
        Err(TaskError::InvalidNoteCursor)
    ));
}

#[test]
fn notes_written_within_one_millisecond_keep_their_edit_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("burst.db"));
    let written: Vec<String> = (0..30)
        .map(|n| note(&store, &format!("Note {n:02}")).title)
        .collect();
    let listed: Vec<String> = store
        .list_notes(100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.title)
        .collect();
    let newest_first: Vec<String> = written.into_iter().rev().collect();
    assert_eq!(listed, newest_first);
}

#[test]
fn the_title_is_the_first_line_without_markdown_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("titles.db"));
    let long = "Plan ".repeat(30);
    let cut = "Plan ".repeat(20);
    let cases = [
        (
            "\n\n   Groceries for Sunday  \nmilk",
            "Groceries for Sunday",
        ),
        ("### *Quarterly* review\n\nBody", "Quarterly review"),
        (
            "- [ ] Call **Dina** about [the venue](https://example.com/venue)",
            "Call Dina about the venue",
        ),
        ("- [x] Book flights", "Book flights"),
        ("Budget: 5 * 3 = 15", "Budget: 5 * 3 = 15"),
        ("", "New Note"),
        ("  \n\n ", "New Note"),
        // Cut at 100 characters, then without the trailing space.
        (long.as_str(), cut.trim_end()),
    ];
    for (markdown, title) in cases {
        assert_eq!(note(&store, markdown).title, title, "markdown {markdown:?}");
    }
}

#[test]
fn the_title_shows_the_first_line_as_the_editor_does() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("blocks.db"));
    let cases = [
        ("> Quoted line", "Quoted line"),
        ("1. First step\n2. Second step", "First step"),
        ("1. [ ] Not a checkbox", "[ ] Not a checkbox"),
        // Text the editor shows as typed.
        ("#tag notes", "#tag notes"),
        ("1) First step", "1) First step"),
        ("---", "---"),
        // A code block's title is its first line of code.
        ("```ts\nconst ready = true;\n```", "const ready = true;"),
        ("```\n```", "New Note"),
    ];
    for (markdown, title) in cases {
        assert_eq!(note(&store, markdown).title, title, "markdown {markdown:?}");
    }
}

#[test]
fn an_oversized_write_is_refused_and_the_last_saved_version_stays() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("size.db"));
    let largest = "é".repeat(200_000);
    let kept = note(&store, &largest);
    assert_eq!(kept.markdown.chars().count(), 200_000);

    let too_large = format!("{largest}!");
    assert!(matches!(
        store.create_note(CreateNote {
            markdown: too_large.clone(),
            folder_id: None,
        }),
        Err(TaskError::NoteTooLong)
    ));
    assert!(matches!(
        store.update_note(UpdateNote {
            id: kept.id.clone(),
            markdown: too_large,
            expected_revision: kept.revision,
        }),
        Err(TaskError::NoteTooLong)
    ));
    let stored = store.get_note(&kept.id).unwrap().unwrap();
    assert_eq!((stored.revision, stored.markdown), (1, largest));
    assert_eq!(store.list_notes(100, None).unwrap().items.len(), 1);
}

#[test]
fn an_interrupted_save_leaves_the_last_saved_version_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("interrupted.db");
    let store = store_at(&path);
    let kept = note(&store, "Trip plan\n\nBook the train");
    // The write fails after SQLite has started changing the row, as a full
    // disk or an I/O error would.
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch(
        "CREATE TRIGGER interrupt AFTER UPDATE OF revision ON notes
         BEGIN SELECT RAISE(ABORT, 'disk I/O error'); END;",
    )
    .unwrap();

    assert!(
        store
            .update_note(UpdateNote {
                id: kept.id.clone(),
                markdown: "Ferry plan\n\nBook the ferry".into(),
                expected_revision: 1,
            })
            .is_err()
    );
    assert_eq!(store.get_note(&kept.id).unwrap().unwrap(), kept);

    raw.execute_batch("DROP TRIGGER interrupt;").unwrap();
    let saved = store
        .update_note(UpdateNote {
            id: kept.id.clone(),
            markdown: "Ferry plan\n\nBook the ferry".into(),
            expected_revision: 1,
        })
        .unwrap();
    assert_eq!((saved.revision, saved.title.as_str()), (2, "Ferry plan"));
}

/// Later schema steps run straight after v6.
const CURRENT_VERSION: i64 = tasknotes_core::tasks::SCHEMA_VERSION;

fn version(db: &Path) -> i64 {
    Connection::open(db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// A database exactly as schema v5 left it, holding one Task.
fn v5_database(db: &Path) -> Task {
    let task = store_at(db)
        .create_task(CreateTask {
            title: "Existing Task".into(),
            description: "Kept across the upgrade".into(),
        })
        .unwrap();
    Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP INDEX tasks_removed_order; DROP INDEX lists_removed_order;
             DROP TABLE task_note_links; DROP TABLE notes_fts; DROP TABLE notes; DROP TABLE note_folders;
             DROP TABLE IF EXISTS audit_entries; DROP TABLE IF EXISTS bulk_requests;
             PRAGMA user_version = 5;",
        )
        .unwrap();
    task
}

#[test]
fn a_v5_database_gains_notes_without_touching_its_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v5.db");
    let task = v5_database(&db);

    let store = store_at(&db);
    assert_eq!(version(&db), CURRENT_VERSION);
    let kept = store.get_task(&task.id).unwrap().unwrap();
    assert_eq!(
        (
            kept.title.as_str(),
            kept.description.as_str(),
            kept.revision
        ),
        ("Existing Task", "Kept across the upgrade", task.revision)
    );
    assert_eq!(note(&store, "First Note").title, "First Note");
}

#[test]
fn a_failed_v6_migration_rolls_back_and_keeps_the_v5_database_usable() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("collision.db");
    let task = v5_database(&db);
    // The notes table is created first; the index name then collides.
    Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE notes_updated_order(collision INTEGER);")
        .unwrap();

    assert!(TaskStore::open(&db).is_err());
    assert_eq!(version(&db), 5);
    let half_done: bool = Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'notes')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !half_done,
        "the notes table must roll back with the failed migration"
    );

    Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TABLE notes_updated_order;")
        .unwrap();
    let store = store_at(&db);
    assert_eq!(
        store.get_task(&task.id).unwrap().unwrap().title,
        "Existing Task"
    );
    assert_eq!(version(&db), CURRENT_VERSION);
}

#[test]
fn the_title_reads_canonical_markdown_escapes_and_marks_as_plain_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("escapes.db"));
    let cases = [
        // The editor saves literal * _ ` ~ \ with a backslash.
        ("Budget: 5 \\* 3 = 15", "Budget: 5 * 3 = 15"),
        (
            "snake\\_case\\_name and snake_case",
            "snake_case_name and snake_case",
        ),
        ("\\# Not a heading", "# Not a heading"),
        ("C:\\\\Users", "C:\\Users"),
        (
            "~~Old plan~~ replaced, run `pnpm test`",
            "Old plan replaced, run pnpm test",
        ),
        ("_under_ and __strong__", "under and strong"),
        // Escaped brackets are text, not a link.
        ("\\[draft\\](later) notes", "[draft](later) notes"),
    ];
    for (markdown, title) in cases {
        assert_eq!(note(&store, markdown).title, title, "markdown {markdown:?}");
    }
}
