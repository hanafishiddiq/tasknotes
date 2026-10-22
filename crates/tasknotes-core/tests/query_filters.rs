use std::collections::HashSet;
use std::path::Path;

use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, Priority, SortMode, Task, TaskError, TaskQuery, TaskStore,
    TaskView, UpdateTaskDetails, WorkStatus,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn make(
    store: &TaskStore,
    title: &str,
    list: Option<&str>,
    status: WorkStatus,
    priority: Priority,
    tags: &[&str],
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
    let progress = match status {
        WorkStatus::Ready => None,
        WorkStatus::InProgress => Some(40),
        WorkStatus::Completed => Some(100),
    };
    // A new Task is already Ready/Medium; the core rejects a no-op change.
    let changed = if progress.is_none() && priority == Priority::Medium {
        created
    } else {
        store
            .change_task(ChangeTask {
                id: created.id,
                expected_revision: created.revision,
                status: None,
                priority: Some(priority),
                progress,
            })
            .unwrap()
    };
    if tags.is_empty() {
        return changed;
    }
    store
        .update_task_details(UpdateTaskDetails {
            id: changed.id,
            expected_revision: changed.revision,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            due_date: None,
        })
        .unwrap()
}

fn titles(store: &TaskStore, query: &TaskQuery) -> Vec<String> {
    store
        .query_tasks(query, 100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|task| task.title)
        .collect()
}

#[test]
fn status_priority_tag_and_list_filters_combine() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("filters.db"));
    let work = store
        .create_list(CreateList {
            name: "Work".into(),
        })
        .unwrap();
    use Priority::*;
    use WorkStatus::*;
    make(
        &store,
        "Inbox urgent draft",
        None,
        InProgress,
        High,
        &["Café"],
    );
    make(&store, "Inbox calm draft", None, Ready, Low, &["Café"]);
    make(
        &store,
        "Work urgent draft",
        Some(&work.id),
        InProgress,
        High,
        &["café"],
    );
    make(
        &store,
        "Work done draft",
        Some(&work.id),
        Completed,
        High,
        &["Other"],
    );

    let query = |status, priority, tag: Option<&str>, view| TaskQuery {
        text: "draft".into(),
        status,
        priority,
        tag: tag.map(str::to_owned),
        view,
        ..TaskQuery::default()
    };
    assert_eq!(
        titles(&store, &query(Some(InProgress), None, None, TaskView::All)),
        vec!["Work urgent draft", "Inbox urgent draft"]
    );
    assert_eq!(
        titles(
            &store,
            &query(None, Some(High), Some("CAFE"), TaskView::All)
        ),
        Vec::<String>::new(),
        "tag filter folds case but not accents: CAFE is not Café"
    );
    assert_eq!(
        titles(
            &store,
            &query(None, Some(High), Some("CAFÉ"), TaskView::All)
        ),
        vec!["Work urgent draft", "Inbox urgent draft"]
    );
    assert_eq!(
        titles(
            &store,
            &query(Some(InProgress), Some(High), Some("café"), TaskView::Inbox)
        ),
        vec!["Inbox urgent draft"]
    );
    assert_eq!(
        titles(
            &store,
            &query(None, None, None, TaskView::List(work.id.clone()))
        ),
        vec!["Work urgent draft", "Work done draft"]
    );
}

#[test]
fn status_groups_stay_fixed_under_both_sorts_while_filtering() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("order.db"));
    use Priority::*;
    use WorkStatus::*;
    make(
        &store,
        "A completed high",
        None,
        Completed,
        VeryHigh,
        &["x"],
    );
    make(&store, "B ready low", None, Ready, Low, &["x"]);
    make(
        &store,
        "C progress medium",
        None,
        InProgress,
        Medium,
        &["x"],
    );
    make(&store, "D ready high", None, Ready, High, &["x"]);
    make(
        &store,
        "E untagged progress",
        None,
        InProgress,
        VeryHigh,
        &[],
    );

    let by = |sort| TaskQuery {
        tag: Some("x".into()),
        sort,
        ..TaskQuery::default()
    };
    assert_eq!(
        titles(&store, &by(SortMode::Priority)),
        vec![
            "C progress medium",
            "D ready high",
            "B ready low",
            "A completed high"
        ]
    );
    assert_eq!(
        titles(&store, &by(SortMode::Newest)),
        vec![
            "C progress medium",
            "D ready high",
            "B ready low",
            "A completed high"
        ]
    );
}

