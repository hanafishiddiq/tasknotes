//! Ticket 13: Custom Order (ADR 0010). A person drags Tasks within a
//! work-status group of one List; each List remembers its sort. Synthetic
//! data only.

use std::path::Path;

use rusqlite::{Connection, params};
use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateSubList, CreateTask, Difficulty, List, MoveList, MoveTask,
    PlaceList, Priority, ReorderTask, SCHEMA_VERSION, SetListRemoved, SortMode, Task, TaskError,
    TaskQuery, TaskStore, TaskView, ViewSort, WorkStatus,
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
    // A move through the CLI switches Inbox to Custom (Q37), so the person
    // sees it; the refused one before it switched nothing.
    assert_eq!(sort_of(&desktop, &TaskView::Inbox), Some(SortMode::Custom));
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

/// Puts a database back as schema v19 left it: without the Pomodoro Sessions
/// of v22, the nested Lists of v21 and the Custom Order of v20.
fn back_to_v19(db: &Path) {
    Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP TABLE pomodoro_sessions;
             DROP INDEX lists_parent; DROP INDEX lists_manual_order;
             DROP INDEX lists_sibling_name; ALTER TABLE lists DROP COLUMN parent_id;
             CREATE UNIQUE INDEX lists_active_name ON lists(name COLLATE NOCASE)
               WHERE removed_at IS NULL;
             CREATE INDEX lists_manual_order ON lists(position, id);
             DROP INDEX tasks_custom_order; ALTER TABLE tasks DROP COLUMN position;
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

fn set_priority(store: &TaskStore, task: &Task, priority: Priority) {
    store
        .change_task(ChangeTask {
            id: task.id.clone(),
            expected_revision: now(store, task).revision,
            priority: Some(priority),
            ..ChangeTask::default()
        })
        .unwrap();
}

/// A List whose Tasks differ in every way a sort looks at: work status,
/// priority, Difficulty, and creation times that tie, so the id decides.
/// One Task is in Removed; it keeps a place no one sees.
fn mixed_list(store: &TaskStore, db: &Path, name: &str) -> (List, Vec<Task>) {
    let work = list(store, name);
    let mut made = Vec::new();
    for n in 0..12 {
        let task = task_in(store, &format!("{name} {n:02}"), Some(&work));
        store
            .change_task(ChangeTask {
                id: task.id.clone(),
                expected_revision: task.revision,
                status: Some(
                    [
                        WorkStatus::Ready,
                        WorkStatus::InProgress,
                        WorkStatus::Completed,
                    ][n % 3],
                ),
                priority: Some(
                    [
                        Priority::Low,
                        Priority::VeryHigh,
                        Priority::Medium,
                        Priority::High,
                    ][n % 4],
                ),
                difficulty: Some(
                    [None, Some(Difficulty::Hard), Some(Difficulty::VeryEasy)][n / 3 % 3],
                ),
                ..ChangeTask::default()
            })
            .unwrap();
        made.push(task);
    }
    // Pairs created in the same millisecond: the id settles their order.
    Connection::open(db)
        .unwrap()
        .execute(
            "UPDATE tasks SET created_at = 1000 + (rowid / 2) WHERE list_id = ?1",
            params![work.id],
        )
        .unwrap();
    let gone = &made[4];
    store
        .set_task_removed(SetRemoved {
            id: gone.id.clone(),
            expected_revision: now(store, gone).revision,
            removed: true,
        })
        .unwrap();
    (work, made)
}

/// Starting a Custom Order from the order on screen, by choosing Custom or
/// by a drag, uses the very order a page of that sort lists (one shared
/// definition), for every sort a List can show.
#[test]
fn custom_starts_from_exactly_the_order_each_sort_lists() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seed.db");
    let store = store_at(&db);
    for sort in SortMode::ALL.into_iter().filter(|s| *s != SortMode::Custom) {
        let (chosen, _) = mixed_list(&store, &db, &format!("Chosen {sort:?}"));
        let view = view_of(Some(&chosen));
        store.set_view_sort(&view, sort).unwrap();
        let listed = titles(&store, view.clone(), sort);
        assert_eq!(listed.len(), 11, "{sort:?}");
        store.set_view_sort(&view, SortMode::Custom).unwrap();
        assert_eq!(
            titles(&store, view.clone(), SortMode::Custom),
            listed,
            "{sort:?}"
        );

        let (dragged, made) = mixed_list(&store, &db, &format!("Dragged {sort:?}"));
        let view = view_of(Some(&dragged));
        store.set_view_sort(&view, sort).unwrap();
        let mut listed = titles(&store, view.clone(), sort);
        // The last Ready Task on screen, dropped above the first Ready one.
        let at = |listed: &[String], task: &Task| {
            listed
                .iter()
                .position(|title| *title == task.title)
                .unwrap()
        };
        let mut ready: Vec<&Task> = made
            .iter()
            .filter(|task| now(&store, task).status == WorkStatus::Ready)
            .collect();
        ready.sort_by_key(|task| at(&listed, task));
        let (first, last) = (ready[0], ready[ready.len() - 1]);
        reorder(&store, last, None, Some(first), Some(sort)).unwrap();
        let (from, to) = (at(&listed, last), at(&listed, first));
        let moving = listed.remove(from);
        listed.insert(to, moving);
        assert_eq!(
            titles(&store, view.clone(), SortMode::Custom),
            listed,
            "{sort:?}"
        );
        assert_eq!(sort_of(&store, &view), Some(SortMode::Custom));
    }
}

