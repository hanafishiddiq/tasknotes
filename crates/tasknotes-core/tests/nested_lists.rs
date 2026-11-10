//! Lists inside Lists (ADR 0009, Ticket 12): five levels at most, the Note
//! Folder tree rule, each List showing only its own Tasks, and the container
//! rule of ADR 0002 for Removed. Synthetic data only.

use std::path::Path;

use rusqlite::Connection;

use tasknotes_core::audit::Interface;
use tasknotes_core::bulk::{ApprovalKind, BulkAction};
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::note_folders::{CreateNoteFolder, MAX_FOLDER_DEPTH};
use tasknotes_core::removed::RemovedKind;
use tasknotes_core::tasks::{
    CreateList, CreateSubList, CreateTask, List, ListParent, MoveList, MoveTask, PlaceList,
    RenameList, ReorderList, SCHEMA_VERSION, SetListRemoved, SortMode, Task, TaskError, TaskStore,
    TaskView,
};
use tasknotes_core::tree::MAX_DEPTH;

struct World {
    _dir: tempfile::TempDir,
    desktop: TaskStore,
    ai: TaskStore,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested.db");
    World {
        desktop: TaskStore::open(&path).unwrap(),
        ai: TaskStore::open_as(&path, Interface::Mcp).unwrap(),
        _dir: dir,
    }
}

