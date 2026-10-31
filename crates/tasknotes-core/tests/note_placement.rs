use std::path::Path;

use tasknotes_core::note_folders::{CreateNoteFolder, NoteFolder};
use tasknotes_core::notes::{
    CreateNote, MoveNote, Note, NotePage, NoteQuery, NoteScope, SetNotePinned, UpdateNote,
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

fn note_in(store: &TaskStore, markdown: &str, folder: Option<&NoteFolder>) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: folder.map(|f| f.id.clone()),
        })
        .unwrap()
}

fn scope(folder: Option<&NoteFolder>) -> NoteQuery {
    NoteQuery {
        folder: folder.map_or(NoteScope::All, |f| NoteScope::Folder(f.id.clone())),
        ..NoteQuery::default()
    }
}

fn titles(page: &NotePage) -> Vec<&str> {
    page.items.iter().map(|item| item.title.as_str()).collect()
}

fn listed(store: &TaskStore, folder: Option<&NoteFolder>) -> Vec<String> {
    let page = store.query_notes(&scope(folder), 100, None).unwrap();
    titles(&page).into_iter().map(str::to_owned).collect()
}

fn pin(store: &TaskStore, note: &Note, pinned: bool) -> Note {
    store
        .set_note_pinned(SetNotePinned {
            id: note.id.clone(),
            pinned,
        })
        .unwrap()
}

#[test]
fn a_folder_lists_only_the_notes_directly_in_it_and_all_notes_lists_every_note() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("folders.db");
    let store = store_at(&db);
    let work = folder(&store, "Work", None);
    let meetings = folder(&store, "Meetings", Some(&work));
    note_in(&store, "Loose idea", None);
    note_in(&store, "Quarterly plan", Some(&work));
    let standup = note_in(&store, "Standup notes", Some(&meetings));
    assert_eq!(standup.folder_id.as_deref(), Some(meetings.id.as_str()));

    let reopened = store_at(&db);
    assert_eq!(listed(&reopened, Some(&work)), vec!["Quarterly plan"]);
    assert_eq!(listed(&reopened, Some(&meetings)), vec!["Standup notes"]);
    assert_eq!(
        listed(&reopened, None),
        vec!["Standup notes", "Quarterly plan", "Loose idea"]
    );
    let summary = &reopened
        .query_notes(&scope(Some(&meetings)), 10, None)
        .unwrap()
        .items[0];
    assert_eq!(summary.folder_id.as_deref(), Some(meetings.id.as_str()));
    assert!(summary.snippet.is_none());

    let missing = store.create_note(CreateNote {
        markdown: "Nowhere".into(),
        folder_id: Some("00000000-0000-4000-8000-000000000000".into()),
    });
    assert!(matches!(missing, Err(TaskError::NoteFolderNotFound)));
    assert!(matches!(
        store.query_notes(
            &NoteQuery {
                folder: NoteScope::Folder("not-a-folder".into()),
                ..NoteQuery::default()
            },
            10,
            None
        ),
        Err(TaskError::NoteFolderNotFound)
    ));
}

#[test]
fn moving_a_note_changes_its_folder_but_not_its_text_revision_or_edit_time() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("move.db"));
    let work = folder(&store, "Work", None);
    let home = folder(&store, "Home", None);
    let note = note_in(&store, "Receipts", Some(&work));

    let moved = store
        .move_note(MoveNote {
            id: note.id.clone(),
            folder_id: Some(home.id.clone()),
            expected_revision: None,
        })
        .unwrap();
    assert_eq!(moved.folder_id.as_deref(), Some(home.id.clone()).as_deref());
    assert_eq!(
        (moved.revision, moved.updated_at, moved.markdown.as_str()),
        (note.revision, note.updated_at, "Receipts")
    );
    assert!(listed(&store, Some(&work)).is_empty());
    assert_eq!(listed(&store, Some(&home)), vec!["Receipts"]);

    // An editor still holding the original revision saves without a conflict.
    store
        .update_note(UpdateNote {
            id: note.id.clone(),
            markdown: "Receipts for March".into(),
            expected_revision: note.revision,
        })
        .unwrap();

    let out = store
        .move_note(MoveNote {
            id: note.id.clone(),
            folder_id: None,
            expected_revision: None,
        })
        .unwrap();
    assert_eq!(out.folder_id, None);
    assert!(matches!(
        store.move_note(MoveNote {
            id: note.id.clone(),
            folder_id: Some("00000000-0000-4000-8000-000000000000".into()),
            expected_revision: None,
        }),
        Err(TaskError::NoteFolderNotFound)
    ));
    assert!(matches!(
        store.move_note(MoveNote {
            id: "00000000-0000-4000-8000-000000000000".into(),
            folder_id: None,
            expected_revision: None,
        }),
        Err(TaskError::NoteNotFound)
    ));
}