/// Q35: choosing Custom for a List that has none of its own yet starts it
/// from the order on screen, as a drag does. Once it has one, looking by
/// another sort and back brings that one back (spec story 9).
#[test]
fn choosing_custom_starts_from_the_order_on_screen_then_keeps_that_order() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let low = task_in(&store, "Low", Some(&work));
    let high = task_in(&store, "High", Some(&work));
    task_in(&store, "Mid", Some(&work));
    set_priority(&store, &low, Priority::Low);
    set_priority(&store, &high, Priority::VeryHigh);
    let view = view_of(Some(&work));
    // Newest first is where new Tasks land; the screen shows Priority.
    assert_eq!(custom(&store, Some(&work)), ["Mid", "High", "Low"]);
    assert_eq!(sort_of(&store, &view), None);

    // A List that remembers no sort shows Priority.
    store.set_view_sort(&view, SortMode::Custom).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["High", "Mid", "Low"]);
    assert_eq!(sort_of(&store, &view), Some(SortMode::Custom));

    // Its own order now: arranged, then looked at by another sort and back.
    after(&store, &low, None).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["Low", "High", "Mid"]);
    store.set_view_sort(&view, SortMode::Newest).unwrap();
    store.set_view_sort(&view, SortMode::Custom).unwrap();
    assert_eq!(custom(&store, Some(&work)), ["Low", "High", "Mid"]);

    // Inbox starts from the sort it remembers, Hardest first here: by
    // Priority or as new Tasks land, Newer would come first.
    let older = task_in(&store, "Older", None);
    let newer = task_in(&store, "Newer", None);
    set_priority(&store, &newer, Priority::VeryHigh);
    store
        .change_task(ChangeTask {
            id: older.id.clone(),
            expected_revision: older.revision,
            difficulty: Some(Some(Difficulty::Hard)),
            ..ChangeTask::default()
        })
        .unwrap();
    assert_eq!(custom(&store, None), ["Newer", "Older"]);
    store
        .set_view_sort(&TaskView::Inbox, SortMode::Difficulty)
        .unwrap();
    store
        .set_view_sort(&TaskView::Inbox, SortMode::Custom)
        .unwrap();
    assert_eq!(custom(&store, None), ["Older", "Newer"]);
}

