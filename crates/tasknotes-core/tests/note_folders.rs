use std::path::Path;

use rusqlite::Connection;

use tasknotes_core::note_folders::{
    CreateNoteFolder, MoveNoteFolder, NoteFolder, RenameNoteFolder,
};
use tasknotes_core::tasks::{TaskError, TaskStore};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn folder(store: &TaskStore, name: &str, parent: Option<&NoteFolder>) -> NoteFolder {
    store
        .create_note_folder(CreateNoteFolder {
            name: name.into(),
            parent_id: parent.map(|p| p.id.clone()),
        })
        .unwrap()
}

fn names(store: &TaskStore, parent: Option<&NoteFolder>) -> Vec<String> {
    store
        .list_note_folders(parent.map(|p| p.id.as_str()), 100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|f| f.name)
        .collect()
}

fn version(db: &Path) -> i64 {
    Connection::open(db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// A database exactly as schema v6 left it, holding one Note.
fn v6_database(db: &Path) {
    drop(store_at(db));
    Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP TABLE task_note_links;
             DROP TRIGGER notes_fts_insert; DROP TRIGGER notes_fts_update;
             DROP TRIGGER notes_fts_delete; DROP TABLE notes_fts;
             DROP INDEX notes_all_order; DROP INDEX notes_folder_order;
             DROP TABLE notes; DROP TABLE note_folders;
             CREATE TABLE notes (
                id TEXT PRIMARY KEY NOT NULL,
                markdown TEXT NOT NULL,
                title TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE INDEX notes_updated_order ON notes(updated_at DESC, id DESC);
             INSERT INTO notes VALUES ('00000000-0000-4000-8000-0000000000aa',
               '# Packing list\n\nPassport and tickets', 'Packing list', 3, 10, 20);
             PRAGMA user_version = 6;",
        )
        .unwrap();
}

#[test]
fn a_v6_database_keeps_its_notes_and_gains_folders() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v6.db");
    v6_database(&db);

    let store = store_at(&db);
    assert_eq!(version(&db), 8);
    let kept = store
        .get_note("00000000-0000-4000-8000-0000000000aa")
        .unwrap()
        .unwrap();
    assert_eq!(
        (kept.title.as_str(), kept.revision, kept.updated_at),
        ("Packing list", 3, 20)
    );
    assert_eq!(kept.folder_id, None);
    assert!(!kept.pinned);
    // Notes written before v8 are searchable straight away.
    let hits = store
        .query_notes(
            &tasknotes_core::notes::NoteQuery {
                text: "passport".into(),
                ..Default::default()
            },
            10,
            None,
        )
        .unwrap();
    assert_eq!(hits.items.len(), 1);
    assert_eq!(folder(&store, "Travel", None).name, "Travel");
}

#[test]
fn a_failed_v8_migration_rolls_back_and_keeps_the_database_usable() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("collision.db");
    v6_database(&db);
    // The folders table is created first; the FTS table name then collides.
    Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE notes_fts(collision INTEGER);")
        .unwrap();

    assert!(TaskStore::open(&db).is_err());
    // The v7 step (Task–Note links) commits on its own; only v8 rolls back.
    assert_eq!(version(&db), 7);
    let half_done: bool = Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'note_folders')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!half_done, "note_folders must roll back with the migration");

    Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TABLE notes_fts;")
        .unwrap();
    let store = store_at(&db);
    assert_eq!(version(&db), 8);
    assert!(
        store
            .get_note("00000000-0000-4000-8000-0000000000aa")
            .unwrap()
            .is_some()
    );
}

#[test]
fn folders_nest_and_the_tree_survives_reopening_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("tree.db");
    let store = store_at(&db);
    let work = folder(&store, "Work", None);
    folder(&store, "Home", None);
    let meetings = folder(&store, "Meetings", Some(&work));
    folder(&store, "2026", Some(&meetings));
    folder(&store, "Archive", Some(&work));

    let reopened = store_at(&db);
    // Siblings sort by name, case-insensitively.
    assert_eq!(names(&reopened, None), vec!["Home", "Work"]);
    assert_eq!(names(&reopened, Some(&work)), vec!["Archive", "Meetings"]);
    assert_eq!(names(&reopened, Some(&meetings)), vec!["2026"]);

    let top = reopened.list_note_folders(None, 100, None).unwrap().items;
    let work_row = top.iter().find(|f| f.name == "Work").unwrap();
    assert_eq!(work_row.parent_id, None);
    assert!(work_row.has_children);
    assert!(!top.iter().find(|f| f.name == "Home").unwrap().has_children);
    let nested = &reopened
        .list_note_folders(Some(&meetings.id), 100, None)
        .unwrap()
        .items[0];
    assert_eq!(nested.parent_id.as_deref(), Some(meetings.id.as_str()));
}

