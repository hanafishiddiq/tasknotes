//! Functional check that search, filters and paging stay correct and page-bounded
//! on a large synthetic database. Timing is printed, not asserted: the measured
//! release benchmark belongs to Ticket 17.

use std::collections::HashSet;
use std::time::Instant;

use rusqlite::{Connection, params};
use tasknotes_core::tasks::{
    Priority, SortMode, Task, TaskQuery, TaskScope, TaskStore, WorkStatus,
};

const TASKS: usize = 6_000;
const LIST_A: &str = "11111111-1111-4111-8111-111111111111";
const REMOVED_LIST: &str = "22222222-2222-4222-8222-222222222222";
const STATUSES: [&str; 3] = ["ready", "in_progress", "completed"];
const PRIORITIES: [&str; 5] = ["very_low", "low", "medium", "high", "very_high"];

fn id(n: usize) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}
fn status(n: usize) -> &'static str {
    STATUSES[n % 3]
}
fn priority(n: usize) -> &'static str {
    PRIORITIES[(n / 3) % 5]
}
fn is_report(n: usize) -> bool {
    n.is_multiple_of(2)
}
fn list_id(n: usize) -> Option<&'static str> {
    match n % 10 {
        0 => Some(LIST_A),
        1 => Some(REMOVED_LIST),
        _ => None,
    }
}
fn removed(n: usize) -> bool {
    n % 50 == 8
}
fn tagged(n: usize) -> bool {
    n.is_multiple_of(4)
}
/// Visible in the Active scope: not Removed and not inside a removed List.
fn active(n: usize) -> bool {
    !removed(n) && list_id(n) != Some(REMOVED_LIST)
}

fn seed(path: &std::path::Path) {
    drop(TaskStore::open(path).unwrap());
    let mut conn = Connection::open(path).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute_batch(&format!(
        "INSERT INTO lists(id,name,revision,created_at,updated_at,removed_at)
         VALUES('{LIST_A}','Projects',1,1,1,NULL),('{REMOVED_LIST}','Old',1,1,1,1);
         INSERT INTO tags(name, fold) VALUES('Work','work');"
    ))
    .unwrap();
    for n in 0..TASKS {
        let progress = match status(n) {
            "ready" => 0,
            "in_progress" => 40,
            _ => 100,
        };
        let title = if is_report(n) {
            format!("Weekly report {n}")
        } else {
            format!("Errand {n}")
        };
        let description = if n == 1234 {
            "Book flights to Zanzibar"
        } else {
            ""
        };
        let next_step = if n == 4322 {
            "Confirm zanzibar hotel"
        } else {
            ""
        };
        tx.execute(
            "INSERT INTO tasks(id,title,description,status,priority,progress,revision,
                created_at,updated_at,previous_incomplete_progress,next_step,list_id,removed_at)
             VALUES(?1,?2,?3,?4,?5,?6,1,?7,?7,0,?8,?9,?10)",
            params![
                id(n),
                title,
                description,
                status(n),
                priority(n),
                progress,
                1_000 + n as i64,
                next_step,
                list_id(n),
                removed(n).then_some(1_i64),
            ],
        )
        .unwrap();
        if tagged(n) {
            tx.execute(
                "INSERT INTO task_tags(task_id, tag_name) VALUES(?1,'Work')",
                [id(n)],
            )
            .unwrap();
        }
        if n % 3 == 0 {
            let note = if n == 5556 {
                "Asked about Zanzibar visas"
            } else {
                "Checked in"
            };
            for (seq, text) in [(0, note), (1, "Later follow-up")] {
                tx.execute(
                    "INSERT INTO progress_entries(id,task_id,previous_status,status,
                        previous_progress,progress,note,next_step,created_at)
                     VALUES(?1,?2,?3,?3,0,0,?4,'',?5)",
                    params![
                        format!("{n:08x}-{seq:04x}-4000-8000-000000000001"),
                        id(n),
                        status(n),
                        text,
                        2_000 + n as i64,
                    ],
                )
                .unwrap();
            }
        }
    }
    tx.commit().unwrap();
}

fn all_pages(store: &TaskStore, query: &TaskQuery) -> Vec<Task> {
    let mut seen = Vec::new();
    let mut cursor = None;
    for pages in 1.. {
        assert!(pages <= TASKS / 50 + 2, "paging did not terminate");
        let page = store.query_tasks(query, 50, cursor.as_deref()).unwrap();
        assert!(page.items.len() <= 50, "a page is bounded");
        seen.extend(page.items);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    seen
}

fn assert_complete(seen: &[Task], expected: &HashSet<String>) {
    let ids: HashSet<_> = seen.iter().map(|task| task.id.clone()).collect();
    assert_eq!(ids.len(), seen.len(), "no Task repeats across pages");
    assert_eq!(&ids, expected, "no matching Task is skipped");
    let rank = |task: &Task| match task.status {
        WorkStatus::InProgress => 0,
        WorkStatus::Ready => 1,
        WorkStatus::Completed => 2,
    };
    assert!(seen.windows(2).all(|pair| rank(&pair[0]) <= rank(&pair[1])));
}

#[test]
fn search_and_filters_stay_correct_and_bounded_on_a_large_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("large.db");
    let started = Instant::now();
    seed(&db);
    eprintln!("seeded {TASKS} Tasks in {:?}", started.elapsed());
    let store = TaskStore::open(&db).unwrap();

    // A rare word in a description, a Next Step and an older Progress note.
    let started = Instant::now();
    let rare = store
        .query_tasks(
            &TaskQuery {
                text: "zanzibar".into(),
                ..TaskQuery::default()
            },
            50,
            None,
        )
        .unwrap();
    eprintln!("rare-word first page: {:?}", started.elapsed());
    let rare_ids: HashSet<_> = rare.items.iter().map(|task| task.id.clone()).collect();
    assert_eq!(rare_ids, [1234, 4322, 5556].map(id).into_iter().collect());

    // Broad search across both sorts.
    let expected: HashSet<_> = (0..TASKS)
        .filter(|&n| is_report(n) && active(n))
        .map(id)
        .collect();
    for sort in [SortMode::Priority, SortMode::Newest] {
        let started = Instant::now();
        let seen = all_pages(
            &store,
            &TaskQuery {
                text: "report".into(),
                sort,
                ..TaskQuery::default()
            },
        );
        eprintln!(
            "{} 'report' matches over all pages ({sort:?}): {:?}",
            seen.len(),
            started.elapsed()
        );
        assert_complete(&seen, &expected);
    }

    // Every filter combined with search.
    let combined: HashSet<_> = (0..TASKS)
        .filter(|&n| {
            is_report(n)
                && active(n)
                && tagged(n)
                && status(n) == "in_progress"
                && priority(n) == "high"
        })
        .map(id)
        .collect();
    assert!(!combined.is_empty());
    let seen = all_pages(
        &store,
        &TaskQuery {
            text: "report".into(),
            status: Some(WorkStatus::InProgress),
            priority: Some(Priority::High),
            tag: Some("work".into()),
            ..TaskQuery::default()
        },
    );
    assert_complete(&seen, &combined);

    // The explicit Removed scope finds only Removed Tasks.
    let removed_expected: HashSet<_> = (0..TASKS)
        .filter(|&n| is_report(n) && removed(n))
        .map(id)
        .collect();
    let seen = all_pages(
        &store,
        &TaskQuery {
            text: "report".into(),
            scope: TaskScope::Removed,
            ..TaskQuery::default()
        },
    );
    assert!(!removed_expected.is_empty());
    assert_complete(&seen, &removed_expected);
}