fn top(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn sub(store: &TaskStore, name: &str, parent: &List) -> Result<List, TaskError> {
    store.create_sub_list(CreateSubList {
        name: name.into(),
        parent_id: parent.id.clone(),
    })
}

fn now(store: &TaskStore, list: &List) -> List {
    tree(store, true)
        .into_iter()
        .find(|item| item.id == list.id)
        .unwrap()
}

fn tree(store: &TaskStore, include_removed: bool) -> Vec<List> {
    store.list_lists(100, None, include_removed).unwrap().items
}

/// Each shown List as `name<parent name`, in the order the tree is read.
fn shape(store: &TaskStore) -> Vec<String> {
    let lists = tree(store, false);
    lists
        .iter()
        .map(|list| match &list.parent_id {
            None => list.name.clone(),
            Some(parent) => {
                let parent = lists.iter().find(|p| &p.id == parent).unwrap();
                format!("{}<{}", list.name, parent.name)
            }
        })
        .collect()
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

fn titles(store: &TaskStore, view: TaskView) -> Vec<String> {
    store
        .list_tasks_in_view(100, None, SortMode::Newest, view)
        .unwrap()
        .items
        .into_iter()
        .map(|task| task.title)
        .collect()
}

fn remove(store: &TaskStore, list: &List, removed: bool) -> Result<List, TaskError> {
    store.set_list_removed(SetListRemoved {
        id: list.id.clone(),
        expected_revision: now(store, list).revision,
        removed,
    })
}

fn move_to(store: &TaskStore, list: &List, parent: Option<&List>) -> Result<List, TaskError> {
    store.move_list(MoveList {
        id: list.id.clone(),
        expected_revision: now(store, list).revision,
        parent_id: parent.map(|p| p.id.clone()),
    })
}

fn place(
    store: &TaskStore,
    list: &List,
    parent: Option<&List>,
    after: Option<&List>,
) -> Result<List, TaskError> {
    store.place_list(PlaceList {
        id: list.id.clone(),
        expected_revision: now(store, list).revision,
        parent_id: parent.map(|p| p.id.clone()),
        after_id: after.map(|a| a.id.clone()),
    })
}

/// A chain of Lists, each inside the one before; the first is top-level.
fn chain(store: &TaskStore, levels: i64) -> Vec<List> {
    let mut lists = vec![top(store, "Level 1")];
    for level in 2..=levels {
        let parent = lists.last().unwrap().clone();
        lists.push(sub(store, &format!("Level {level}"), &parent).unwrap());
    }
    lists
}

#[test]
fn a_sub_list_starts_empty_and_each_list_shows_only_its_own_tasks() {
    let w = world();
    let work = top(&w.desktop, "Synthetic Work");
    task_in(&w.desktop, "Synthetic parent task", &work);
    let reports = sub(&w.desktop, "Synthetic Reports", &work).unwrap();

    assert_eq!(reports.parent_id.as_deref(), Some(work.id.as_str()));
    assert_eq!(reports.revision, 1);
    assert!(titles(&w.desktop, TaskView::List(reports.id.clone())).is_empty());
    task_in(&w.desktop, "Synthetic child task", &reports);
    assert_eq!(
        titles(&w.desktop, TaskView::List(work.id.clone())),
        ["Synthetic parent task"],
        "nesting never rolls a child's Tasks up"
    );
    assert_eq!(
        titles(&w.desktop, TaskView::List(reports.id.clone())),
        ["Synthetic child task"]
    );
    assert!(now(&w.desktop, &work).has_children);
    assert!(!now(&w.desktop, &reports).has_children);
}

#[test]
fn the_tree_reads_parents_before_children_in_manual_order_across_pages() {
    let w = world();
    let a = top(&w.desktop, "A");
    let b = top(&w.desktop, "B");
    let a1 = sub(&w.desktop, "A1", &a).unwrap();
    let a2 = sub(&w.desktop, "A2", &a).unwrap();
    sub(&w.desktop, "A1x", &a1).unwrap();
    sub(&w.desktop, "B1", &b).unwrap();
    place(&w.desktop, &a2, Some(&a), None).unwrap();

    assert_eq!(
        shape(&w.desktop),
        ["A", "A2<A", "A1<A", "A1x<A1", "B", "B1<B"]
    );
    // Page by page, the same order.
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = w.desktop.list_lists(2, cursor.as_deref(), false).unwrap();
        seen.extend(page.items.into_iter().map(|l| l.name));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(seen, ["A", "A2", "A1", "A1x", "B", "B1"]);
    assert!(matches!(
        w.desktop.list_lists(2, Some("v3|1|x|active"), false),
        Err(TaskError::InvalidListCursor)
    ));
    // Reordering keeps a List among the siblings it has.
    w.desktop
        .reorder_list(ReorderList {
            id: a1.id.clone(),
            expected_revision: now(&w.desktop, &a1).revision,
            after_id: None,
            parent: ListParent::Current,
        })
        .unwrap();
    assert_eq!(
        shape(&w.desktop),
        ["A", "A1<A", "A1x<A1", "A2<A", "B", "B1<B"]
    );
}

#[test]
fn lists_nest_five_levels_at_most_by_the_folder_rule() {
    assert_eq!(MAX_DEPTH, 5);
    assert_eq!(MAX_FOLDER_DEPTH, MAX_DEPTH, "one limit for both trees");
    let w = world();
    let levels = chain(&w.desktop, 5);
    let too_deep = sub(&w.desktop, "Level 6", &levels[4]).unwrap_err();
    assert!(matches!(too_deep, TaskError::ListTooDeep), "{too_deep:?}");
    assert_eq!(too_deep.code(), "invalid_input");

    // Moving a List with one inside it under level 4 would make level 6.
    let pair = top(&w.desktop, "Pair");
    sub(&w.desktop, "Pair child", &pair).unwrap();
    assert!(matches!(
        move_to(&w.desktop, &pair, Some(&levels[3])),
        Err(TaskError::ListTooDeep)
    ));
    assert!(matches!(
        place(&w.desktop, &pair, Some(&levels[3]), None),
        Err(TaskError::ListTooDeep)
    ));
    // Under level 3 it fits exactly.
    move_to(&w.desktop, &pair, Some(&levels[2])).unwrap();
    assert_eq!(
        now(&w.desktop, &pair).parent_id.as_deref(),
        Some(levels[2].id.as_str())
    );
}

#[test]
fn a_list_cannot_land_inside_itself() {
    let w = world();
    let levels = chain(&w.desktop, 3);
    for target in [&levels[0], &levels[1], &levels[2]] {
        let refused = move_to(&w.desktop, &levels[0], Some(target)).unwrap_err();
        assert!(
            matches!(refused, TaskError::ListCycle),
            "{} into {}: {refused:?}",
            levels[0].name,
            target.name
        );
    }
    assert!(matches!(
        place(&w.desktop, &levels[0], Some(&levels[2]), None),
        Err(TaskError::ListCycle)
    ));
    // And out again to the top level, by move or by drop.
    move_to(&w.desktop, &levels[2], None).unwrap();
    assert_eq!(now(&w.desktop, &levels[2]).parent_id, None);
    place(&w.desktop, &levels[2], Some(&levels[0]), Some(&levels[1])).unwrap();
    assert_eq!(
        shape(&w.desktop),
        ["Level 1", "Level 2<Level 1", "Level 3<Level 1"]
    );
    assert!(matches!(
        move_to(&w.desktop, &levels[2], Some(&levels[0])),
        Err(TaskError::ListUnchanged)
    ));
}

#[test]
fn inbox_stays_on_top_and_holds_no_lists() {
    let w = world();
    let list = top(&w.desktop, "Synthetic");
    let folder = w
        .desktop
        .create_note_folder(CreateNoteFolder {
            name: "Synthetic folder".into(),
            parent_id: None,
        })
        .unwrap();
    // Inbox is no List: nothing can be put inside it, nor anything else that is not a List.
    for parent in ["inbox", "Inbox", folder.id.as_str()] {
        let made = w.desktop.create_sub_list(CreateSubList {
            name: "Inside".into(),
            parent_id: parent.into(),
        });
        assert!(matches!(made, Err(TaskError::ListNotFound)), "{parent}");
        let moved = w.desktop.move_list(MoveList {
            id: list.id.clone(),
            expected_revision: list.revision,
            parent_id: Some(parent.into()),
        });
        assert!(matches!(moved, Err(TaskError::ListNotFound)), "{parent}");
    }
    // A Task in no List is in Inbox, wherever its Lists sit.
    let parent = top(&w.desktop, "Parent");
    move_to(&w.desktop, &list, Some(&parent)).unwrap();
    w.desktop
        .create_task(CreateTask {
            title: "Synthetic unsorted".into(),
            description: String::new(),
        })
        .unwrap();
    assert_eq!(titles(&w.desktop, TaskView::Inbox), ["Synthetic unsorted"]);
}

/// Owner decision Q33 (2026-11-10): like Note Folders, a List's name is
/// unique, ignoring case, only among the Lists that share its parent; the
/// top-level Lists are siblings of each other.
#[test]
fn names_are_unique_among_siblings_ignoring_case() {
    let w = world();
    let it = top(&w.desktop, "Adamaja IT");
    let personal = top(&w.desktop, "Personal");
    let reports = sub(&w.desktop, "Reports", &it).unwrap();
    let own = sub(&w.desktop, "reports", &personal).unwrap();
    // A top-level List may share a name with a List inside another.
    let top_reports = top(&w.desktop, "Reports");
    assert_eq!(
        shape(&w.desktop),
        [
            "Adamaja IT",
            "Reports<Adamaja IT",
            "Personal",
            "reports<Personal",
            "Reports"
        ]
    );

    // Among siblings the name is taken, whatever its case.
    for refused in [
        sub(&w.desktop, "REPORTS", &it),
        w.desktop.create_list(CreateList {
            name: "personal".into(),
        }),
        w.desktop.rename_list(RenameList {
            id: personal.id.clone(),
            expected_revision: personal.revision,
            name: "ADAMAJA it".into(),
        }),
    ] {
        let error = refused.unwrap_err();
        assert!(matches!(error, TaskError::ListNameConflict), "{error:?}");
        assert_eq!(error.code(), "name_conflict");
        assert_eq!(
            error.to_string(),
            "A List with this name already exists here"
        );
    }

    // A move or a drop into a parent that has a sibling of that name is refused.
    assert!(matches!(
        move_to(&w.desktop, &own, Some(&it)),
        Err(TaskError::ListNameConflict)
    ));
    assert!(matches!(
        place(&w.desktop, &own, Some(&it), Some(&reports)),
        Err(TaskError::ListNameConflict)
    ));
    // The top level already has a Reports.
    assert!(matches!(
        move_to(&w.desktop, &reports, None),
        Err(TaskError::ListNameConflict)
    ));
    assert_eq!(
        now(&w.desktop, &own).parent_id.as_deref(),
        Some(personal.id.as_str())
    );

    // A List in Removed holds no name; restoring it needs its own back.
    remove(&w.desktop, &top_reports, true).unwrap();
    move_to(&w.desktop, &reports, None).unwrap();
    assert!(matches!(
        remove(&w.desktop, &top_reports, false),
        Err(TaskError::ListNameConflict)
    ));
    // Renaming among other siblings is free, and moving then fits.
    w.desktop
        .rename_list(RenameList {
            id: own.id.clone(),
            expected_revision: now(&w.desktop, &own).revision,
            name: "Reports".into(),
        })
        .unwrap();
    move_to(&w.desktop, &own, Some(&it)).unwrap();
}

#[test]
fn removing_a_list_hides_the_lists_and_tasks_inside_until_it_comes_back() {
    let w = world();
    let work = top(&w.desktop, "Work");
    let reports = sub(&w.desktop, "Reports", &work).unwrap();
    let weekly = sub(&w.desktop, "Weekly", &reports).unwrap();
    let other = top(&w.desktop, "Other");
    let deep = task_in(&w.desktop, "Synthetic deep task", &weekly);
    let own = task_in(&w.desktop, "Synthetic removed on its own", &reports);
    w.desktop
        .set_task_removed(tasknotes_core::removed::SetRemoved {
            id: own.id.clone(),
            expected_revision: own.revision,
            removed: true,
        })
        .unwrap();
    let floating = task_in(&w.desktop, "Synthetic floating", &other);

    remove(&w.desktop, &work, true).unwrap();

    assert_eq!(shape(&w.desktop), ["Other"]);
    assert!(
        titles(&w.desktop, TaskView::All)
            .iter()
            .all(|t| t != "Synthetic deep task")
    );
    // Nothing can be written into a hidden List or Task.
    assert!(matches!(
        w.desktop.create_task_in_list(
            CreateTask {
                title: "Into hidden".into(),
                description: String::new()
            },
            Some(&weekly.id)
        ),
        Err(TaskError::ListRemoved)
    ));
    assert!(matches!(
        w.desktop.move_task(MoveTask {
            id: floating.id.clone(),
            expected_revision: floating.revision,
            list_id: Some(weekly.id.clone()),
        }),
        Err(TaskError::ListRemoved)
    ));
    assert!(matches!(
        w.desktop.get_task(&deep.id).map(|t| t.is_some()),
        Ok(true)
    ));
    assert!(matches!(
        w.desktop.move_task(MoveTask {
            id: deep.id.clone(),
            expected_revision: deep.revision,
            list_id: None,
        }),
        Err(TaskError::TaskListRemoved)
    ));
    assert!(matches!(
        sub(&w.desktop, "New inside", &reports),
        Err(TaskError::ListRemoved)
    ));
    assert!(matches!(
        move_to(&w.desktop, &other, Some(&weekly)),
        Err(TaskError::ListRemoved)
    ));
    assert!(matches!(
        move_to(&w.desktop, &weekly, None),
        Err(TaskError::ListRemoved)
    ));
    // Inside a Removed List, a List is neither removed nor restored alone.
    assert!(matches!(
        remove(&w.desktop, &reports, true),
        Err(TaskError::ListRemoved)
    ));
    // The Removed view lists only the List removed on its own; the Task
    // removed on its own is marked as waiting for its container.
    let removed = w.desktop.list_removed("", 100, None).unwrap().items;
    let kinds: Vec<(RemovedKind, &str, bool)> = removed
        .iter()
        .map(|i| (i.kind, i.title.as_str(), i.in_removed_container))
        .collect();
    assert!(kinds.contains(&(RemovedKind::List, "Work", false)));
    assert!(kinds.contains(&(RemovedKind::Task, "Synthetic removed on its own", true)));
    assert!(!kinds.iter().any(|(_, title, _)| *title == "Reports"));

    remove(&w.desktop, &work, false).unwrap();
    assert_eq!(
        shape(&w.desktop),
        ["Work", "Reports<Work", "Weekly<Reports", "Other"]
    );
    assert_eq!(
        titles(&w.desktop, TaskView::List(weekly.id.clone())),
        ["Synthetic deep task"]
    );
    assert!(
        titles(&w.desktop, TaskView::List(reports.id.clone())).is_empty(),
        "what was Removed on its own stays Removed"
    );
}

#[test]
fn a_list_removed_inside_a_removed_list_waits_for_it() {
    let w = world();
    let work = top(&w.desktop, "Work");
    let reports = sub(&w.desktop, "Reports", &work).unwrap();
    remove(&w.desktop, &reports, true).unwrap();
    remove(&w.desktop, &work, true).unwrap();
    let removed = w.desktop.list_removed("", 100, None).unwrap().items;
    let reports_item = removed.iter().find(|i| i.id == reports.id).unwrap();
    assert!(reports_item.in_removed_container);
    assert!(matches!(
        remove(&w.desktop, &reports, false),
        Err(TaskError::ListRemoved)
    ));
    remove(&w.desktop, &work, false).unwrap();
    assert_eq!(shape(&w.desktop), ["Work"], "Reports stays in Removed");
    remove(&w.desktop, &reports, false).unwrap();
    assert_eq!(shape(&w.desktop), ["Work", "Reports<Work"]);
}

#[test]
fn through_ai_removing_a_list_that_holds_a_list_needs_approval() {
    let w = world();
    let empty = top(&w.ai, "Synthetic empty");
    remove(&w.ai, &empty, true).unwrap();
    let work = top(&w.ai, "Work");
    let reports = sub(&w.ai, "Reports", &work).unwrap();
    task_in(&w.ai, "Synthetic nested task", &reports);

    let refused = remove(&w.ai, &work, true).unwrap_err();
    assert!(matches!(refused, TaskError::NeedsApproval), "{refused:?}");
    assert!(!now(&w.ai, &work).removed);

    let requested =
        w.ai.request_bulk(BulkAction::RemoveList, vec![work.id.clone()])
            .unwrap();
    let pending = &w.desktop.pending_bulk_requests().unwrap()[0];
    assert_eq!(pending.target.as_deref(), Some("Work"));
    let lines: Vec<(ApprovalKind, &str)> = pending
        .items
        .iter()
        .map(|l| (l.kind, l.title.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            (ApprovalKind::List, "Reports"),
            (ApprovalKind::Task, "Synthetic nested task")
        ],
        "the person sees everything the removal hides"
    );
    w.desktop
        .approve_bulk_request(&requested.request.id)
        .unwrap();
    assert!(now(&w.desktop, &work).removed);
    assert_eq!(shape(&w.desktop), Vec::<String>::new());
    // The desktop removes directly.
    remove(&w.desktop, &work, false).unwrap();
    remove(&w.desktop, &work, true).unwrap();
}

#[test]
fn moves_are_revisioned_and_audited_as_list_move_or_reorder() {
    let w = world();
    let work = top(&w.desktop, "Work");
    let home = top(&w.desktop, "Home");
    let errands = sub(&w.desktop, "Errands", &work).unwrap();
    let chores = sub(&w.desktop, "Chores", &work).unwrap();

    let stale = w.ai.move_list(MoveList {
        id: errands.id.clone(),
        expected_revision: errands.revision + 7,
        parent_id: Some(home.id.clone()),
    });
    assert!(matches!(stale, Err(TaskError::RevisionConflict)));

    let moved = move_to(&w.ai, &errands, Some(&home)).unwrap();
    assert_eq!(moved.revision, errands.revision + 1);
    let placed = place(&w.ai, &chores, Some(&home), Some(&errands)).unwrap();
    assert_eq!(placed.parent_id.as_deref(), Some(home.id.as_str()));
    place(&w.ai, &chores, Some(&home), None).unwrap();
    // Neighbours keep their revisions.
    assert_eq!(now(&w.ai, &errands).revision, moved.revision);

    let actions = |id: &str| -> Vec<(String, String)> {
        let mut entries = w
            .desktop
            .list_audit_entries(Some(id), 20, None)
            .unwrap()
            .items;
        entries.reverse();
        entries
            .into_iter()
            .filter(|e| e.interface == Interface::Mcp)
            .map(|e| (e.action.as_str().to_owned(), e.outcome.as_str().to_owned()))
            .collect()
    };
    assert_eq!(
        actions(&errands.id),
        [
            ("list.move".to_owned(), "rejected".to_owned()),
            ("list.move".to_owned(), "applied".to_owned())
        ]
    );
    assert_eq!(
        actions(&chores.id),
        [
            ("list.move".to_owned(), "applied".to_owned()),
            ("list.reorder".to_owned(), "applied".to_owned())
        ]
    );
}

#[test]
fn exports_keep_the_nesting() {
    let w = world();
    let work = top(&w.desktop, "Work");
    let reports = sub(&w.desktop, "Reports", &work).unwrap();
    task_in(&w.desktop, "Synthetic weekly report", &reports);
    task_in(&w.desktop, "Synthetic work task", &work);
    let out = tempfile::tempdir().unwrap();

    let json = out.path().join("export.json");
    w.desktop
        .export_data(ExportRequest {
            format: ExportFormat::Json,
            destination: json.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    let lists = doc["lists"].as_array().unwrap();
    let parent_of =
        |name: &str| lists.iter().find(|l| l["name"] == name).unwrap()["parent_id"].clone();
    assert_eq!(parent_of("Work"), serde_json::Value::Null);
    assert_eq!(parent_of("Reports"), serde_json::json!(work.id));

    let md = out.path().join("markdown");
    w.desktop
        .export_data(ExportRequest {
            format: ExportFormat::Markdown,
            destination: md.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let work_file = std::fs::read_to_string(md.join("Tasks/Work.md")).unwrap();
    assert!(work_file.contains("Synthetic work task"));
    assert!(!work_file.contains("Synthetic weekly report"));
    let reports_file = std::fs::read_to_string(md.join("Tasks/Work/Reports.md")).unwrap();
    assert!(reports_file.contains("Synthetic weekly report"));
}

/// Puts a database back as schema v19 left it: Lists had no parent, and
/// their names were unique among all Lists not in Removed.
fn back_to_v19(db: &Path) {
    Connection::open(db)
        .unwrap()
        .execute_batch(
            "DROP INDEX lists_parent; DROP INDEX lists_manual_order;
             DROP INDEX lists_sibling_name; ALTER TABLE lists DROP COLUMN parent_id;
             CREATE UNIQUE INDEX lists_active_name ON lists(name COLLATE NOCASE)
               WHERE removed_at IS NULL;
             CREATE INDEX lists_manual_order ON lists(position, id);
             PRAGMA user_version = 19;",
        )
        .unwrap();
}

#[test]
fn upgrading_keeps_every_list_at_the_top_in_its_order_with_its_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade.db");
    let store = TaskStore::open(&db).unwrap();
    let a = top(&store, "Alpha");
    let b = top(&store, "Bravo");
    task_in(&store, "Synthetic in Bravo", &b);
    store
        .reorder_list(ReorderList {
            id: b.id.clone(),
            expected_revision: b.revision,
            after_id: None,
            parent: ListParent::Current,
        })
        .unwrap();
    drop(store);
    back_to_v19(&db);

    let store = TaskStore::open(&db).unwrap();
    let version: i64 = Connection::open(&db)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(shape(&store), ["Bravo", "Alpha"]);
    assert!(tree(&store, true).iter().all(|l| l.parent_id.is_none()));
    assert_eq!(
        titles(&store, TaskView::List(b.id.clone())),
        ["Synthetic in Bravo"]
    );
    sub(&store, "Alpha child", &a).unwrap();
    // Names that were unique among all Lists stay valid; from now on only
    // siblings must differ, so a List inside Alpha may be called Bravo.
    sub(&store, "Bravo", &a).unwrap();
    assert!(matches!(
        store.create_list(CreateList {
            name: "alpha".into()
        }),
        Err(TaskError::ListNameConflict)
    ));
}
