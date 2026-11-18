//! Hide Completed (batch 4, owner decision 2026-11-17): a view can leave its
//! Completed Tasks (Won't Do included) out of its pages, and count them with
//! the same filters, so "Show Completed (N)" says how many are hidden.

use std::path::Path;

use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, Priority, Task, TaskQuery, TaskStore, TaskView, WorkStatus,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn make(
    store: &TaskStore,
    title: &str,
    list: Option<&str>,
    progress: Option<u8>,
    priority: Priority,
) -> Task {
    let created = store
        .create_task_in_list(
            CreateTask {
                title: title.into(),
                description: String::new(),
            },
            list,
        )
        .unwrap();
    if progress.is_none() && priority == Priority::Medium {
        return created;
    }
    store
        .change_task(ChangeTask {
            id: created.id,
            expected_revision: created.revision,
            progress,
            priority: Some(priority),
            ..Default::default()
        })
        .unwrap()
}

fn titles(store: &TaskStore, query: &TaskQuery) -> Vec<String> {
    let mut all = Vec::new();
    let mut cursor = None;
    loop {
        let page = store.query_tasks(query, 2, cursor.as_deref()).unwrap();
        all.extend(page.items.into_iter().map(|task| task.title));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return all,
        }
    }
}

#[test]
fn a_view_can_leave_its_completed_tasks_out_and_count_them_with_its_filters() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("hide.db"));
    let work = store
        .create_list(CreateList {
            name: "Synthetic work".into(),
        })
        .unwrap();
    use Priority::*;
    make(&store, "Work started", Some(&work.id), Some(40), High);
    make(&store, "Work waiting", Some(&work.id), None, Medium);
    make(&store, "Work done high", Some(&work.id), Some(100), High);
    make(&store, "Work done low", Some(&work.id), Some(100), Low);
    let set_aside = make(&store, "Work set aside", Some(&work.id), None, Medium);
    store
        .change_task(ChangeTask {
            id: set_aside.id,
            expected_revision: set_aside.revision,
            wont_do: Some(true),
            ..Default::default()
        })
        .unwrap();
    make(&store, "Inbox done", None, Some(100), Medium);

    let view = TaskView::List(work.id.clone());
    let hidden = TaskQuery {
        view: view.clone(),
        hide_completed: true,
        ..TaskQuery::default()
    };
    // Every page leaves Completed out, Won't Do included, across cursors.
    assert_eq!(titles(&store, &hidden), ["Work started", "Work waiting"]);

    // The count uses the same view and filters as the page would.
    let completed = TaskQuery {
        view: view.clone(),
        status: Some(WorkStatus::Completed),
        ..TaskQuery::default()
    };
    assert_eq!(store.count_tasks(&completed).unwrap(), 3);
    let high = TaskQuery {
        priority: Some(High),
        ..completed.clone()
    };
    assert_eq!(store.count_tasks(&high).unwrap(), 1);
    let everywhere = TaskQuery {
        status: Some(WorkStatus::Completed),
        ..TaskQuery::default()
    };
    assert_eq!(store.count_tasks(&everywhere).unwrap(), 4);

    // A cursor from the shown query does not continue the hidden one.
    let shown = TaskQuery {
        view,
        ..TaskQuery::default()
    };
    let first = store.query_tasks(&shown, 1, None).unwrap();
    assert!(
        store
            .query_tasks(&hidden, 1, first.next_cursor.as_deref())
            .is_err()
    );
}
