//! Ticket 13: Custom Order (ADR 0010). A person drags Tasks within a
//! work-status group of one List; each List remembers its sort. Synthetic
//! data only.

use std::path::Path;

use rusqlite::{Connection, params};
use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, MoveTask, Priority, ReorderTask, SCHEMA_VERSION,
    SortMode, Task, TaskError, TaskQuery, TaskStore, TaskView, ViewSort, WorkStatus,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn fresh() -> (tempfile::TempDir, TaskStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("custom.db"));
    (dir, store)
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn task_in(store: &TaskStore, title: &str, list: Option<&List>) -> Task {
    store
        .create_task_in_list(
            CreateTask {
                title: title.into(),
                description: String::new(),
            },
            list.map(|l| l.id.as_str()),
        )
        .unwrap()
}

fn now(store: &TaskStore, task: &Task) -> Task {
    store.get_task(&task.id).unwrap().unwrap()
}

fn view_of(list: Option<&List>) -> TaskView {
    list.map_or(TaskView::Inbox, |l| TaskView::List(l.id.clone()))
}

fn titles(store: &TaskStore, view: TaskView, sort: SortMode) -> Vec<String> {
    store
        .query_tasks(
            &TaskQuery {
                view,
                sort,
                ..TaskQuery::default()
            },
            100,
            None,
        )
        .unwrap()
        .items
        .into_iter()
        .map(|task| task.title)
        .collect()
}

fn custom(store: &TaskStore, list: Option<&List>) -> Vec<String> {
    titles(store, view_of(list), SortMode::Custom)
}

fn reorder(
    store: &TaskStore,
    task: &Task,
    after: Option<&Task>,
    before: Option<&Task>,
    shown: Option<SortMode>,
) -> Result<Task, TaskError> {
    store.reorder_task(ReorderTask {
        id: task.id.clone(),
        expected_revision: now(store, task).revision,
        after_id: after.map(|t| t.id.clone()),
        before_id: before.map(|t| t.id.clone()),
        shown_sort: shown,
    })
}

fn after(store: &TaskStore, task: &Task, anchor: Option<&Task>) -> Result<Task, TaskError> {
    reorder(store, task, anchor, None, None)
}

fn start(store: &TaskStore, task: &Task) -> Task {
    store
        .change_task(ChangeTask {
            id: task.id.clone(),
            expected_revision: now(store, task).revision,
            status: Some(WorkStatus::InProgress),
            ..ChangeTask::default()
        })
        .unwrap()
}

fn sort_of(store: &TaskStore, view: &TaskView) -> Option<SortMode> {
    store
        .view_sorts()
        .unwrap()
        .into_iter()
        .find(|item| item.view == *view)
        .map(|item| item.sort)
}

#[test]
fn a_new_task_lands_at_the_top_of_its_group_in_custom_order() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    for title in ["One", "Two", "Three"] {
        task_in(&store, title, Some(&work));
    }
    task_in(&store, "Inbox one", None);
    task_in(&store, "Inbox two", None);
    assert_eq!(custom(&store, Some(&work)), ["Three", "Two", "One"]);
    assert_eq!(custom(&store, None), ["Inbox two", "Inbox one"]);
}

#[test]
fn a_task_moves_after_another_before_another_or_first_within_its_group() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let one = task_in(&store, "One", Some(&work));
    let two = task_in(&store, "Two", Some(&work));
    let three = task_in(&store, "Three", Some(&work));
    // Three, Two, One
    let moved = after(&store, &three, Some(&one)).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["Two", "One", "Three"]);
    // Only the moved Task's revision changes; neighbours keep theirs.
    assert_eq!(moved.revision, three.revision + 1);
    assert_eq!(now(&store, &two).revision, two.revision);
    assert_eq!(now(&store, &one).revision, one.revision);

    after(&store, &one, None).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["One", "Two", "Three"]);
    reorder(&store, &three, None, Some(&two), None).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["One", "Three", "Two"]);
    reorder(&store, &two, None, Some(&one), None).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["Two", "One", "Three"]);

    // A move nothing shows is refused as unchanged.
    let err = after(&store, &one, Some(&two)).unwrap_err();
    assert_eq!(err.code(), "unchanged");
    let err = reorder(&store, &one, None, Some(&three), None).unwrap_err();
    assert_eq!(err.code(), "unchanged");
    // Naming both anchors is a mistake of the caller.
    let err = reorder(&store, &one, Some(&two), Some(&three), None).unwrap_err();
    assert_eq!(err.code(), "invalid_input");
}

