//! Manual sidebar order (ADR-0007): Lists and Note Folders stay where the
//! person put them. Synthetic data only.

use std::path::Path;

use rusqlite::{Connection, params};

use tasknotes_core::note_folders::{
    CreateNoteFolder, MAX_FOLDER_DEPTH, MoveNoteFolder, NoteFolder, PlaceNoteFolder,
    RenameNoteFolder,
};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    CreateList, List, RenameList, ReorderList, SCHEMA_VERSION, SetListRemoved, TaskError, TaskStore,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn list_names(store: &TaskStore) -> Vec<String> {
    store
        .list_lists(100, None, false)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.name)
        .collect()
}

fn current_list(store: &TaskStore, id: &str) -> List {
    store
        .list_lists(100, None, true)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .unwrap()
}

fn reorder(store: &TaskStore, item: &List, after: Option<&List>) -> Result<List, TaskError> {
    store.reorder_list(ReorderList {
        id: item.id.clone(),
        expected_revision: current_list(store, &item.id).revision,
        after_id: after.map(|a| a.id.clone()),
    })
}

fn folder(store: &TaskStore, name: &str, parent: Option<&NoteFolder>) -> NoteFolder {
    store
        .create_note_folder(CreateNoteFolder {
            name: name.into(),
            parent_id: parent.map(|p| p.id.clone()),
        })
        .unwrap()
}

fn folder_names(store: &TaskStore, parent: Option<&NoteFolder>) -> Vec<String> {
    store
        .list_note_folders(parent.map(|p| p.id.as_str()), 100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.name)
        .collect()
}

fn place(
    store: &TaskStore,
    item: &NoteFolder,
    parent: Option<&NoteFolder>,
    after: Option<&NoteFolder>,
) -> Result<NoteFolder, TaskError> {
    store.place_note_folder(PlaceNoteFolder {
        id: item.id.clone(),
        expected_revision: store.get_note_folder(&item.id).unwrap().unwrap().revision,
        parent_id: parent.map(|p| p.id.clone()),
        after_id: after.map(|a| a.id.clone()),
    })
}

/// Puts a database back exactly as schema v15 left it, for the upgrade tests.
fn back_to_v15(db: &Path) {
    let conn = Connection::open(db).unwrap();
    conn.execute_batch(
        "DROP INDEX lists_manual_order;
         DROP INDEX note_folders_manual_order;
         ALTER TABLE lists DROP COLUMN position;
         ALTER TABLE note_folders DROP COLUMN position;
         CREATE INDEX lists_created_order ON lists(created_at DESC, id DESC);
         PRAGMA user_version = 15;",
    )
    .unwrap();
}

#[test]
fn upgrading_from_v15_keeps_every_list_in_the_order_it_showed_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade-lists.db");
    let store = store_at(&db);
    let oldest = list(&store, "Oldest");
    let middle = list(&store, "Middle");
    let hidden = list(&store, "In Removed");
    let newest = list(&store, "Newest");
    store
        .set_list_removed(SetListRemoved {
            id: hidden.id.clone(),
            expected_revision: hidden.revision,
            removed: true,
        })
        .unwrap();
    drop(store);
    back_to_v15(&db);
    {
        let conn = Connection::open(&db).unwrap();
        for (item, created_at) in [
            (&oldest, 100),
            (&middle, 200),
            (&hidden, 300),
            (&newest, 400),
        ] {
            conn.execute(
                "UPDATE lists SET created_at = ?1 WHERE id = ?2",
                params![created_at, item.id],
            )
            .unwrap();
        }
    }
    let before: Vec<(String, i64, i64)> = {
        let conn = Connection::open(&db).unwrap();
        let mut statement = conn
            .prepare("SELECT id, revision, updated_at FROM lists ORDER BY id")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };

    let store = store_at(&db);

    assert_eq!(list_names(&store), ["Newest", "Middle", "Oldest"]);
    let all: Vec<String> = store
        .list_lists(100, None, true)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.name)
        .collect();
    assert_eq!(all, ["Newest", "In Removed", "Middle", "Oldest"]);
    let conn = Connection::open(&db).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    // Upgrading changes nothing anyone made: no revision or time moves.
    let mut statement = conn
        .prepare("SELECT id, revision, updated_at FROM lists ORDER BY id")
        .unwrap();
    let after: Vec<(String, i64, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(after, before);
}