/// Q37: a move through the CLI or MCP switches its List to Custom, as a
/// drag does, so the person sees the order the AI made. It moves within
/// the Custom Order the AI read. A refused move switches nothing.
#[test]
fn a_move_from_any_interface_switches_its_list_to_custom() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("q37.db");
    let desktop = store_at(&db);
    let work = list(&desktop, "Synthetic Work");
    let one = task_in(&desktop, "One", Some(&work));
    let two = task_in(&desktop, "Two", Some(&work));
    let three = task_in(&desktop, "Three", Some(&work));
    set_priority(&desktop, &one, Priority::VeryHigh);
    let view = view_of(Some(&work));
    desktop.set_view_sort(&view, SortMode::Priority).unwrap();

    for interface in [Interface::Cli, Interface::Mcp] {
        let ai = TaskStore::open_as(&db, interface).unwrap();
        // Refused: nothing moves and the List keeps its sort.
        let err = after(&ai, &two, Some(&three)).unwrap_err();
        assert_eq!(err.code(), "unchanged");
        assert_eq!(sort_of(&desktop, &view), Some(SortMode::Priority));
        let err = after(&ai, &three, Some(&now(&desktop, &three))).unwrap_err();
        assert_eq!(err.code(), "unchanged");
        assert_eq!(sort_of(&desktop, &view), Some(SortMode::Priority));

        // The AI read Three, Two, One in Custom and puts Three last.
        assert_eq!(custom(&ai, Some(&work)), ["Three", "Two", "One"]);
        after(&ai, &three, Some(&one)).unwrap();
        assert_eq!(custom(&desktop, Some(&work)), ["Two", "One", "Three"]);
        assert_eq!(sort_of(&desktop, &view), Some(SortMode::Custom));

        // Back as it was for the next interface.
        after(&ai, &three, None).unwrap();
        desktop.set_view_sort(&view, SortMode::Priority).unwrap();
    }
}

/// Finding 6: switching a large List to Custom rewrites every position in
/// one transaction. Timed on 5,000 synthetic Tasks; the budget is loose so
/// a slow machine still passes, and the time is printed for the record.
#[test]
fn switching_a_list_of_5000_tasks_to_custom_is_quick() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("large.db");
    let store = store_at(&db);
    let chosen = list(&store, "Synthetic Large Chosen");
    let dragged = list(&store, "Synthetic Large Dragged");
    {
        let mut conn = Connection::open(&db).unwrap();
        let tx = conn.transaction().unwrap();
        let mut insert = tx
            .prepare(
                "INSERT INTO tasks (id, title, description, status, priority, progress,
                 revision, created_at, updated_at, next_step, list_id, position, difficulty)
                 VALUES (?1, ?2, '', ?3, ?4, ?5, 1, ?6, ?6, '', ?7, ?8, ?9)",
            )
            .unwrap();
        for target in [&chosen, &dragged] {
            for n in 0..5000_i64 {
                let status = ["ready", "in_progress", "completed"][(n % 3) as usize];
                let progress = [0, 40, 100][(n % 3) as usize];
                let priority = ["very_low", "low", "medium", "high", "very_high"][(n % 5) as usize];
                let difficulty = [None, Some("easy"), Some("hard")][(n % 3) as usize];
                insert
                    .execute(params![
                        uuid::Uuid::new_v4().to_string(),
                        format!("Synthetic large {n}"),
                        status,
                        priority,
                        progress,
                        1_000 + n * 7 % 4999,
                        target.id,
                        -n,
                        difficulty,
                    ])
                    .unwrap();
            }
        }
        drop(insert);
        tx.commit().unwrap();
    }
    let view = view_of(Some(&chosen));
    let listed = titles(&store, view.clone(), SortMode::Priority);

    let started = std::time::Instant::now();
    store.set_view_sort(&view, SortMode::Custom).unwrap();
    let choose = started.elapsed();
    assert_eq!(titles(&store, view, SortMode::Custom), listed);

    let view = view_of(Some(&dragged));
    // The top two on screen, both In Progress; the second dropped above the first.
    let page = store
        .query_tasks(
            &TaskQuery {
                view: view.clone(),
                ..TaskQuery::default()
            },
            2,
            None,
        )
        .unwrap()
        .items;
    let (first, second) = (&page[0], &page[1]);
    assert_eq!(first.status, second.status);
    let started = std::time::Instant::now();
    reorder(&store, second, None, Some(first), Some(SortMode::Priority)).unwrap();
    let drag = started.elapsed();
    assert_eq!(sort_of(&store, &view), Some(SortMode::Custom));

    eprintln!(
        "5,000-Task List switched to Custom: by choosing it {} ms, by a drag {} ms",
        choose.as_millis(),
        drag.as_millis()
    );
    assert!(choose.as_millis() < 2_000, "{choose:?}");
    assert!(drag.as_millis() < 2_000, "{drag:?}");
}