#[test]
fn a_move_never_crosses_a_work_status_group_or_a_list() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let home = list(&store, "Synthetic Home");
    let ready = task_in(&store, "Ready one", Some(&work));
    let started = start(&store, &task_in(&store, "Started", Some(&work)));
    let elsewhere = task_in(&store, "Elsewhere", Some(&home));
    let gone = task_in(&store, "Gone", Some(&work));
    store
        .set_task_removed(SetRemoved {
            id: gone.id.clone(),
            expected_revision: gone.revision,
            removed: true,
        })
        .unwrap();

    for anchor in [&started, &elsewhere, &gone] {
        let err = after(&store, &ready, Some(anchor)).unwrap_err();
        assert!(
            matches!(err, TaskError::TaskOrderAnchorChanged),
            "{anchor:?}: {err:?}"
        );
        assert_eq!(err.code(), "revision_conflict");
        let err = reorder(&store, &ready, None, Some(anchor), None).unwrap_err();
        assert_eq!(err.code(), "revision_conflict");
    }
    // Nothing moved and no status changed.
    assert_eq!(now(&store, &ready).status, WorkStatus::Ready);
    assert_eq!(now(&store, &ready).revision, ready.revision);
    assert_eq!(custom(&store, Some(&work)), ["Started", "Ready one"]);
}

#[test]
fn a_stale_revision_is_refused_and_recorded_and_an_applied_move_is_audited() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("audit.db");
    let desktop = store_at(&db);
    let one = task_in(&desktop, "One", None);
    let two = task_in(&desktop, "Two", None);
    let cli = TaskStore::open_as(&db, Interface::Cli).unwrap();

    let stale = cli
        .reorder_task(ReorderTask {
            id: two.id.clone(),
            expected_revision: two.revision + 7,
            after_id: Some(one.id.clone()),
            before_id: None,
            shown_sort: None,
        })
        .unwrap_err();
    assert_eq!(stale.code(), "revision_conflict");

    let moved = cli
        .reorder_task(ReorderTask {
            id: two.id.clone(),
            expected_revision: two.revision,
            after_id: Some(one.id.clone()),
            before_id: None,
            shown_sort: None,
        })
        .unwrap();
    let entries = desktop.list_audit_entries(Some(&two.id), 10, None).unwrap();
    let reorders: Vec<_> = entries
        .items
        .iter()
        .filter(|entry| entry.action == AuditAction::TaskReorder)
        .map(|entry| (entry.interface, entry.outcome, entry.revision))
        .collect();
    assert_eq!(
        reorders,
        [
            (Interface::Cli, AuditOutcome::Applied, Some(moved.revision)),
            (
                Interface::Cli,
                AuditOutcome::Rejected,
                Some(two.revision + 7)
            ),
        ]
    );
    assert_eq!(AuditAction::TaskReorder.as_str(), "task.reorder");
    // The CLI reads and moves the Custom Order; the List's sort is the person's.
    assert_eq!(sort_of(&desktop, &TaskView::Inbox), None);
}

#[test]
fn a_drag_under_another_sort_starts_custom_from_that_order_and_switches_the_list() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let low = task_in(&store, "Low", Some(&work));
    let high = task_in(&store, "High", Some(&work));
    let mid = task_in(&store, "Mid", Some(&work));
    for (task, priority) in [(&low, Priority::Low), (&high, Priority::VeryHigh)] {
        store
            .change_task(ChangeTask {
                id: task.id.clone(),
                expected_revision: task.revision,
                priority: Some(priority),
                ..ChangeTask::default()
            })
            .unwrap();
    }
    let view = view_of(Some(&work));
    assert_eq!(
        titles(&store, view.clone(), SortMode::Priority),
        ["High", "Mid", "Low"]
    );
    assert_eq!(custom(&store, Some(&work)), ["Mid", "High", "Low"]);
    store.set_view_sort(&view, SortMode::Priority).unwrap();

    // Changing priority never switches the List to Custom.
    store
        .change_task(ChangeTask {
            id: mid.id.clone(),
            expected_revision: mid.revision,
            priority: Some(Priority::High),
            ..ChangeTask::default()
        })
        .unwrap();
    assert_eq!(sort_of(&store, &view), Some(SortMode::Priority));

    // A refused drag changes nothing, not even the sort.
    let err = reorder(&store, &low, Some(&mid), None, Some(SortMode::Priority)).unwrap_err();
    assert_eq!(err.code(), "unchanged");
    assert_eq!(sort_of(&store, &view), Some(SortMode::Priority));
    assert_eq!(custom(&store, Some(&work)), ["Mid", "High", "Low"]);

    // On screen: High, Mid, Low. Low dragged above Mid.
    reorder(&store, &low, None, Some(&mid), Some(SortMode::Priority)).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["High", "Low", "Mid"]);
    assert_eq!(sort_of(&store, &view), Some(SortMode::Custom));

    // The Custom Order stays when the person looks by another sort and back.
    store.set_view_sort(&view, SortMode::Newest).unwrap();
    assert_eq!(sort_of(&store, &view), Some(SortMode::Newest));
    store.set_view_sort(&view, SortMode::Custom).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["High", "Low", "Mid"]);
}

