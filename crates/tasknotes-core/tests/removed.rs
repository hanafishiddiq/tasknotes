//! Ticket 09: Removed is a reversible disposition (ADR-0002). Synthetic data only.

use std::path::Path;

use tasknotes_core::note_folders::{
    CreateNoteFolder, MoveNoteFolder, NoteFolder, RenameNoteFolder,
};
use tasknotes_core::notes::{
    CreateNote, MoveNote, Note, NoteQuery, NoteScope, SetNotePinned, UpdateNote,
};
use tasknotes_core::removed::{RemovedKind, SetRemoved};
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, MoveTask, RecordProgress, SetListRemoved, Task,
    TaskError, TaskQuery, TaskStore, UpdateTask, UpdateTaskDetails, WorkStatus,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn task(store: &TaskStore, title: &str) -> Task {
    store
        .create_task(CreateTask {
            title: title.into(),
            description: "Synthetic description".into(),
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

fn set(id: &str, revision: i64, removed: bool) -> SetRemoved {
    SetRemoved {
        id: id.into(),
        expected_revision: revision,
        removed,
    }
}

fn visible_titles(store: &TaskStore, text: &str) -> Vec<String> {
    store
        .query_tasks(
            &TaskQuery {
                text: text.into(),
                ..TaskQuery::default()
            },
            100,
            None,
        )
        .unwrap()
        .items
        .into_iter()
        .map(|t| t.title)
        .collect()
}

#[test]
fn a_removed_task_leaves_views_and_search_and_restore_brings_back_everything() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let started = task(&store, "Draft quarterly report");
    let started = store
        .record_progress(RecordProgress {
            id: started.id.clone(),
            expected_revision: started.revision,
            status: None,
            progress: Some(40),
            next_step: Some("Collect figures".into()),
            note: Some("Outline written".into()),
        })
        .unwrap();
    task(&store, "Water the plants");
    let plan = note(&store, "# Report plan");
    let link = store.link_note(&started.id, &plan.id).unwrap();

    let removed = store
        .set_task_removed(set(&started.id, started.revision, true))
        .unwrap();
    assert!(removed.removed);
    assert_eq!(removed.revision, started.revision + 1);
    assert_eq!(visible_titles(&store, ""), vec!["Water the plants"]);
    assert!(visible_titles(&store, "quarterly").is_empty());
    assert!(visible_titles(&store, "figures").is_empty());
    assert!(
        store
            .list_note_tasks(&plan.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );

    let restored = store
        .set_task_removed(set(&started.id, removed.revision, false))
        .unwrap();
    assert!(!restored.removed);
    assert_eq!(
        (
            restored.title.as_str(),
            restored.description.as_str(),
            restored.status,
            restored.progress,
            restored.next_step.as_str()
        ),
        (
            "Draft quarterly report",
            "Synthetic description",
            WorkStatus::InProgress,
            40,
            "Collect figures"
        )
    );
    assert_eq!(
        visible_titles(&store, "quarterly"),
        vec!["Draft quarterly report"]
    );
    let history = store.list_progress_entries(&started.id, 10, None).unwrap();
    assert_eq!(history.items.len(), 1);
    assert_eq!(history.items[0].note, "Outline written");
    let linked = store.list_note_tasks(&plan.id, 50, None).unwrap().items;
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].link_id, link.id);
}

#[test]
fn removing_a_task_checks_its_revision_and_refuses_a_repeat() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let item = task(&store, "Book the venue");
    assert!(matches!(
        store.set_task_removed(set(&item.id, item.revision + 5, true)),
        Err(TaskError::RevisionConflict)
    ));
    let removed = store
        .set_task_removed(set(&item.id, item.revision, true))
        .unwrap();
    assert!(matches!(
        store.set_task_removed(set(&item.id, removed.revision, true)),
        Err(TaskError::RemovedUnchanged)
    ));
    assert!(matches!(
        store.set_task_removed(set("00000000-0000-4000-8000-000000000001", 1, true)),
        Err(TaskError::NotFound)
    ));
    // Still there, row and all: Removed is never a deletion.
    assert!(store.get_task(&item.id).unwrap().unwrap().removed);
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn set_list(store: &TaskStore, list: &List, removed: bool) -> List {
    store
        .set_list_removed(SetListRemoved {
            id: list.id.clone(),
            expected_revision: list.revision,
            removed,
        })
        .unwrap()
}

fn task_in(store: &TaskStore, title: &str, list: &List) -> Task {
    store
        .create_task_in_list(
            CreateTask {
                title: title.into(),
                description: String::new(),
            },
            Some(&list.id),
        )
        .unwrap()
}

#[test]
fn restoring_a_list_leaves_a_task_removed_on_its_own_in_removed() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let trip = list(&store, "Trip");
    let pack = task_in(&store, "Pack bags", &trip);
    let alone = task_in(&store, "Cancel hotel", &trip);
    let alone = store
        .set_task_removed(set(&alone.id, alone.revision, true))
        .unwrap();

    let trip = set_list(&store, &trip, true);
    assert!(visible_titles(&store, "").is_empty());
    // Removing the List did not touch the child's own disposition.
    assert!(!store.get_task(&pack.id).unwrap().unwrap().removed);
    // A Task Removed on its own cannot come back while its List is in Removed.
    assert!(matches!(
        store.set_task_removed(set(&alone.id, alone.revision, false)),
        Err(TaskError::TaskListRemoved)
    ));

    set_list(&store, &trip, false);
    assert_eq!(visible_titles(&store, ""), vec!["Pack bags"]);
    let back = store
        .set_task_removed(set(&alone.id, alone.revision, false))
        .unwrap();
    assert_eq!(back.list_id.as_deref(), Some(trip.id.as_str()));
    assert_eq!(visible_titles(&store, "hotel"), vec!["Cancel hotel"]);
}

