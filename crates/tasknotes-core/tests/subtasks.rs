//! Subtasks (owner, batch 4, 2026-11-18): a simple checklist inside one Task,
//! each item a long text (up to 10,000 characters) that is done or not. The
//! Task shows how many are done ("2/5"); its progress stays the person's own.
//! Removing an item is reversible, never a deletion. Synthetic data only.

use std::path::Path;

use tasknotes_core::audit::Interface;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::subtasks::{AddSubtask, MoveSubtask, Subtask, UpdateSubtask};
use tasknotes_core::tasks::{CreateTask, Task, TaskError, TaskStore, WorkStatus};

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

fn add(store: &TaskStore, task: &Task, text: &str) -> Subtask {
    store
        .add_subtask(AddSubtask {
            task_id: task.id.clone(),
            text: text.into(),
        })
        .unwrap()
}

fn texts(store: &TaskStore, task: &Task) -> Vec<String> {
    store
        .list_subtasks(&task.id)
        .unwrap()
        .into_iter()
        .map(|item| item.text)
        .collect()
}

fn counts(store: &TaskStore, task: &Task) -> (u32, u32) {
    let task = store.get_task(&task.id).unwrap().unwrap();
    (task.subtasks_done, task.subtasks_total)
}

fn set_done(store: &TaskStore, item: &Subtask, done: bool) -> Result<Subtask, TaskError> {
    store.update_subtask(UpdateSubtask {
        id: item.id.clone(),
        expected_revision: item.revision,
        text: None,
        done: Some(done),
    })
}

#[test]
fn items_are_added_in_order_and_the_task_counts_the_done_ones() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let trip = task(&store, "Synthetic trip");
    let first = add(&store, &trip, "Book the train");
    add(&store, &trip, "  Pack the bag  ");
    assert_eq!(texts(&store, &trip), ["Book the train", "Pack the bag"]);
    assert_eq!(counts(&store, &trip), (0, 2));

    let done = set_done(&store, &first, true).unwrap();
    assert!(done.done);
    assert_eq!(done.revision, first.revision + 1);
    assert_eq!(counts(&store, &trip), (1, 2));
    // The Task's own progress and status are untouched: they stay the person's.
    let after = store.get_task(&trip.id).unwrap().unwrap();
    assert_eq!(
        (after.status, after.progress, after.revision),
        (WorkStatus::Ready, 0, trip.revision)
    );
    // Lists of Tasks carry the counts too.
    let listed = store.list_tasks(50, None).unwrap();
    let listed = listed.items.iter().find(|each| each.id == trip.id).unwrap();
    assert_eq!((listed.subtasks_done, listed.subtasks_total), (1, 2));
}

#[test]
fn text_is_long_or_short_but_never_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plan = task(&store, "Synthetic plan");
    let long = "a".repeat(10_000);
    assert_eq!(add(&store, &plan, &long).text.chars().count(), 10_000);
    let lines = add(&store, &plan, "First line\nsecond line");
    assert_eq!(lines.text, "First line\nsecond line");

    for text in ["", "   \n ", &"b".repeat(10_001)] {
        let refused = store
            .add_subtask(AddSubtask {
                task_id: plan.id.clone(),
                text: text.into(),
            })
            .unwrap_err();
        assert_eq!(refused.code(), "invalid_input", "{text:.10}");
    }
    let refused = store
        .update_subtask(UpdateSubtask {
            id: lines.id.clone(),
            expected_revision: lines.revision,
            text: Some(" ".into()),
            done: None,
        })
        .unwrap_err();
    assert_eq!(refused.code(), "invalid_input");
}

#[test]
fn edits_name_the_revision_they_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plan = task(&store, "Synthetic plan");
    let item = add(&store, &plan, "Draft");
    let edited = store
        .update_subtask(UpdateSubtask {
            id: item.id.clone(),
            expected_revision: item.revision,
            text: Some("Draft the outline".into()),
            done: None,
        })
        .unwrap();
    assert_eq!(edited.text, "Draft the outline");

    let stale = set_done(&store, &item, true).unwrap_err();
    assert_eq!(stale.code(), "revision_conflict");
    let same = store
        .update_subtask(UpdateSubtask {
            id: edited.id.clone(),
            expected_revision: edited.revision,
            text: Some("Draft the outline".into()),
            done: Some(false),
        })
        .unwrap_err();
    assert_eq!(same.code(), "unchanged");
    let nothing = store
        .update_subtask(UpdateSubtask {
            id: edited.id.clone(),
            expected_revision: edited.revision,
            text: None,
            done: None,
        })
        .unwrap_err();
    assert_eq!(nothing.code(), "invalid_input");
    let missing = store
        .update_subtask(UpdateSubtask {
            id: "no-such-item".into(),
            expected_revision: 1,
            text: None,
            done: Some(true),
        })
        .unwrap_err();
    assert_eq!(missing.code(), "not_found");
}