#[test]
fn each_view_remembers_its_sort_across_restarts_and_only_the_desktop_sets_it() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sorts.db");
    let store = store_at(&db);
    let work = list(&store, "Synthetic Work");
    let home = list(&store, "Synthetic Home");
    assert!(store.view_sorts().unwrap().is_empty());
    store
        .set_view_sort(&TaskView::List(work.id.clone()), SortMode::Custom)
        .unwrap();
    store
        .set_view_sort(&TaskView::List(home.id.clone()), SortMode::Difficulty)
        .unwrap();
    store
        .set_view_sort(&TaskView::Inbox, SortMode::Newest)
        .unwrap();
    store
        .set_view_sort(&TaskView::All, SortMode::Difficulty)
        .unwrap();
    let err = store
        .set_view_sort(&TaskView::All, SortMode::Custom)
        .unwrap_err();
    assert_eq!(err.code(), "invalid_input");
    let err = store
        .set_view_sort(
            &TaskView::List(uuid::Uuid::new_v4().to_string()),
            SortMode::Newest,
        )
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
    drop(store);

    let again = store_at(&db);
    let mut sorts = again.view_sorts().unwrap();
    sorts.sort_by_key(|item| format!("{:?}", item.view));
    let mut expected = vec![
        ViewSort {
            view: TaskView::All,
            sort: SortMode::Difficulty,
        },
        ViewSort {
            view: TaskView::Inbox,
            sort: SortMode::Newest,
        },
        ViewSort {
            view: TaskView::List(work.id.clone()),
            sort: SortMode::Custom,
        },
        ViewSort {
            view: TaskView::List(home.id.clone()),
            sort: SortMode::Difficulty,
        },
    ];
    expected.sort_by_key(|item| format!("{:?}", item.view));
    assert_eq!(sorts, expected);

    let mcp = TaskStore::open_as(&db, Interface::Mcp).unwrap();
    let err = mcp
        .set_view_sort(&TaskView::Inbox, SortMode::Priority)
        .unwrap_err();
    assert_eq!(err.code(), "desktop_only");
    // The desktop sends a view as it sends one to list Tasks.
    let sent: ViewSort =
        serde_json::from_value(serde_json::json!({"view": {"kind": "inbox"}, "sort": "custom"}))
            .unwrap();
    assert_eq!(sent.sort, SortMode::Custom);
}

#[test]
fn custom_order_needs_one_list_or_inbox() {
    let (_dir, store) = fresh();
    let err = store
        .query_tasks(
            &TaskQuery {
                sort: SortMode::Custom,
                ..TaskQuery::default()
            },
            10,
            None,
        )
        .unwrap_err();
    assert_eq!(err.code(), "invalid_input");
}

#[test]
fn custom_pages_cover_every_task_once_in_order() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let tasks: Vec<Task> = (0..7)
        .map(|n| task_in(&store, &format!("T{n}"), Some(&work)))
        .collect();
    start(&store, &tasks[2]);
    start(&store, &tasks[5]);
    // Shuffle a little: T0 to the top of Ready.
    after(&store, &tasks[0], None).unwrap();
    let all = custom(&store, Some(&work));
    assert_eq!(all, ["T5", "T2", "T0", "T6", "T4", "T3", "T1"]);
    let query = TaskQuery {
        view: view_of(Some(&work)),
        sort: SortMode::Custom,
        ..TaskQuery::default()
    };
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = store.query_tasks(&query, 2, cursor.as_deref()).unwrap();
        seen.extend(page.items.into_iter().map(|task| task.title));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, all);
    // A Custom cursor belongs to its query.
    let first = store.query_tasks(&query, 2, None).unwrap();
    let err = store
        .query_tasks(
            &TaskQuery {
                sort: SortMode::Priority,
                ..query
            },
            2,
            first.next_cursor.as_deref(),
        )
        .unwrap_err();
    assert_eq!(err.code(), "invalid_cursor");
}