#[test]
fn a_task_in_removed_cannot_be_changed_until_it_is_restored() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let item = task(&store, "Renew passport");
    let item = store
        .set_task_removed(set(&item.id, item.revision, true))
        .unwrap();
    let id = item.id.clone();
    let revision = item.revision;
    let refused = |result: Result<Task, TaskError>| {
        assert!(matches!(result, Err(TaskError::TaskRemoved)), "{result:?}")
    };
    refused(store.update_task(UpdateTask {
        id: id.clone(),
        title: "Changed".into(),
        description: String::new(),
        expected_revision: revision,
    }));
    refused(store.change_task(ChangeTask {
        id: id.clone(),
        expected_revision: revision,
        status: None,
        priority: None,
        progress: Some(50),
    }));
    refused(store.update_task_details(UpdateTaskDetails {
        id: id.clone(),
        expected_revision: revision,
        tags: vec!["errand".into()],
        due_date: None,
    }));
    let other = list(&store, "Errands");
    refused(store.move_task(MoveTask {
        id: id.clone(),
        expected_revision: revision,
        list_id: Some(other.id.clone()),
    }));
    let doc = note(&store, "Passport checklist");
    assert!(matches!(
        store.link_note(&id, &doc.id),
        Err(TaskError::TaskRemoved)
    ));
    let kept = store.get_task(&id).unwrap().unwrap();
    assert_eq!(
        (kept.title.as_str(), kept.progress, kept.revision),
        ("Renew passport", 0, revision)
    );
}

fn version(db: &Path) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// A database exactly as schema v8 left it, holding one Note in one folder.
fn v8_database(db: &Path) -> (String, String) {
    let store = store_at(db);
    let travel = store
        .create_note_folder(CreateNoteFolder {
            name: "Travel".into(),
            parent_id: None,
        })
        .unwrap();
    let packing = store
        .create_note(CreateNote {
            markdown: "# Packing list\n\nPassport".into(),
            folder_id: Some(travel.id.clone()),
        })
        .unwrap();
    drop(store);
    rusqlite::Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP INDEX tasks_removed_order; DROP INDEX notes_removed_order;
             DROP INDEX lists_removed_order; DROP INDEX note_folders_removed_order;
             DROP INDEX note_folders_sibling_name;
             ALTER TABLE notes DROP COLUMN removed_at;
             ALTER TABLE note_folders DROP COLUMN removed_at;
             CREATE UNIQUE INDEX note_folders_sibling_name
               ON note_folders(coalesce(parent_id, ''), name_fold);
             DROP TABLE IF EXISTS audit_entries;
             PRAGMA user_version = 8;",
        )
        .unwrap();
    (travel.id, packing.id)
}