#[test]
fn upgrading_from_v15_keeps_each_folders_children_in_name_order() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade-folders.db");
    let store = store_at(&db);
    let zebra = folder(&store, "Zebra", None);
    folder(&store, "apple", None);
    folder(&store, "Mango", None);
    folder(&store, "two", Some(&zebra));
    folder(&store, "One", Some(&zebra));
    drop(store);
    back_to_v15(&db);

    let store = store_at(&db);

    assert_eq!(folder_names(&store, None), ["apple", "Mango", "Zebra"]);
    assert_eq!(folder_names(&store, Some(&zebra)), ["One", "two"]);
}

#[test]
fn a_new_list_or_folder_lands_at_the_bottom() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("new.db"));
    list(&store, "First");
    list(&store, "Second");
    list(&store, "Third");
    assert_eq!(list_names(&store), ["First", "Second", "Third"]);

    let parent = folder(&store, "Zulu", None);
    folder(&store, "Alpha", None);
    folder(&store, "Yankee", Some(&parent));
    folder(&store, "Bravo", Some(&parent));
    assert_eq!(folder_names(&store, None), ["Zulu", "Alpha"]);
    assert_eq!(folder_names(&store, Some(&parent)), ["Yankee", "Bravo"]);
}

#[test]
fn reordering_a_list_survives_reopening_and_only_the_moved_list_gets_a_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("reorder.db");
    let store = store_at(&db);
    let a = list(&store, "A");
    let b = list(&store, "B");
    let c = list(&store, "C");
    let d = list(&store, "D");

    let moved = reorder(&store, &d, Some(&a)).unwrap();
    assert_eq!(moved.revision, d.revision + 1);
    assert!(moved.updated_at > d.updated_at);
    assert_eq!(list_names(&store), ["A", "D", "B", "C"]);

    reorder(&store, &c, None).unwrap();
    assert_eq!(list_names(&store), ["C", "A", "D", "B"]);

    drop(store);
    let reopened = store_at(&db);
    assert_eq!(list_names(&reopened), ["C", "A", "D", "B"]);
    // A neighbour's open rename must not fail because something moved past it.
    assert_eq!(current_list(&reopened, &a.id).revision, a.revision);
    assert_eq!(current_list(&reopened, &b.id).revision, b.revision);

    let audit = reopened.list_audit_entries(Some(&d.id), 10, None).unwrap();
    assert!(
        audit
            .items
            .iter()
            .any(|entry| entry.action.as_str() == "list.reorder"
                && entry.outcome.as_str() == "applied"),
        "the move is on record"
    );
}

#[test]
fn a_stale_revision_is_refused_and_leaves_the_order_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("stale.db"));
    let a = list(&store, "A");
    let b = list(&store, "B");
    store
        .rename_list(RenameList {
            id: b.id.clone(),
            expected_revision: b.revision,
            name: "B renamed elsewhere".into(),
        })
        .unwrap();

    let refused = store.reorder_list(ReorderList {
        id: b.id.clone(),
        expected_revision: b.revision,
        after_id: None,
    });

    assert!(matches!(refused, Err(TaskError::RevisionConflict)));
    assert_eq!(list_names(&store), ["A", "B renamed elsewhere"]);
    assert_eq!(current_list(&store, &a.id).revision, a.revision);
    let audit = store.list_audit_entries(Some(&b.id), 10, None).unwrap();
    assert!(
        audit
            .items
            .iter()
            .any(|entry| entry.action.as_str() == "list.reorder"
                && entry.outcome.as_str() == "rejected"),
        "the refusal is on record"
    );
}

#[test]
fn two_writers_with_the_same_revision_cannot_both_move_a_list() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("race.db");
    let store = store_at(&db);
    let a = list(&store, "A");
    let b = list(&store, "B");
    let c = list(&store, "C");

    let wins = std::thread::scope(|scope| {
        let writers: Vec<_> = [None, Some(a.id.clone())]
            .into_iter()
            .map(|after_id| {
                let db = &db;
                let id = c.id.clone();
                let expected_revision = c.revision;
                scope.spawn(move || {
                    store_at(db)
                        .reorder_list(ReorderList {
                            id,
                            expected_revision,
                            after_id,
                        })
                        .is_ok()
                })
            })
            .collect();
        writers
            .into_iter()
            .map(|writer| writer.join().unwrap())
            .filter(|won| *won)
            .count()
    });

    assert_eq!(wins, 1);
    assert_eq!(current_list(&store, &c.id).revision, c.revision + 1);
    assert_eq!(current_list(&store, &b.id).revision, b.revision);
}