#[test]
fn a_task_keeps_its_place_through_status_changes_and_lands_on_top_of_another_list() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let home = list(&store, "Synthetic Home");
    let one = task_in(&store, "One", Some(&work));
    let two = task_in(&store, "Two", Some(&work));
    task_in(&store, "Home one", Some(&home));
    // Two, One. Two starts; One starts: In Progress keeps the List's sequence.
    start(&store, &one);
    start(&store, &two);
    assert_eq!(custom(&store, Some(&work)), ["Two", "One"]);
    let moved = store
        .move_task(MoveTask {
            id: one.id.clone(),
            expected_revision: now(&store, &one).revision,
            list_id: Some(home.id.clone()),
        })
        .unwrap();
    assert_eq!(moved.list_id.as_deref(), Some(home.id.as_str()));
    let started_home = task_in(&store, "Home two", Some(&home));
    after(&store, &started_home, Some(&now(&store, &one))).unwrap_err();
    assert_eq!(custom(&store, Some(&home)), ["One", "Home two", "Home one"]);
}

#[test]
fn a_restored_task_comes_back_to_its_place() {
    let (_dir, store) = fresh();
    let one = task_in(&store, "One", None);
    let two = task_in(&store, "Two", None);
    let three = task_in(&store, "Three", None);
    after(&store, &three, Some(&one)).unwrap();
    assert_eq!(custom(&store, None), ["Two", "One", "Three"]);
    let removed = store
        .set_task_removed(SetRemoved {
            id: one.id.clone(),
            expected_revision: now(&store, &one).revision,
            removed: true,
        })
        .unwrap();
    assert_eq!(custom(&store, None), ["Two", "Three"]);
    store
        .set_task_removed(SetRemoved {
            id: one.id.clone(),
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();
    assert_eq!(custom(&store, None), ["Two", "One", "Three"]);
    let _ = two;
}

#[test]
fn exports_carry_each_tasks_position() {
    let (dir, store) = fresh();
    let one = task_in(&store, "One", None);
    let two = task_in(&store, "Two", None);
    after(&store, &two, Some(&one)).unwrap();
    let destination = dir.path().join("export.json");
    store
        .export_data(ExportRequest {
            format: ExportFormat::Json,
            destination: destination.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(destination).unwrap()).unwrap();
    let position = |id: &str| {
        document["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|task| task["id"] == id)
            .unwrap()["position"]
            .as_i64()
            .unwrap()
    };
    assert!(position(&one.id) < position(&two.id));
}

/// Puts a database back as schema v19 left it.
fn back_to_v19(db: &Path) {
    Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP INDEX tasks_custom_order; ALTER TABLE tasks DROP COLUMN position;
             DROP TABLE view_sorts; PRAGMA user_version = 19;",
        )
        .unwrap();
}

#[test]
fn upgrading_from_v19_starts_custom_order_newest_first_and_changes_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade.db");
    let store = store_at(&db);
    let work = list(&store, "Synthetic Work");
    for title in ["W1", "W2", "W3"] {
        task_in(&store, title, Some(&work));
    }
    for title in ["I1", "I2"] {
        task_in(&store, title, None);
    }
    drop(store);
    back_to_v19(&db);
    let rows = || -> Vec<(String, i64, i64)> {
        let conn = Connection::open(&db).unwrap();
        let mut statement = conn
            .prepare("SELECT id, revision, updated_at FROM tasks ORDER BY id")
            .unwrap();
        statement
            .query_map(params![], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    let before = rows();

    let store = store_at(&db);
    let version: i64 = Connection::open(&db)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(custom(&store, Some(&work)), ["W3", "W2", "W1"]);
    assert_eq!(custom(&store, None), ["I2", "I1"]);
    assert!(store.view_sorts().unwrap().is_empty());
    assert_eq!(rows(), before);
}