#[test]
fn a_v8_database_keeps_its_notes_and_folders_and_gains_removed() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v8.db");
    let (folder_id, note_id) = v8_database(&db);
    assert_eq!(version(&db), 8);

    let store = store_at(&db);
    assert_eq!(version(&db), tasknotes_core::tasks::SCHEMA_VERSION);
    let kept = store.get_note(&note_id).unwrap().unwrap();
    assert_eq!(kept.folder_id.as_deref(), Some(folder_id.as_str()));
    assert!(!kept.removed);
    assert!(!store.get_note_folder(&folder_id).unwrap().unwrap().removed);
    store
        .set_note_removed(set(&note_id, kept.revision, true))
        .unwrap();
}

#[test]
fn a_failed_v9_migration_rolls_back_and_keeps_the_database_usable() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("collision.db");
    let (_, note_id) = v8_database(&db);
    // The columns are added first; the index name then collides.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE notes_removed_order(collision INTEGER);")
        .unwrap();

    assert!(TaskStore::open(&db).is_err());
    assert_eq!(version(&db), 8);
    let half_done: bool = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('notes') WHERE name = 'removed_at')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !half_done,
        "notes.removed_at must roll back with the migration"
    );

    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TABLE notes_removed_order;")
        .unwrap();
    let store = store_at(&db);
    assert_eq!(version(&db), tasknotes_core::tasks::SCHEMA_VERSION);
    assert!(store.get_note(&note_id).unwrap().is_some());
}

fn note_titles(store: &TaskStore, query: NoteQuery) -> Vec<String> {
    store
        .query_notes(&query, 100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|n| n.title)
        .collect()
}

#[test]
fn a_removed_note_leaves_lists_search_and_links_and_restore_brings_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("n.db"));
    let work = store
        .create_note_folder(CreateNoteFolder {
            name: "Work".into(),
            parent_id: None,
        })
        .unwrap();
    let minutes = store
        .create_note(CreateNote {
            markdown: "# Meeting minutes\n\nBudget agreed".into(),
            folder_id: Some(work.id.clone()),
        })
        .unwrap();
    note(&store, "# Shopping");
    let follow_up = task(&store, "Send follow-up");
    let link = store.link_note(&follow_up.id, &minutes.id).unwrap();

    let removed = store
        .set_note_removed(set(&minutes.id, minutes.revision, true))
        .unwrap();
    assert!(removed.removed);
    assert_eq!(removed.updated_at, minutes.updated_at, "not an edit");
    assert_eq!(note_titles(&store, NoteQuery::default()), vec!["Shopping"]);
    assert!(
        note_titles(
            &store,
            NoteQuery {
                folder: NoteScope::Folder(work.id.clone()),
                ..Default::default()
            }
        )
        .is_empty()
    );
    assert!(
        note_titles(
            &store,
            NoteQuery {
                text: "budget".into(),
                ..Default::default()
            }
        )
        .is_empty()
    );
    assert!(
        store
            .list_task_notes(&follow_up.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );

    let restored = store
        .set_note_removed(set(&minutes.id, removed.revision, false))
        .unwrap();
    assert_eq!(restored.markdown, minutes.markdown);
    assert_eq!(restored.folder_id.as_deref(), Some(work.id.as_str()));
    assert_eq!(
        note_titles(
            &store,
            NoteQuery {
                text: "budget".into(),
                ..Default::default()
            }
        ),
        vec!["Meeting minutes"]
    );
    let linked = store
        .list_task_notes(&follow_up.id, 50, None)
        .unwrap()
        .items;
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].link_id, link.id);
}