#[test]
fn placing_after_a_list_that_is_gone_says_the_view_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("anchor.db"));
    let a = list(&store, "A");
    let b = list(&store, "B");
    let c = list(&store, "C");
    let removed = store
        .set_list_removed(SetListRemoved {
            id: a.id.clone(),
            expected_revision: a.revision,
            removed: true,
        })
        .unwrap();

    let after_removed = reorder(&store, &c, Some(&removed)).unwrap_err();
    assert!(matches!(after_removed, TaskError::OrderAnchorChanged));
    assert_eq!(after_removed.code(), "revision_conflict");
    let unknown = store.reorder_list(ReorderList {
        id: c.id.clone(),
        expected_revision: c.revision,
        after_id: Some("00000000-0000-4000-8000-000000000000".into()),
    });
    assert!(matches!(unknown, Err(TaskError::OrderAnchorChanged)));
    assert_eq!(list_names(&store), ["B", "C"]);
    assert_eq!(current_list(&store, &b.id).revision, b.revision);
}

#[test]
fn a_move_that_changes_nothing_shown_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("unchanged.db"));
    let a = list(&store, "A");
    let hidden = list(&store, "Hidden");
    let c = list(&store, "C");
    store
        .set_list_removed(SetListRemoved {
            id: hidden.id.clone(),
            expected_revision: hidden.revision,
            removed: true,
        })
        .unwrap();

    assert!(matches!(
        reorder(&store, &a, None),
        Err(TaskError::ListUnchanged)
    ));
    // A List in Removed between the two is not something the person sees.
    assert!(matches!(
        reorder(&store, &c, Some(&a)),
        Err(TaskError::ListUnchanged)
    ));
    assert!(matches!(
        reorder(&store, &c, Some(&c)),
        Err(TaskError::ListUnchanged)
    ));
    assert_eq!(current_list(&store, &c.id).revision, c.revision);
}

#[test]
fn a_removed_list_cannot_be_moved_and_comes_back_where_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("removed.db"));
    let a = list(&store, "A");
    let b = list(&store, "B");
    let c = list(&store, "C");
    let d = list(&store, "D");
    let removed = store
        .set_list_removed(SetListRemoved {
            id: b.id.clone(),
            expected_revision: b.revision,
            removed: true,
        })
        .unwrap();

    assert!(matches!(
        reorder(&store, &removed, Some(&a)),
        Err(TaskError::ListRemoved)
    ));
    reorder(&store, &d, None).unwrap();
    assert_eq!(list_names(&store), ["D", "A", "C"]);

    store
        .set_list_removed(SetListRemoved {
            id: b.id.clone(),
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();

    assert_eq!(list_names(&store), ["D", "A", "B", "C"]);
    assert_eq!(current_list(&store, &c.id).revision, c.revision);
}

#[test]
fn list_pages_walk_the_manual_order_without_repeats_or_gaps() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("pages.db"));
    let items: Vec<List> = (0..7).map(|n| list(&store, &format!("L{n}"))).collect();
    reorder(&store, &items[6], None).unwrap();
    reorder(&store, &items[0], Some(&items[3])).unwrap();
    let expected = list_names(&store);

    let mut walked = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = store.list_lists(3, cursor.as_deref(), false).unwrap();
        walked.extend(page.items.into_iter().map(|item| item.name));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    assert_eq!(walked, expected);
    assert_eq!(expected[0], "L6");
    assert!(matches!(
        store.list_lists(
            3,
            Some("v2|1|00000000-0000-4000-8000-000000000000|active"),
            false
        ),
        Err(TaskError::InvalidListCursor)
    ));
}

#[test]
fn folders_reorder_among_siblings_and_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("folders.db");
    let store = store_at(&db);
    let a = folder(&store, "A", None);
    let b = folder(&store, "B", None);
    let c = folder(&store, "C", None);

    let moved = place(&store, &c, None, Some(&a)).unwrap();
    assert_eq!(moved.revision, c.revision + 1);
    assert_eq!(folder_names(&store, None), ["A", "C", "B"]);
    place(&store, &b, None, None).unwrap();
    assert_eq!(folder_names(&store, None), ["B", "A", "C"]);

    assert!(matches!(
        place(&store, &b, None, None),
        Err(TaskError::NoteFolderUnchanged)
    ));

    store
        .rename_note_folder(RenameNoteFolder {
            id: a.id.clone(),
            expected_revision: a.revision,
            name: "Zzz".into(),
        })
        .unwrap();
    assert_eq!(
        folder_names(&store, None),
        ["B", "Zzz", "C"],
        "a new name does not move a folder"
    );

    drop(store);
    assert_eq!(folder_names(&store_at(&db), None), ["B", "Zzz", "C"]);
}

