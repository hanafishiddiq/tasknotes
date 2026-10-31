//! Query timing against a seeded dataset, the same calls the desktop makes:
//! one store, a fresh connection per operation, pages of 50.

use std::time::Instant;

use serde_json::{Value, json};
use tasknotes_core::notes::{NoteQuery, NoteScope};
use tasknotes_core::tasks::{
    Priority, SortMode, TaskQuery, TaskScope, TaskStore, TaskView, WorkStatus,
};

use crate::dataset::{NOTE_COMMON, NOTE_RARE, PROGRESS_WORD, TASK_COMMON, TASK_MEDIUM, TASK_RARE};

pub const PAGE: u32 = 50;

pub enum Query {
    Tasks(TaskQuery),
    /// The last of `pages` pages, reached by walking the cursor once.
    TasksDeep(TaskQuery, usize),
    Notes(NoteQuery),
    Audit,
}

pub struct Scenario {
    pub name: &'static str,
    pub search: bool,
    pub query: Query,
}

fn text(text: &str) -> TaskQuery {
    TaskQuery {
        text: text.to_owned(),
        ..TaskQuery::default()
    }
}

fn note_text(text: &str) -> NoteQuery {
    NoteQuery {
        text: text.to_owned(),
        ..NoteQuery::default()
    }
}

pub fn scenarios(manifest: &Value) -> Vec<Scenario> {
    let list = manifest["list"].as_str().unwrap_or_default().to_owned();
    let folder = manifest["folder"].as_str().unwrap_or_default().to_owned();
    let tag = manifest["tag"].as_str().unwrap_or_default().to_owned();
    let s = |name, search, query| Scenario {
        name,
        search,
        query,
    };
    vec![
        s(
            "tasks: first page, priority order",
            false,
            Query::Tasks(TaskQuery::default()),
        ),
        s(
            "tasks: first page, newest order",
            false,
            Query::Tasks(TaskQuery {
                sort: SortMode::Newest,
                ..TaskQuery::default()
            }),
        ),
        s(
            "tasks: one List",
            false,
            Query::Tasks(TaskQuery {
                view: TaskView::List(list.clone()),
                ..TaskQuery::default()
            }),
        ),
        s(
            "tasks: Inbox",
            false,
            Query::Tasks(TaskQuery {
                view: TaskView::Inbox,
                ..TaskQuery::default()
            }),
        ),
        s(
            "tasks: Removed",
            false,
            Query::Tasks(TaskQuery {
                scope: TaskScope::Removed,
                ..TaskQuery::default()
            }),
        ),
        s(
            "tasks: walking 100 pages, total",
            false,
            Query::TasksDeep(TaskQuery::default(), 100),
        ),
        s(
            "tasks: filter by status and priority",
            false,
            Query::Tasks(TaskQuery {
                status: Some(WorkStatus::InProgress),
                priority: Some(Priority::High),
                ..TaskQuery::default()
            }),
        ),
        s(
            "tasks: filter by tag",
            false,
            Query::Tasks(TaskQuery {
                tag: Some(tag.clone()),
                ..TaskQuery::default()
            }),
        ),
        s(
            "search tasks: one letter (typing starts)",
            true,
            Query::Tasks(text("b")),
        ),
        s("search tasks: two letters", true, Query::Tasks(text("ba"))),
        s(
            "search tasks: common word (20%)",
            true,
            Query::Tasks(text(TASK_COMMON)),
        ),
        s(
            "search tasks: common prefix",
            true,
            Query::Tasks(text(&TASK_COMMON[..3])),
        ),
        s(
            "search tasks: medium word (5%)",
            true,
            Query::Tasks(text(TASK_MEDIUM)),
        ),
        s(
            "search tasks: rare word (0.1%)",
            true,
            Query::Tasks(text(TASK_RARE)),
        ),
        s(
            "search tasks: Progress Entry word",
            true,
            Query::Tasks(text(PROGRESS_WORD)),
        ),
        s(
            "search tasks: two common words",
            true,
            Query::Tasks(text(&format!("{TASK_COMMON} {TASK_MEDIUM}"))),
        ),
        s(
            "search tasks: no match",
            true,
            Query::Tasks(text("nomatchanywhere")),
        ),
        s(
            "search tasks: word in one List",
            true,
            Query::Tasks(TaskQuery {
                view: TaskView::List(list),
                ..text(TASK_COMMON)
            }),
        ),
        s(
            "search tasks: word and status",
            true,
            Query::Tasks(TaskQuery {
                status: Some(WorkStatus::Ready),
                ..text(TASK_COMMON)
            }),
        ),
        s(
            "search tasks: word and tag",
            true,
            Query::Tasks(TaskQuery {
                tag: Some(tag),
                ..text(TASK_COMMON)
            }),
        ),
        s(
            "search tasks: word, newest order",
            true,
            Query::Tasks(TaskQuery {
                sort: SortMode::Newest,
                ..text(TASK_COMMON)
            }),
        ),
        s(
            "notes: first page",
            false,
            Query::Notes(NoteQuery::default()),
        ),
        s(
            "notes: one folder",
            false,
            Query::Notes(NoteQuery {
                folder: NoteScope::Folder(folder),
                ..NoteQuery::default()
            }),
        ),
        s(
            "search notes: one letter (typing starts)",
            true,
            Query::Notes(note_text("b")),
        ),
        s(
            "search notes: common word (30%)",
            true,
            Query::Notes(note_text(NOTE_COMMON)),
        ),
        s(
            "search notes: rare word (0.2%)",
            true,
            Query::Notes(note_text(NOTE_RARE)),
        ),
        s(
            "search notes: no match",
            true,
            Query::Notes(note_text("nomatchanywhere")),
        ),
        s("audit: first page", false, Query::Audit),
    ]
}

/// Run a query once; how many records came back. A refusal stops the run,
/// because a timing of an error would say nothing.
pub fn run(store: &TaskStore, query: &Query) -> usize {
    match query {
        Query::Tasks(q) => store.query_tasks(q, PAGE, None).expect("query").items.len(),
        Query::TasksDeep(q, pages) => {
            let mut cursor = None;
            let mut last = 0;
            for _ in 0..*pages {
                let page = store
                    .query_tasks(q, PAGE, cursor.as_deref())
                    .expect("query");
                last = page.items.len();
                cursor = page.next_cursor;
            }
            last
        }
        Query::Notes(q) => store.query_notes(q, PAGE, None).expect("query").items.len(),
        Query::Audit => store
            .list_audit_entries(None, PAGE, None)
            .expect("audit")
            .items
            .len(),
    }
}

/// Nearest-rank percentile of milliseconds.
pub fn percentile(sorted: &[f64], percent: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    sorted[rank - 1]
}

pub fn summarize(mut samples: Vec<f64>) -> Value {
    samples.sort_by(f64::total_cmp);
    let round = |value: f64| (value * 100.0).round() / 100.0;
    json!({
        "runs": samples.len(),
        "p50_ms": round(percentile(&samples, 50)),
        "p95_ms": round(percentile(&samples, 95)),
        "p99_ms": round(percentile(&samples, 99)),
        "max_ms": round(samples.last().copied().unwrap_or(0.0)),
    })
}

/// Warm timings: the first run of each query is thrown away, then `runs`.
pub fn warm(store: &TaskStore, scenario: &Scenario, runs: usize) -> (Vec<f64>, usize) {
    let found = run(store, &scenario.query);
    let samples = (0..runs)
        .map(|_| {
            let started = Instant::now();
            run(store, &scenario.query);
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    (samples, found)
}