#[test]
fn a_note_in_removed_cannot_be_changed_until_it_is_restored() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("n.db"));
    let draft = note(&store, "# Letter");
    assert!(matches!(
        store.set_note_removed(set(&draft.id, draft.revision + 1, true)),
        Err(TaskError::NoteRevisionConflict)
    ));
    let removed = store
        .set_note_removed(set(&draft.id, draft.revision, true))
        .unwrap();
    assert!(matches!(
        store.set_note_removed(set(&draft.id, removed.revision, true)),
        Err(TaskError::RemovedUnchanged)
    ));
    let refused = |result: Result<Note, TaskError>| {
        assert!(matches!(result, Err(TaskError::NoteRemoved)), "{result:?}")
    };
    refused(store.update_note(UpdateNote {
        id: draft.id.clone(),
        markdown: "# Changed".into(),
        expected_revision: removed.revision,
    }));
    refused(store.move_note(MoveNote {
        id: draft.id.clone(),
        folder_id: None,
    }));
    refused(store.set_note_pinned(SetNotePinned {
        id: draft.id.clone(),
        pinned: true,
    }));
    let errand = task(&store, "Post the letter");
    assert!(matches!(
        store.link_note(&errand.id, &draft.id),
        Err(TaskError::NoteRemoved)
    ));
    assert_eq!(
        store.get_note(&draft.id).unwrap().unwrap().markdown,
        "# Letter"
    );
}

fn folder(store: &TaskStore, name: &str, parent: Option<&NoteFolder>) -> NoteFolder {
    store
        .create_note_folder(CreateNoteFolder {
            name: name.into(),
            parent_id: parent.map(|p| p.id.clone()),
        })
        .unwrap()
}

fn note_at(store: &TaskStore, markdown: &str, folder: &NoteFolder) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: Some(folder.id.clone()),
        })
        .unwrap()
}

fn folder_names(store: &TaskStore, parent: Option<&NoteFolder>) -> Vec<String> {
    store
        .list_note_folders(parent.map(|p| p.id.as_str()), 100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|f| f.name)
        .collect()
}

fn all_notes(store: &TaskStore) -> Vec<String> {
    note_titles(store, NoteQuery::default())
}

