//! One Task in several Lists (owner, batch 4, 2026-11-18): a Task keeps its
//! own List and can also be shown in others, identical everywhere: one row,
//! so completing it in one completes it in all. Each List keeps its own
//! Custom Order for it. Removing it from one List only takes it out of that
//! List. Synthetic data only.

use std::path::Path;

use tasknotes_core::audit::Interface;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, MoveTask, ReorderTask, ShowTaskIn, SortMode, Task,
    TaskQuery, TaskStore, TaskView, WorkStatus,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn task_in(store: &TaskStore, title: &str, list: &List) -> Task {
    let task = store
        .create_task(CreateTask {
            title: title.into(),
            description: String::new(),
        })
        .unwrap();
    store
        .move_task(MoveTask {
            id: task.id.clone(),
            expected_revision: task.revision,
            list_id: Some(list.id.clone()),
        })
        .unwrap()
}

fn show(
    store: &TaskStore,
    task: &Task,
    list: &List,
) -> Result<Task, tasknotes_core::tasks::TaskError> {
    store.show_task_in(ShowTaskIn {
        id: task.id.clone(),
        expected_revision: task.revision,
        list_id: list.id.clone(),
    })
}

fn hide(
    store: &TaskStore,
    task: &Task,
    list: &List,
) -> Result<Task, tasknotes_core::tasks::TaskError> {
    store.hide_task_from(ShowTaskIn {
        id: task.id.clone(),
        expected_revision: task.revision,
        list_id: list.id.clone(),
    })
}

fn titles_in(store: &TaskStore, list: &List, sort: SortMode) -> Vec<String> {
    let query = TaskQuery {
        view: TaskView::List(list.id.clone()),
        sort,
        ..TaskQuery::default()
    };
    store
        .query_tasks(&query, 50, None)
        .unwrap()
        .items
        .into_iter()
        .map(|task| task.title)
        .collect()
}

#[test]
fn a_task_shown_in_a_second_list_is_the_same_task_in_both() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let personal = list(&store, "Synthetic Personal");
    let big = list(&store, "Synthetic Big Task");
    let trip = task_in(&store, "Synthetic trip", &personal);

    let shown = show(&store, &trip, &big).unwrap();
    assert_eq!(shown.list_id.as_deref(), Some(personal.id.as_str()));
    assert_eq!(shown.also_in, std::slice::from_ref(&big.id));
    assert_eq!(shown.revision, trip.revision + 1);
    assert_eq!(
        titles_in(&store, &personal, SortMode::Priority),
        ["Synthetic trip"]
    );
    assert_eq!(
        titles_in(&store, &big, SortMode::Priority),
        ["Synthetic trip"]
    );

    // Completed in one is completed in all: there is one Task.
    let done = store
        .change_task(ChangeTask {
            id: trip.id.clone(),
            expected_revision: shown.revision,
            status: Some(WorkStatus::Completed),
            ..ChangeTask::default()
        })
        .unwrap();
    let in_big = TaskQuery {
        view: TaskView::List(big.id.clone()),
        ..TaskQuery::default()
    };
    let seen = store.query_tasks(&in_big, 50, None).unwrap().items;
    assert_eq!(
        (seen[0].status, seen[0].revision),
        (WorkStatus::Completed, done.revision)
    );

    // The sidebar counts it in each List, and once in All tasks.
    let reopened = store
        .change_task(ChangeTask {
            id: trip.id.clone(),
            expected_revision: done.revision,
            status: Some(WorkStatus::Ready),
            ..ChangeTask::default()
        })
        .unwrap();
    let counts = store.unfinished_counts().unwrap();
    assert_eq!(counts.lists.get(&personal.id), Some(&1));
    assert_eq!(counts.lists.get(&big.id), Some(&1));
    assert_eq!(counts.all, 1);
    let _ = reopened;
}

#[test]
fn removing_it_from_one_list_leaves_it_in_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let personal = list(&store, "Synthetic Personal");
    let big = list(&store, "Synthetic Big Task");
    let work = list(&store, "Synthetic Work");
    let trip = task_in(&store, "Synthetic trip", &personal);
    let trip = show(&store, &trip, &big).unwrap();
    let trip = show(&store, &trip, &work).unwrap();

    // From a List it is also shown in: only that List lets it go.
    let trip = hide(&store, &trip, &work).unwrap();
    assert_eq!(trip.also_in, std::slice::from_ref(&big.id));
    assert!(titles_in(&store, &work, SortMode::Priority).is_empty());

    // From its own List: the List it is also in becomes its own.
    let trip = hide(&store, &trip, &personal).unwrap();
    assert_eq!(trip.list_id.as_deref(), Some(big.id.as_str()));
    assert!(trip.also_in.is_empty());
    assert!(titles_in(&store, &personal, SortMode::Priority).is_empty());
    assert_eq!(
        titles_in(&store, &big, SortMode::Priority),
        ["Synthetic trip"]
    );

    // In one List only, it is not taken out: Move to Removed is for that.
    let refused = hide(&store, &trip, &big).unwrap_err();
    assert_eq!(refused.code(), "invalid_input");
    // Showing it again brings it back (Undo).
    let trip = show(&store, &trip, &work).unwrap();
    assert_eq!(trip.also_in, std::slice::from_ref(&work.id));
}