#[test]
fn an_item_moves_to_another_place_in_the_checklist() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plan = task(&store, "Synthetic plan");
    let a = add(&store, &plan, "A");
    add(&store, &plan, "B");
    let c = add(&store, &plan, "C");
    let moved = store
        .move_subtask(MoveSubtask {
            id: c.id.clone(),
            expected_revision: c.revision,
            index: 0,
        })
        .unwrap();
    assert_eq!(moved.revision, c.revision + 1);
    assert_eq!(texts(&store, &plan), ["C", "A", "B"]);
    store
        .move_subtask(MoveSubtask {
            id: a.id.clone(),
            expected_revision: a.revision,
            index: 99, // past the end: last
        })
        .unwrap();
    assert_eq!(texts(&store, &plan), ["C", "B", "A"]);
}

#[test]
fn removing_an_item_is_reversible_and_it_comes_back_in_its_place() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plan = task(&store, "Synthetic plan");
    add(&store, &plan, "A");
    let b = set_done(&store, &add(&store, &plan, "B"), true).unwrap();
    add(&store, &plan, "C");

    let removed = store.remove_subtask(&b.id, b.revision).unwrap();
    assert!(removed.removed);
    assert_eq!(texts(&store, &plan), ["A", "C"]);
    assert_eq!(counts(&store, &plan), (0, 2));
    assert_eq!(
        store
            .remove_subtask(&b.id, removed.revision)
            .unwrap_err()
            .code(),
        "unchanged"
    );

    let back = store.restore_subtask(&b.id).unwrap();
    assert!(!back.removed && back.done);
    assert_eq!(texts(&store, &plan), ["A", "B", "C"]);
    assert_eq!(counts(&store, &plan), (1, 3));
}

#[test]
fn a_removed_task_keeps_its_checklist_but_takes_no_changes() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plan = task(&store, "Synthetic plan");
    let item = add(&store, &plan, "A");
    store
        .set_task_removed(SetRemoved {
            id: plan.id.clone(),
            expected_revision: plan.revision,
            removed: true,
        })
        .unwrap();
    let refused = store
        .add_subtask(AddSubtask {
            task_id: plan.id.clone(),
            text: "B".into(),
        })
        .unwrap_err();
    assert_eq!(refused.code(), "removed");
    assert_eq!(set_done(&store, &item, true).unwrap_err().code(), "removed");
    assert_eq!(texts(&store, &plan), ["A"]);
}

#[test]
fn a_checklist_holds_at_most_500_items() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plan = task(&store, "Synthetic plan");
    for index in 0..500 {
        add(&store, &plan, &format!("Item {index}"));
    }
    let refused = store
        .add_subtask(AddSubtask {
            task_id: plan.id.clone(),
            text: "One too many".into(),
        })
        .unwrap_err();
    assert_eq!(refused.code(), "invalid_input");
}

#[test]
fn every_change_is_in_the_audit_history_and_the_window_hears_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let desktop = store_at(&path);
    let plan = task(&desktop, "Synthetic plan");
    let seen = desktop.outside_changes(None).unwrap().latest;
    let cli = TaskStore::open_as(&path, Interface::Cli).unwrap();
    let item = add(&cli, &plan, "From the CLI");
    let done = set_done(&cli, &item, true).unwrap();
    let removed = cli.remove_subtask(&done.id, done.revision).unwrap();
    cli.restore_subtask(&removed.id).unwrap();

    assert!(desktop.outside_changes(Some(seen)).unwrap().changed);
    let actions: Vec<String> = desktop
        .list_audit_entries(None, 50, None)
        .unwrap()
        .items
        .into_iter()
        .map(|entry| entry.action.as_str().to_owned())
        .collect();
    for action in [
        "subtask.create",
        "subtask.update",
        "subtask.remove",
        "subtask.restore",
    ] {
        assert!(
            actions.iter().any(|each| each == action),
            "{action} in {actions:?}"
        );
    }
}

#[test]
fn a_v24_database_gains_checklists_and_keeps_its_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let plan = task(&store_at(&path), "Synthetic older plan");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("DROP TABLE task_subtasks; PRAGMA user_version = 24;")
            .unwrap();
    }
    let store = store_at(&path);
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, tasknotes_core::tasks::SCHEMA_VERSION);
    let kept = store.get_task(&plan.id).unwrap().unwrap();
    assert_eq!((kept.title.as_str(), kept.subtasks_total), ("Synthetic older plan", 0));
    add(&store, &kept, "Works after the upgrade");
    assert_eq!(counts(&store, &kept), (0, 1));
}