#[test]
fn removing_a_folder_hides_its_subtree_and_restoring_it_keeps_individually_removed_items_removed() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("f.db"));
    let work = folder(&store, "Work", None);
    let meetings = folder(&store, "Meetings", Some(&work));
    let archive = folder(&store, "Archive", Some(&work));
    let plan = note_at(&store, "# Plan", &work);
    let standup = note_at(&store, "# Standup notes", &meetings);
    let old = note_at(&store, "# Old agenda", &meetings);
    let old = store
        .set_note_removed(set(&old.id, old.revision, true))
        .unwrap();
    let archive = store
        .set_note_folder_removed(set(&archive.id, archive.revision, true))
        .unwrap();
    note(&store, "# Loose note");
    let follow = task(&store, "Send standup summary");
    store.link_note(&follow.id, &standup.id).unwrap();

    let work = store
        .set_note_folder_removed(set(&work.id, work.revision, true))
        .unwrap();
    assert!(work.removed);
    assert_eq!(all_notes(&store), vec!["Loose note"]);
    assert!(folder_names(&store, None).is_empty());
    assert!(matches!(
        store.query_notes(
            &NoteQuery {
                folder: NoteScope::Folder(meetings.id.clone()),
                ..Default::default()
            },
            10,
            None
        ),
        Err(TaskError::NoteFolderRemoved)
    ));
    assert!(
        note_titles(
            &store,
            NoteQuery {
                text: "standup".into(),
                ..Default::default()
            }
        )
        .is_empty()
    );
    assert!(
        store
            .list_task_notes(&follow.id, 50, None)
            .unwrap()
            .items
            .is_empty()
    );
    // Hidden, not Removed: their own disposition is untouched.
    assert!(!store.get_note(&plan.id).unwrap().unwrap().removed);
    assert!(
        !store
            .get_note_folder(&meetings.id)
            .unwrap()
            .unwrap()
            .removed
    );
    // Nothing comes back on its own while the folder above it is in Removed.
    assert!(matches!(
        store.set_note_removed(set(&old.id, old.revision, false)),
        Err(TaskError::NoteFolderRemoved)
    ));
    assert!(matches!(
        store.set_note_folder_removed(set(&archive.id, archive.revision, false)),
        Err(TaskError::NoteFolderRemoved)
    ));

    store
        .set_note_folder_removed(set(&work.id, work.revision, false))
        .unwrap();
    assert_eq!(folder_names(&store, None), vec!["Work"]);
    assert_eq!(folder_names(&store, Some(&work)), vec!["Meetings"]);
    assert_eq!(all_notes(&store).len(), 3);
    assert!(!all_notes(&store).contains(&"Old agenda".to_owned()));
    assert_eq!(
        store
            .list_task_notes(&follow.id, 50, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(store.get_note(&old.id).unwrap().unwrap().removed);
    assert!(store.get_note_folder(&archive.id).unwrap().unwrap().removed);
    // Now they can come back.
    store
        .set_note_removed(set(&old.id, old.revision, false))
        .unwrap();
    store
        .set_note_folder_removed(set(&archive.id, archive.revision, false))
        .unwrap();
    assert_eq!(
        folder_names(&store, Some(&work)),
        vec!["Archive", "Meetings"]
    );
}

#[test]
fn nothing_can_be_put_into_or_changed_inside_a_removed_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("f.db"));
    let home = folder(&store, "Home", None);
    let garden = folder(&store, "Garden", Some(&home));
    let seeds = note_at(&store, "# Seeds", &garden);
    let loose = note(&store, "# Loose");
    let home = store
        .set_note_folder_removed(set(&home.id, home.revision, true))
        .unwrap();
    let refused = |result: Result<(), TaskError>| {
        assert!(
            matches!(result, Err(TaskError::NoteFolderRemoved)),
            "{result:?}"
        )
    };
    refused(unit(store.create_note(CreateNote {
        markdown: "# New".into(),
        folder_id: Some(garden.id.clone()),
    })));
    refused(unit(store.create_note_folder(CreateNoteFolder {
        name: "Beds".into(),
        parent_id: Some(garden.id.clone()),
    })));
    refused(unit(store.move_note(MoveNote {
        id: loose.id.clone(),
        folder_id: Some(garden.id.clone()),
    })));
    refused(unit(store.update_note(UpdateNote {
        id: seeds.id.clone(),
        markdown: "# Seeds 2".into(),
        expected_revision: seeds.revision,
    })));
    refused(unit(store.rename_note_folder(RenameNoteFolder {
        id: garden.id.clone(),
        expected_revision: garden.revision,
        name: "Yard".into(),
    })));
    refused(unit(store.move_note_folder(MoveNoteFolder {
        id: garden.id.clone(),
        expected_revision: garden.revision,
        parent_id: None,
    })));
    refused(unit(store.rename_note_folder(RenameNoteFolder {
        id: home.id.clone(),
        expected_revision: home.revision,
        name: "House".into(),
    })));
    refused(unit(store.list_note_folders(Some(&garden.id), 10, None)));
}

#[test]
fn a_removed_folder_frees_its_name_and_cannot_be_restored_over_an_active_sibling() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("f.db"));
    let first = folder(&store, "Ideas", None);
    assert!(matches!(
        store.set_note_folder_removed(set(&first.id, first.revision + 1, true)),
        Err(TaskError::NoteFolderRevisionConflict)
    ));
    let first = store
        .set_note_folder_removed(set(&first.id, first.revision, true))
        .unwrap();
    assert!(matches!(
        store.set_note_folder_removed(set(&first.id, first.revision, true)),
        Err(TaskError::RemovedUnchanged)
    ));
    let second = folder(&store, "ideas", None);
    assert!(matches!(
        store.set_note_folder_removed(set(&first.id, first.revision, false)),
        Err(TaskError::NoteFolderNameConflict)
    ));
    store
        .rename_note_folder(RenameNoteFolder {
            id: second.id.clone(),
            expected_revision: second.revision,
            name: "Ideas 2".into(),
        })
        .unwrap();
    store
        .set_note_folder_removed(set(&first.id, first.revision, false))
        .unwrap();
    assert_eq!(folder_names(&store, None), vec!["Ideas", "Ideas 2"]);
}