// Custom Order with Lists inside Lists (ADR 0009, Ticket 12): each List keeps
// a Custom Order of its own Tasks only, whatever its place in the tree.

fn sub_list(store: &TaskStore, name: &str, parent: &List) -> List {
    store
        .create_sub_list(CreateSubList {
            name: name.into(),
            parent_id: parent.id.clone(),
        })
        .unwrap()
}

fn list_now(store: &TaskStore, list: &List) -> List {
    store
        .list_lists(100, None, true)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == list.id)
        .unwrap()
}

/// Each Task of the List as stored: its place, revision and update time.
fn stored_tasks(db: &Path, list: &List) -> Vec<(String, i64, i64, i64)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT id, position, revision, updated_at FROM tasks WHERE list_id = ?1 ORDER BY id",
        )
        .unwrap();
    statement
        .query_map(params![list.id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn a_list_inside_a_list_keeps_a_custom_order_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("nested-custom.db");
    let store = store_at(&db);
    let work = list(&store, "Synthetic Work");
    let p1 = task_in(&store, "P1", Some(&work));
    task_in(&store, "P2", Some(&work));
    let reports = sub_list(&store, "Synthetic Reports", &work);
    let c1 = task_in(&store, "C1", Some(&reports));
    task_in(&store, "C2", Some(&reports));
    let c3 = task_in(&store, "C3", Some(&reports));
    assert_eq!(custom(&store, Some(&work)), ["P2", "P1"]);
    assert_eq!(custom(&store, Some(&reports)), ["C3", "C2", "C1"]);
    let parent_before = stored_tasks(&db, &work);

    // A move inside the Sub-List rewrites its own sequence only, and only
    // the Sub-List switches to Custom.
    after(&store, &c3, Some(&c1)).unwrap();
    assert_eq!(custom(&store, Some(&reports)), ["C2", "C1", "C3"]);
    assert_eq!(custom(&store, Some(&work)), ["P2", "P1"]);
    assert_eq!(stored_tasks(&db, &work), parent_before);
    assert_eq!(
        sort_of(&store, &view_of(Some(&reports))),
        Some(SortMode::Custom)
    );
    assert_eq!(sort_of(&store, &view_of(Some(&work))), None);

    // Nesting never rolls Tasks up: a parent's Task is no anchor in its
    // Sub-List, nor the other way round.
    for (moving, anchor) in [(&c1, &p1), (&p1, &c1)] {
        let err = after(&store, moving, Some(anchor)).unwrap_err();
        assert!(matches!(err, TaskError::TaskOrderAnchorChanged), "{err:?}");
        let err = reorder(&store, moving, None, Some(anchor), None).unwrap_err();
        assert_eq!(err.code(), "revision_conflict");
    }

    // A Task moved from the parent into its Sub-List lands at the top there.
    store
        .move_task(MoveTask {
            id: p1.id.clone(),
            expected_revision: now(&store, &p1).revision,
            list_id: Some(reports.id.clone()),
        })
        .unwrap();
    assert_eq!(custom(&store, Some(&reports)), ["P1", "C2", "C1", "C3"]);
    assert_eq!(custom(&store, Some(&work)), ["P2"]);
}

#[test]
fn moving_a_list_to_another_parent_keeps_its_custom_order_and_its_sort() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("moved-custom.db");
    let store = store_at(&db);
    let work = list(&store, "Synthetic Work");
    let home = list(&store, "Synthetic Home");
    let reports = sub_list(&store, "Synthetic Reports", &work);
    let r1 = task_in(&store, "R1", Some(&reports));
    task_in(&store, "R2", Some(&reports));
    let r3 = task_in(&store, "R3", Some(&reports));
    after(&store, &r3, Some(&r1)).unwrap();
    store
        .set_view_sort(&view_of(Some(&work)), SortMode::Difficulty)
        .unwrap();
    let before = stored_tasks(&db, &reports);
    let sorts_before = store.view_sorts().unwrap();

    // Into another List, then to the top level by a sidebar drop: the List's
    // id is its view's key, so its remembered sort follows it.
    store
        .move_list(MoveList {
            id: reports.id.clone(),
            expected_revision: list_now(&store, &reports).revision,
            parent_id: Some(home.id.clone()),
        })
        .unwrap();
    assert_eq!(
        list_now(&store, &reports).parent_id.as_deref(),
        Some(home.id.as_str())
    );
    assert_eq!(stored_tasks(&db, &reports), before);
    assert_eq!(custom(&store, Some(&reports)), ["R2", "R1", "R3"]);
    store
        .place_list(PlaceList {
            id: reports.id.clone(),
            expected_revision: list_now(&store, &reports).revision,
            parent_id: None,
            after_id: None,
        })
        .unwrap();
    assert_eq!(list_now(&store, &reports).parent_id, None);
    assert_eq!(stored_tasks(&db, &reports), before);
    assert_eq!(store.view_sorts().unwrap(), sorts_before);
    assert_eq!(
        sort_of(&store, &view_of(Some(&reports))),
        Some(SortMode::Custom)
    );

    // Its Custom Order is still the one to move within.
    after(&store, &r1, None).unwrap();
    assert_eq!(custom(&store, Some(&reports)), ["R1", "R2", "R3"]);
}