#[test]
fn placing_a_folder_under_another_parent_changes_parent_and_order_together() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("parents.db"));
    let work = folder(&store, "Work", None);
    let home = folder(&store, "Home", None);
    let one = folder(&store, "One", Some(&work));
    folder(&store, "Two", Some(&work));

    let moved = place(&store, &home, Some(&work), Some(&one)).unwrap();

    assert_eq!(moved.parent_id.as_deref(), Some(work.id.as_str()));
    assert_eq!(folder_names(&store, None), ["Work"]);
    assert_eq!(folder_names(&store, Some(&work)), ["One", "Home", "Two"]);
    let audit = store.list_audit_entries(Some(&home.id), 10, None).unwrap();
    assert!(
        audit
            .items
            .iter()
            .any(|entry| entry.action.as_str() == "folder.move"),
        "a change of parent is recorded as a move"
    );

    // Back out to the top, first in line.
    place(&store, &home, None, None).unwrap();
    assert_eq!(folder_names(&store, None), ["Home", "Work"]);
    assert_eq!(folder_names(&store, Some(&work)), ["One", "Two"]);
}

#[test]
fn placing_a_folder_keeps_every_tree_rule() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("rules.db"));
    let top = folder(&store, "Top", None);
    let inner = folder(&store, "Inner", Some(&top));
    let other = folder(&store, "Other", None);
    let clash = folder(&store, "Inner", Some(&other));

    // Into its own subtree.
    assert!(matches!(
        place(&store, &top, Some(&inner), None),
        Err(TaskError::NoteFolderCycle)
    ));
    // Over a sibling's name in the new parent.
    assert!(matches!(
        place(&store, &clash, Some(&top), None),
        Err(TaskError::NoteFolderNameConflict)
    ));
    // After a folder that lives somewhere else.
    let anchored = place(&store, &other, None, Some(&inner)).unwrap_err();
    assert!(matches!(anchored, TaskError::OrderAnchorChanged));

    // Below level 5.
    let mut deep = top.clone();
    let chain: Vec<NoteFolder> = (2..=MAX_FOLDER_DEPTH)
        .map(|level| {
            deep = folder(&store, &format!("Level {level}"), Some(&deep));
            deep.clone()
        })
        .collect();
    assert!(matches!(
        place(&store, &other, chain.last(), None),
        Err(TaskError::NoteFolderTooDeep)
    ));

    // A stale revision.
    let stale = store.place_note_folder(PlaceNoteFolder {
        id: other.id.clone(),
        expected_revision: other.revision + 7,
        parent_id: None,
        after_id: None,
    });
    assert!(matches!(stale, Err(TaskError::NoteFolderRevisionConflict)));

    // Into a folder in Removed.
    let removed = folder(&store, "Gone", None);
    store
        .set_note_folder_removed(SetRemoved {
            id: removed.id.clone(),
            expected_revision: removed.revision,
            removed: true,
        })
        .unwrap();
    assert!(matches!(
        place(&store, &other, Some(&removed), None),
        Err(TaskError::NoteFolderRemoved)
    ));

    assert_eq!(folder_names(&store, None), ["Top", "Other"]);
}

#[test]
fn moving_a_folder_to_another_parent_lands_it_at_the_bottom() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("move.db"));
    let work = folder(&store, "Work", None);
    let home = folder(&store, "Aaa", None);
    folder(&store, "Zed", Some(&work));

    store
        .move_note_folder(MoveNoteFolder {
            id: home.id.clone(),
            expected_revision: home.revision,
            parent_id: Some(work.id.clone()),
        })
        .unwrap();

    assert_eq!(folder_names(&store, Some(&work)), ["Zed", "Aaa"]);
}

#[test]
fn folder_pages_walk_the_manual_order_without_repeats_or_gaps() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("folder-pages.db"));
    let items: Vec<NoteFolder> = (0..5)
        .map(|n| folder(&store, &format!("F{n}"), None))
        .collect();
    place(&store, &items[4], None, None).unwrap();
    let expected = folder_names(&store, None);

    let mut walked = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = store.list_note_folders(None, 2, cursor.as_deref()).unwrap();
        walked.extend(page.items.into_iter().map(|item| item.name));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    assert_eq!(walked, expected);
    assert_eq!(expected[0], "F4");
}