fn unit<T>(result: Result<T, TaskError>) -> Result<(), TaskError> {
    result.map(|_| ())
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(3));
}

#[test]
fn the_removed_view_lists_every_kind_newest_first_in_bounded_pages() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("r.db"));
    let trip = list(&store, "Trip");
    let tickets = task_in(&store, "Buy tickets", &trip);
    task_in(&store, "Pack bags", &trip);
    let draft = note(&store, "# Draft speech");
    let recipes = folder(&store, "Recipes", None);
    note_at(&store, "# Soup", &recipes);
    task(&store, "Still active");

    store
        .set_task_removed(set(&tickets.id, tickets.revision, true))
        .unwrap();
    pause();
    store
        .set_note_removed(set(&draft.id, draft.revision, true))
        .unwrap();
    pause();
    store
        .set_note_folder_removed(set(&recipes.id, recipes.revision, true))
        .unwrap();
    pause();
    set_list(&store, &trip, true);

    let first = store.list_removed("", 3, None).unwrap();
    let seen: Vec<(RemovedKind, &str)> = first
        .items
        .iter()
        .map(|item| (item.kind, item.title.as_str()))
        .collect();
    assert_eq!(
        seen,
        vec![
            (RemovedKind::List, "Trip"),
            (RemovedKind::NoteFolder, "Recipes"),
            (RemovedKind::Note, "Draft speech"),
        ]
    );
    assert!(first.items.iter().all(|item| !item.in_removed_container));
    let rest = store
        .list_removed("", 3, first.next_cursor.as_deref())
        .unwrap();
    assert_eq!(
        rest.items.len(),
        1,
        "children only hidden by a container are not listed"
    );
    assert_eq!(rest.items[0].kind, RemovedKind::Task);
    assert_eq!(rest.items[0].id, tickets.id);
    assert!(rest.items[0].in_removed_container, "its List is in Removed");
    assert!(rest.next_cursor.is_none());

    assert!(matches!(
        store.list_removed("", 0, None),
        Err(TaskError::InvalidPageSize)
    ));
    assert!(matches!(
        store.list_removed("", 10, Some("not-a-cursor")),
        Err(TaskError::InvalidRemovedCursor)
    ));
}

#[test]
fn an_unlinked_link_cannot_be_brought_back_onto_a_removed_task_or_note() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("l.db"));
    let errand = task(&store, "Return library books");
    let list_note = note(&store, "# Book list");
    let link = store.link_note(&errand.id, &list_note.id).unwrap();
    store.unlink_note(&link.id).unwrap();
    let removed = store
        .set_note_removed(set(&list_note.id, list_note.revision, true))
        .unwrap();
    assert!(matches!(
        store.restore_link(&link.id),
        Err(TaskError::NoteRemoved)
    ));
    store
        .set_note_removed(set(&list_note.id, removed.revision, false))
        .unwrap();
    let errand = store
        .set_task_removed(set(&errand.id, errand.revision, true))
        .unwrap();
    assert!(matches!(
        store.restore_link(&link.id),
        Err(TaskError::TaskRemoved)
    ));
    store
        .set_task_removed(set(&errand.id, errand.revision, false))
        .unwrap();
    assert!(store.restore_link(&link.id).unwrap().unlinked_at.is_none());
}

#[test]
fn an_item_hidden_by_a_removed_container_cannot_be_removed_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("h.db"));
    let trip = list(&store, "Trip");
    let pack = task_in(&store, "Pack bags", &trip);
    let work = folder(&store, "Work", None);
    let meetings = folder(&store, "Meetings", Some(&work));
    let agenda = note_at(&store, "# Agenda", &meetings);
    set_list(&store, &trip, true);
    store
        .set_note_folder_removed(set(&work.id, work.revision, true))
        .unwrap();

    // Removing these now would make "Removed on its own" mean nothing:
    // restoring the container must bring them back.
    assert!(matches!(
        store.set_task_removed(set(&pack.id, pack.revision, true)),
        Err(TaskError::TaskListRemoved)
    ));
    assert!(matches!(
        store.set_note_removed(set(&agenda.id, agenda.revision, true)),
        Err(TaskError::NoteFolderRemoved)
    ));
    assert!(matches!(
        store.set_note_folder_removed(set(&meetings.id, meetings.revision, true)),
        Err(TaskError::NoteFolderRemoved)
    ));
    assert!(!store.get_task(&pack.id).unwrap().unwrap().removed);
    assert!(!store.get_note(&agenda.id).unwrap().unwrap().removed);
    assert!(
        !store
            .get_note_folder(&meetings.id)
            .unwrap()
            .unwrap()
            .removed
    );
}