#[test]
fn tasks_inside_a_removed_list_leave_every_custom_order_and_come_back_in_theirs() {
    let (_dir, store) = fresh();
    let work = list(&store, "Synthetic Work");
    let reports = sub_list(&store, "Synthetic Reports", &work);
    let weekly = sub_list(&store, "Synthetic Weekly", &reports);
    let w1 = task_in(&store, "W1", Some(&weekly));
    task_in(&store, "W2", Some(&weekly));
    let w3 = task_in(&store, "W3", Some(&weekly));
    task_in(&store, "R1", Some(&reports));
    task_in(&store, "Inbox one", None);
    after(&store, &w3, Some(&w1)).unwrap();
    assert_eq!(custom(&store, Some(&weekly)), ["W2", "W1", "W3"]);

    store
        .set_list_removed(SetListRemoved {
            id: work.id.clone(),
            expected_revision: list_now(&store, &work).revision,
            removed: true,
        })
        .unwrap();
    // Neither List inside is in Removed itself, yet both are hidden with it:
    // no Custom listing shows their Tasks, and none can be rearranged.
    assert!(!list_now(&store, &weekly).removed);
    for hidden in [&reports, &weekly] {
        let err = store
            .query_tasks(
                &TaskQuery {
                    view: view_of(Some(hidden)),
                    sort: SortMode::Custom,
                    ..TaskQuery::default()
                },
                100,
                None,
            )
            .unwrap_err();
        assert_eq!(err.code(), "removed");
        let err = store
            .set_view_sort(&view_of(Some(hidden)), SortMode::Custom)
            .unwrap_err();
        assert_eq!(err.code(), "removed");
    }
    assert_eq!(custom(&store, None), ["Inbox one"]);
    assert_eq!(
        titles(&store, TaskView::All, SortMode::Priority),
        ["Inbox one"]
    );
    let err = after(&store, &w1, None).unwrap_err();
    assert!(matches!(err, TaskError::TaskListRemoved), "{err:?}");
    let err = after(&store, &w3, Some(&w1)).unwrap_err();
    assert_eq!(err.code(), "removed");

    store
        .set_list_removed(SetListRemoved {
            id: work.id.clone(),
            expected_revision: list_now(&store, &work).revision,
            removed: false,
        })
        .unwrap();
    assert_eq!(custom(&store, Some(&weekly)), ["W2", "W1", "W3"]);
    assert_eq!(
        sort_of(&store, &view_of(Some(&weekly))),
        Some(SortMode::Custom)
    );
}