#[test]
fn showing_it_where_it_already_is_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let personal = list(&store, "Synthetic Personal");
    let big = list(&store, "Synthetic Big Task");
    let trip = task_in(&store, "Synthetic trip", &personal);
    assert_eq!(
        show(&store, &trip, &personal).unwrap_err().code(),
        "unchanged"
    );
    let trip = show(&store, &trip, &big).unwrap();
    assert_eq!(show(&store, &trip, &big).unwrap_err().code(), "unchanged");
    assert_eq!(
        hide(&store, &trip, &list(&store, "Synthetic Other"))
            .unwrap_err()
            .code(),
        "unchanged"
    );
    // A stale revision is refused like any change to the Task.
    let stale = store
        .show_task_in(ShowTaskIn {
            id: trip.id.clone(),
            expected_revision: 1,
            list_id: big.id.clone(),
        })
        .unwrap_err();
    assert_eq!(stale.code(), "revision_conflict");
}

#[test]
fn moving_it_to_a_list_it_is_also_in_makes_that_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let personal = list(&store, "Synthetic Personal");
    let big = list(&store, "Synthetic Big Task");
    let trip = task_in(&store, "Synthetic trip", &personal);
    let trip = show(&store, &trip, &big).unwrap();
    let moved = store
        .move_task(MoveTask {
            id: trip.id.clone(),
            expected_revision: trip.revision,
            list_id: Some(big.id.clone()),
        })
        .unwrap();
    assert!(moved.also_in.is_empty());
    assert_eq!(
        titles_in(&store, &big, SortMode::Priority),
        ["Synthetic trip"]
    );
}

#[test]
fn each_list_keeps_its_own_custom_order_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let personal = list(&store, "Synthetic Personal");
    let big = list(&store, "Synthetic Big Task");
    // Newer first at the top: c, b, a in Big Task.
    for title in ["a", "b", "c"] {
        task_in(&store, title, &big);
    }
    let x = task_in(&store, "x", &personal);
    task_in(&store, "y", &personal);
    let x = show(&store, &x, &big).unwrap();
    // Shown in Big Task at the top of its group, as a new Task lands.
    assert_eq!(
        titles_in(&store, &big, SortMode::Custom),
        ["x", "c", "b", "a"]
    );
    let personal_before = titles_in(&store, &personal, SortMode::Custom);

    let all = store
        .query_tasks(
            &TaskQuery {
                view: TaskView::List(big.id.clone()),
                sort: SortMode::Custom,
                ..TaskQuery::default()
            },
            50,
            None,
        )
        .unwrap()
        .items;
    let a = all.iter().find(|task| task.title == "a").unwrap();
    store
        .reorder_task(ReorderTask {
            id: x.id.clone(),
            expected_revision: x.revision,
            after_id: Some(a.id.clone()),
            before_id: None,
            shown_sort: None,
            list_id: Some(big.id.clone()),
        })
        .unwrap();
    assert_eq!(
        titles_in(&store, &big, SortMode::Custom),
        ["c", "b", "a", "x"]
    );
    assert_eq!(
        titles_in(&store, &personal, SortMode::Custom),
        personal_before
    );
}

#[test]
fn a_removed_list_takes_no_task_and_changes_are_audited() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store = store_at(&path);
    let personal = list(&store, "Synthetic Personal");
    let big = list(&store, "Synthetic Big Task");
    let trip = task_in(&store, "Synthetic trip", &personal);
    store
        .set_list_removed(tasknotes_core::tasks::SetListRemoved {
            id: big.id.clone(),
            expected_revision: big.revision,
            removed: true,
        })
        .unwrap();
    assert_eq!(show(&store, &trip, &big).unwrap_err().code(), "removed");

    let other = list(&store, "Synthetic Other");
    let cli = TaskStore::open_as(&path, Interface::Cli).unwrap();
    let trip = show(&cli, &trip, &other).unwrap();
    hide(&cli, &trip, &other).unwrap();
    let actions: Vec<String> = store
        .list_audit_entries(Some(&trip.id), 20, None)
        .unwrap()
        .items
        .into_iter()
        .map(|entry| entry.action.as_str().to_owned())
        .collect();
    assert!(actions.contains(&"task.show_in".to_owned()), "{actions:?}");
    assert!(
        actions.contains(&"task.hide_from".to_owned()),
        "{actions:?}"
    );
}