#[test]
fn restoring_a_folder_brings_its_note_back_into_a_tasks_linked_notes() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("k.db"));
    let launch = task(&store, "Plan product launch");
    let work = folder(&store, "Work", None);
    let meetings = folder(&store, "Meetings", Some(&work));
    let minutes = note_at(&store, "# Launch minutes", &meetings);
    store.link_note(&launch.id, &minutes.id).unwrap();
    let linked = |store: &TaskStore| -> Vec<String> {
        store
            .list_task_notes(&launch.id, 50, None)
            .unwrap()
            .items
            .into_iter()
            .map(|n| n.title)
            .collect()
    };
    assert_eq!(linked(&store), vec!["Launch minutes"]);

    let work = store
        .set_note_folder_removed(set(&work.id, work.revision, true))
        .unwrap();
    assert!(linked(&store).is_empty(), "hidden with the folder above it");

    store
        .set_note_folder_removed(set(&work.id, work.revision, false))
        .unwrap();
    assert_eq!(linked(&store), vec!["Launch minutes"]);
}

fn removed_titles(store: &TaskStore, text: &str) -> Vec<String> {
    store
        .list_removed(text, 100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.title)
        .collect()
}

#[test]
fn the_removed_view_can_be_searched_across_every_kind() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("s.db"));
    let garden = list(&store, "Garden chores");
    let seeds = task_in(&store, "Order seeds", &garden);
    let pruning = task(&store, "Prune roses");
    let pruning = store
        .record_progress(RecordProgress {
            id: pruning.id.clone(),
            expected_revision: pruning.revision,
            status: None,
            progress: Some(20),
            next_step: None,
            note: Some("Bought new secateurs".into()),
        })
        .unwrap();
    let diary = note(&store, "# Garden diary\n\nThe tulips came up early.");
    let plans = folder(&store, "Garden plans", None);
    let other = note(&store, "# Shopping");
    store
        .set_task_removed(set(&seeds.id, seeds.revision, true))
        .unwrap();
    store
        .set_task_removed(set(&pruning.id, pruning.revision, true))
        .unwrap();
    store
        .set_note_removed(set(&diary.id, diary.revision, true))
        .unwrap();
    store
        .set_note_removed(set(&other.id, other.revision, true))
        .unwrap();
    store
        .set_note_folder_removed(set(&plans.id, plans.revision, true))
        .unwrap();
    set_list(&store, &garden, true);
    task(&store, "Garden party invitations");

    let mut found = removed_titles(&store, "garden");
    found.sort();
    // Names, Task titles and Note text all match; active items never do.
    assert_eq!(found, vec!["Garden chores", "Garden diary", "Garden plans"]);
    assert_eq!(removed_titles(&store, "secateurs"), vec!["Prune roses"]);
    assert_eq!(removed_titles(&store, "tulip"), vec!["Garden diary"]);
    assert_eq!(removed_titles(&store, "seeds order"), vec!["Order seeds"]);
    assert!(removed_titles(&store, "!!!").is_empty());
    assert_eq!(removed_titles(&store, "  ").len(), 6);

    // Pages stay bounded, and a cursor only continues the search it came from.
    let first = store.list_removed("garden", 2, None).unwrap();
    assert_eq!(first.items.len(), 2);
    let rest = store
        .list_removed("garden", 2, first.next_cursor.as_deref())
        .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert!(rest.next_cursor.is_none());
    assert!(matches!(
        store.list_removed("roses", 2, first.next_cursor.as_deref()),
        Err(TaskError::InvalidRemovedCursor)
    ));
}