#[test]
fn search_pages_never_repeat_or_skip_and_keep_group_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("paging.db"));
    let statuses = [
        WorkStatus::Ready,
        WorkStatus::InProgress,
        WorkStatus::Completed,
    ];
    let priorities = [Priority::Low, Priority::Medium, Priority::High];
    let mut expected = HashSet::new();
    for n in 0..130 {
        let title = if n % 4 == 0 {
            format!("Unrelated {n}")
        } else {
            format!("Report {n}")
        };
        let task = make(
            &store,
            &title,
            None,
            statuses[n % 3],
            priorities[(n / 3) % 3],
            &[],
        );
        if n % 4 != 0 {
            expected.insert(task.id);
        }
    }
    for sort in [SortMode::Priority, SortMode::Newest] {
        let query = TaskQuery {
            text: "report".into(),
            sort,
            ..TaskQuery::default()
        };
        let mut seen = Vec::new();
        let mut cursor = None;
        for pages in 1.. {
            // A paging bug must fail the test, not hang it.
            assert!(pages <= 20, "paging did not terminate");
            let page = store.query_tasks(&query, 7, cursor.as_deref()).unwrap();
            assert!(page.items.len() <= 7);
            seen.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let ids: HashSet<_> = seen.iter().map(|task| task.id.clone()).collect();
        assert_eq!(ids.len(), seen.len(), "no Task repeats across pages");
        assert_eq!(ids, expected, "no matching Task is skipped");
        let ranks: Vec<_> = seen
            .iter()
            .map(|task| match task.status {
                WorkStatus::InProgress => 0,
                WorkStatus::Ready => 1,
                WorkStatus::Completed => 2,
            })
            .collect();
        assert!(
            ranks.windows(2).all(|pair| pair[0] <= pair[1]),
            "groups stay fixed"
        );
    }
}

#[test]
fn a_cursor_only_continues_the_query_that_produced_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("cursor.db"));
    for n in 0..5 {
        make(
            &store,
            &format!("Report {n}"),
            None,
            WorkStatus::Ready,
            Priority::Medium,
            &["x"],
        );
    }
    let first = TaskQuery {
        text: "report".into(),
        ..TaskQuery::default()
    };
    let cursor = store
        .query_tasks(&first, 2, None)
        .unwrap()
        .next_cursor
        .unwrap();
    for other in [
        TaskQuery {
            text: "rep".into(),
            ..TaskQuery::default()
        },
        TaskQuery {
            text: "report".into(),
            tag: Some("x".into()),
            ..TaskQuery::default()
        },
        TaskQuery {
            text: "report".into(),
            status: Some(WorkStatus::Ready),
            ..TaskQuery::default()
        },
        TaskQuery {
            text: "report".into(),
            sort: SortMode::Newest,
            ..TaskQuery::default()
        },
        TaskQuery {
            text: "report".into(),
            view: TaskView::Inbox,
            ..TaskQuery::default()
        },
    ] {
        assert!(matches!(
            store.query_tasks(&other, 2, Some(&cursor)),
            Err(TaskError::InvalidCursor)
        ));
    }
    assert!(store.query_tasks(&first, 2, Some(&cursor)).is_ok());
}

#[test]
fn a_query_can_check_whether_one_task_matches_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("one.db"));
    let lease = make(
        &store,
        "Renew lease",
        None,
        WorkStatus::InProgress,
        Priority::High,
        &["Home"],
    );
    make(
        &store,
        "Renew passport",
        None,
        WorkStatus::InProgress,
        Priority::High,
        &["Home"],
    );
    let matches = |query: TaskQuery| {
        let page = store
            .query_tasks(
                &TaskQuery {
                    task_id: Some(lease.id.clone()),
                    ..query
                },
                1,
                None,
            )
            .unwrap();
        assert!(page.next_cursor.is_none());
        page.items
            .iter()
            .map(|task| task.title.clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(
        matches(TaskQuery {
            text: "renew".into(),
            status: Some(WorkStatus::InProgress),
            tag: Some("home".into()),
            ..TaskQuery::default()
        }),
        vec!["Renew lease"]
    );
    assert!(
        matches(TaskQuery {
            text: "passport".into(),
            ..TaskQuery::default()
        })
        .is_empty()
    );
    assert!(
        matches(TaskQuery {
            priority: Some(Priority::Low),
            ..TaskQuery::default()
        })
        .is_empty()
    );
    assert_eq!(
        matches(TaskQuery {
            view: TaskView::Inbox,
            ..TaskQuery::default()
        }),
        vec!["Renew lease"]
    );
}
