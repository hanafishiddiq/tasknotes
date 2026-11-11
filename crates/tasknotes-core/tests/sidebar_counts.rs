//! Batch 3, Ticket 05: the number beside each sidebar folder counts that
//! place's unfinished Tasks (Ready and In Progress, Q45). Completed (Won't Do
//! included) and Removed never count, and a List counts only its own Tasks
//! (GLOSSARY "List"). Synthetic data only.

use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateSubList, CreateTask, List, SetListRemoved, Task, TaskStore,
    UnfinishedCounts, WorkStatus,
};

struct World {
    _dir: tempfile::TempDir,
    store: TaskStore,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    World {
        store: TaskStore::open(&dir.path().join("counts.db")).unwrap(),
        _dir: dir,
    }
}

fn task(store: &TaskStore, title: &str, list: Option<&List>) -> Task {
    store
        .create_task_in_list(
            CreateTask {
                title: title.into(),
                description: String::new(),
            },
            list.map(|list| list.id.as_str()),
        )
        .unwrap()
}

fn change(store: &TaskStore, task: &Task, input: ChangeTask) -> Task {
    store
        .change_task(ChangeTask {
            id: task.id.clone(),
            expected_revision: task.revision,
            ..input
        })
        .unwrap()
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn counts(store: &TaskStore) -> UnfinishedCounts {
    store.unfinished_counts().unwrap()
}

#[test]
fn each_place_counts_only_its_ready_and_in_progress_tasks() {
    let World { store, _dir } = world();
    let work = list(&store, "Synthetic work");
    let reports = store
        .create_sub_list(CreateSubList {
            name: "Synthetic reports".into(),
            parent_id: work.id.clone(),
        })
        .unwrap();
    let empty = list(&store, "Synthetic empty");

    task(&store, "Ready in the Inbox", None);
    let started = task(&store, "Started in the Inbox", None);
    change(
        &store,
        &started,
        ChangeTask {
            progress: Some(40),
            ..ChangeTask::default()
        },
    );
    let done = task(&store, "Done in the Inbox", None);
    change(
        &store,
        &done,
        ChangeTask {
            status: Some(WorkStatus::Completed),
            ..ChangeTask::default()
        },
    );
    let set_aside = task(&store, "Set aside in the Inbox", None);
    change(
        &store,
        &set_aside,
        ChangeTask {
            wont_do: Some(true),
            ..ChangeTask::default()
        },
    );
    task(&store, "Ready at work", Some(&work));
    task(&store, "Also ready at work", Some(&work));
    task(&store, "Ready in reports", Some(&reports));

    let counts = counts(&store);
    assert_eq!(counts.all, 5, "2 in the Inbox, 2 at work, 1 in reports");
    assert_eq!(counts.inbox, 2, "Completed and Won't Do do not count");
    // Work counts its own two, not the one in the List inside it.
    assert_eq!(counts.lists.get(&work.id), Some(&2));
    assert_eq!(counts.lists.get(&reports.id), Some(&1));
    // A List with nothing unfinished is left out: it shows no number.
    assert_eq!(counts.lists.get(&empty.id), None);
}

#[test]
fn removed_tasks_and_the_tasks_of_a_removed_list_do_not_count() {
    let World { store, _dir } = world();
    let errands = list(&store, "Synthetic errands");
    let gone = task(&store, "Removed from the Inbox", None);
    task(&store, "Kept in the Inbox", None);
    task(&store, "Bulbs", Some(&errands));
    store
        .set_task_removed(SetRemoved {
            id: gone.id.clone(),
            expected_revision: gone.revision,
            removed: true,
        })
        .unwrap();
    assert_eq!((counts(&store).all, counts(&store).inbox), (2, 1));

    store
        .set_list_removed(SetListRemoved {
            id: errands.id.clone(),
            expected_revision: errands.revision,
            removed: true,
        })
        .unwrap();
    let counts = counts(&store);
    assert_eq!(counts.all, 1);
    assert!(counts.lists.is_empty(), "{:?}", counts.lists);
}

#[test]
fn a_list_inside_a_removed_list_counts_nothing_until_it_is_restored() {
    let World { store, _dir } = world();
    let outer = list(&store, "Synthetic outer");
    let inner = store
        .create_sub_list(CreateSubList {
            name: "Synthetic inner".into(),
            parent_id: outer.id.clone(),
        })
        .unwrap();
    task(&store, "Deep in the inner List", Some(&inner));
    assert_eq!(counts(&store).lists.get(&inner.id), Some(&1));

    let removed = store
        .set_list_removed(SetListRemoved {
            id: outer.id.clone(),
            expected_revision: outer.revision,
            removed: true,
        })
        .unwrap();
    assert_eq!(counts(&store), UnfinishedCounts::default());

    store
        .set_list_removed(SetListRemoved {
            id: outer.id.clone(),
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();
    assert_eq!(counts(&store).all, 1);
}