#[test]
fn sibling_names_are_unique_ignoring_case_but_may_repeat_under_other_parents() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("names.db"));
    let rapat = folder(&store, "Rapat", None);
    let clash = store.create_note_folder(CreateNoteFolder {
        name: "  rapat ".into(),
        parent_id: None,
    });
    assert!(matches!(clash, Err(TaskError::NoteFolderNameConflict)));
    // Unicode case folding, not only ASCII.
    folder(&store, "Één", None);
    assert!(matches!(
        store.create_note_folder(CreateNoteFolder {
            name: "één".into(),
            parent_id: None
        }),
        Err(TaskError::NoteFolderNameConflict)
    ));

    // Full case folding in any Unicode form: ß and ss, final and medial
    // sigma, a precomposed é and e with a combining accent.
    for (first, second) in [
        ("Straße", "STRASSE"),
        ("ΣΑΣ", "σας"),
        ("école", "e\u{301}cole"),
    ] {
        folder(&store, first, None);
        assert!(
            matches!(
                store.create_note_folder(CreateNoteFolder {
                    name: second.into(),
                    parent_id: None
                }),
                Err(TaskError::NoteFolderNameConflict)
            ),
            "{first:?} and {second:?} must clash"
        );
    }

    // The same name under another parent is allowed.
    assert_eq!(folder(&store, "rapat", Some(&rapat)).name, "rapat");
    for bad in ["", "   ", &"x".repeat(101)] {
        assert!(matches!(
            store.create_note_folder(CreateNoteFolder {
                name: bad.into(),
                parent_id: None
            }),
            Err(TaskError::InvalidNoteFolderName)
        ));
    }
    let missing_parent = store.create_note_folder(CreateNoteFolder {
        name: "Orphan".into(),
        parent_id: Some("00000000-0000-4000-8000-000000000000".into()),
    });
    assert!(matches!(missing_parent, Err(TaskError::NoteFolderNotFound)));
}

#[test]
fn a_sixth_level_folder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("depth.db"));
    let mut parent: Option<NoteFolder> = None;
    for level in 1..=5 {
        parent = Some(folder(&store, &format!("Level {level}"), parent.as_ref()));
    }
    let too_deep = store.create_note_folder(CreateNoteFolder {
        name: "Level 6".into(),
        parent_id: parent.as_ref().map(|p| p.id.clone()),
    });
    assert!(matches!(too_deep, Err(TaskError::NoteFolderTooDeep)));
    assert!(names(&store, parent.as_ref()).is_empty());
}

#[test]
fn folder_pages_are_bounded_and_never_repeat_or_skip() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("pages.db"));
    for name in ["delta", "Alpha", "charlie", "Bravo", "echo"] {
        folder(&store, name, None);
    }
    let first = store.list_note_folders(None, 2, None).unwrap();
    let second = store
        .list_note_folders(None, 2, first.next_cursor.as_deref())
        .unwrap();
    let third = store
        .list_note_folders(None, 2, second.next_cursor.as_deref())
        .unwrap();
    let all: Vec<_> = [first.items, second.items, third.items]
        .concat()
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(all, vec!["Alpha", "Bravo", "charlie", "delta", "echo"]);
    assert!(third.next_cursor.is_none());

    assert!(matches!(
        store.list_note_folders(None, 0, None),
        Err(TaskError::InvalidPageSize)
    ));
    assert!(matches!(
        store.list_note_folders(None, 2, Some("junk")),
        Err(TaskError::InvalidNoteFolderCursor)
    ));
    // A cursor from one folder's children is refused for another's.
    let parent = folder(&store, "Parent", None);
    assert!(matches!(
        store.list_note_folders(Some(&parent.id), 2, first.next_cursor.as_deref()),
        Err(TaskError::InvalidNoteFolderCursor)
    ));
}

fn move_to(
    store: &TaskStore,
    folder: &NoteFolder,
    parent: Option<&NoteFolder>,
) -> Result<NoteFolder, TaskError> {
    let current = store.get_note_folder(&folder.id).unwrap().unwrap();
    store.move_note_folder(MoveNoteFolder {
        id: folder.id.clone(),
        expected_revision: current.revision,
        parent_id: parent.map(|p| p.id.clone()),
    })
}