#[test]
fn pinned_notes_come_first_and_pinning_is_kept_without_counting_as_an_edit() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("pins.db");
    let store = store_at(&db);
    let work = folder(&store, "Work", None);
    let oldest = note_in(&store, "Oldest", Some(&work));
    let middle = note_in(&store, "Middle", Some(&work));
    note_in(&store, "Newest", Some(&work));

    let pinned = pin(&store, &oldest, true);
    assert!(pinned.pinned);
    assert_eq!(
        (pinned.revision, pinned.updated_at),
        (oldest.revision, oldest.updated_at)
    );
    pin(&store, &middle, true);
    // Pinning twice is harmless.
    pin(&store, &middle, true);

    let reopened = store_at(&db);
    // Pinned first, each group most recently edited first.
    assert_eq!(
        listed(&reopened, Some(&work)),
        vec!["Middle", "Oldest", "Newest"]
    );
    assert_eq!(listed(&reopened, None), vec!["Middle", "Oldest", "Newest"]);
    let first = &reopened.query_notes(&scope(None), 1, None).unwrap().items[0];
    assert!(first.pinned);

    pin(&reopened, &middle, false);
    assert_eq!(
        listed(&reopened, Some(&work)),
        vec!["Oldest", "Newest", "Middle"]
    );
    assert!(matches!(
        reopened.set_note_pinned(SetNotePinned {
            id: "00000000-0000-4000-8000-000000000000".into(),
            pinned: true
        }),
        Err(TaskError::NoteNotFound)
    ));
}

#[test]
fn pages_across_pinned_and_unpinned_notes_never_repeat_or_skip() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("pages.db"));
    let work = folder(&store, "Work", None);
    let mut expected_pinned = Vec::new();
    let mut expected_rest = Vec::new();
    for n in 0..25 {
        let note = note_in(&store, &format!("Note {n:02}"), Some(&work));
        if n % 4 == 0 {
            pin(&store, &note, true);
            expected_pinned.push(note.title);
        } else {
            expected_rest.push(note.title);
        }
    }
    expected_pinned.reverse();
    expected_rest.reverse();
    let expected = [expected_pinned, expected_rest].concat();

    for query in [scope(Some(&work)), scope(None)] {
        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = store.query_notes(&query, 4, cursor.as_deref()).unwrap();
            assert!(page.items.len() <= 4);
            seen.extend(page.items.iter().map(|item| item.title.clone()));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen, expected);
    }

    // A cursor is valid only for the query that produced it.
    let all = store.query_notes(&scope(None), 4, None).unwrap();
    assert!(matches!(
        store.query_notes(&scope(Some(&work)), 4, all.next_cursor.as_deref()),
        Err(TaskError::InvalidNoteCursor)
    ));
    assert!(matches!(
        store.query_notes(&scope(None), 101, None),
        Err(TaskError::InvalidPageSize)
    ));
}

#[test]
fn a_move_naming_a_stale_revision_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let work = folder(&store, "Work", None);
    let note = note_in(&store, "# Synthetic agenda", None);
    let edited = store
        .update_note(UpdateNote {
            id: note.id.clone(),
            markdown: "# Synthetic agenda, revised".into(),
            expected_revision: note.revision,
        })
        .unwrap();
    let stale = store.move_note(MoveNote {
        id: note.id.clone(),
        folder_id: Some(work.id.clone()),
        expected_revision: Some(note.revision),
    });
    assert!(matches!(stale, Err(TaskError::NoteRevisionConflict)));
    assert_eq!(store.get_note(&note.id).unwrap().unwrap().folder_id, None);
    let moved = store
        .move_note(MoveNote {
            id: note.id.clone(),
            folder_id: Some(work.id.clone()),
            expected_revision: Some(edited.revision),
        })
        .unwrap();
    assert_eq!(moved.folder_id, Some(work.id));
    assert_eq!(
        moved.revision, edited.revision,
        "placement keeps the revision"
    );
}
