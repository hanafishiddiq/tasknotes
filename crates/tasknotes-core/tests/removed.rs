//! Ticket 09: Removed is a reversible disposition (ADR-0002). Synthetic data only.

use std::path::Path;

use tasknotes_core::notes::{CreateNote, Note};
use tasknotes_core::removed::SetRemoved;
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