#[test]
fn a_folder_cannot_move_into_itself_or_any_of_its_subfolders() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("cycle.db"));
    let work = folder(&store, "Work", None);
    let projects = folder(&store, "Projects", Some(&work));
    let alpha = folder(&store, "Alpha", Some(&projects));

    for target in [&work, &projects, &alpha] {
        assert!(
            matches!(
                move_to(&store, &work, Some(target)),
                Err(TaskError::NoteFolderCycle)
            ),
            "moving Work under {}",
            target.name
        );
    }
    // The tree is unchanged after the refusals.
    assert_eq!(names(&store, None), vec!["Work"]);
    assert_eq!(names(&store, Some(&projects)), vec!["Alpha"]);

    // Moving a subfolder up, or to the top level, is fine.
    let moved = move_to(&store, &alpha, Some(&work)).unwrap();
    assert_eq!(moved.parent_id.as_deref(), Some(work.id.as_str()));
    assert_eq!(names(&store, Some(&work)), vec!["Alpha", "Projects"]);
    move_to(&store, &projects, None).unwrap();
    assert_eq!(names(&store, None), vec!["Projects", "Work"]);
}

#[test]
fn a_move_is_refused_when_the_moved_subtree_would_pass_five_levels() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("move-depth.db"));
    // A chain three levels deep and a separate subtree two levels tall.
    let one = folder(&store, "One", None);
    let two = folder(&store, "Two", Some(&one));
    let three = folder(&store, "Three", Some(&two));
    let branch = folder(&store, "Branch", None);
    folder(&store, "Leaf", Some(&branch));

    // Branch at level 4 puts Leaf at level 5: allowed.
    move_to(&store, &branch, Some(&three)).unwrap();
    // Back to the top, then under a level-4 folder: Leaf would be level 6.
    move_to(&store, &branch, None).unwrap();
    let four = folder(&store, "Four", Some(&three));
    assert!(matches!(
        move_to(&store, &branch, Some(&four)),
        Err(TaskError::NoteFolderTooDeep)
    ));
    assert_eq!(
        store
            .get_note_folder(&branch.id)
            .unwrap()
            .unwrap()
            .parent_id,
        None
    );
}

#[test]
fn moving_or_renaming_respects_names_at_the_destination_and_the_revision() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("move-names.db"));
    let work = folder(&store, "Work", None);
    let home = folder(&store, "Home", None);
    let work_notes = folder(&store, "Notes", Some(&work));
    folder(&store, "notes", Some(&home));

    assert!(matches!(
        move_to(&store, &work_notes, Some(&home)),
        Err(TaskError::NoteFolderNameConflict)
    ));
    assert!(matches!(
        move_to(&store, &work_notes, Some(&work)),
        Err(TaskError::NoteFolderUnchanged)
    ));

    // A stale revision never overwrites a newer change.
    let renamed = store
        .rename_note_folder(RenameNoteFolder {
            id: work_notes.id.clone(),
            expected_revision: work_notes.revision,
            name: "Meeting notes".into(),
        })
        .unwrap();
    assert_eq!(
        (renamed.name.as_str(), renamed.revision),
        ("Meeting notes", 2)
    );
    let stale = store.move_note_folder(MoveNoteFolder {
        id: work_notes.id.clone(),
        expected_revision: work_notes.revision,
        parent_id: Some(home.id.clone()),
    });
    assert!(matches!(stale, Err(TaskError::NoteFolderRevisionConflict)));
    let stale_rename = store.rename_note_folder(RenameNoteFolder {
        id: work_notes.id.clone(),
        expected_revision: work_notes.revision,
        name: "Other".into(),
    });
    assert!(matches!(
        stale_rename,
        Err(TaskError::NoteFolderRevisionConflict)
    ));

    // Now named differently, it may join Home.
    let moved = move_to(&store, &work_notes, Some(&home)).unwrap();
    assert_eq!(moved.revision, 3);
    assert_eq!(names(&store, Some(&home)), vec!["Meeting notes", "notes"]);

    // Changing only the case of its own name is not a clash.
    let recased = store
        .rename_note_folder(RenameNoteFolder {
            id: home.id.clone(),
            expected_revision: home.revision,
            name: "HOME".into(),
        })
        .unwrap();
    assert_eq!(recased.name, "HOME");
    assert!(matches!(
        store.rename_note_folder(RenameNoteFolder {
            id: home.id.clone(),
            expected_revision: recased.revision,
            name: "work".into(),
        }),
        Err(TaskError::NoteFolderNameConflict)
    ));
}
