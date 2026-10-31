//! Functional check that Note listing and search stay correct and page-bounded
//! on a large synthetic collection. Timing is printed, not asserted: the
//! measured release benchmark belongs to Ticket 17.

use std::collections::HashSet;
use std::time::Instant;

use rusqlite::{Connection, params};
use tasknotes_core::notes::{CreateNote, NoteQuery, NoteScope};
use tasknotes_core::tasks::TaskStore;

const NOTES: usize = 5_000;
const FOLDER: &str = "33333333-3333-4333-8333-333333333333";

fn id(n: usize) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}
fn in_folder(n: usize) -> bool {
    n.is_multiple_of(5)
}
fn is_report(n: usize) -> bool {
    n.is_multiple_of(2)
}
fn pinned(n: usize) -> bool {
    n.is_multiple_of(97)
}

fn seed(path: &std::path::Path) {
    drop(TaskStore::open(path).unwrap());
    let mut conn = Connection::open(path).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute(
        "INSERT INTO note_folders(id, parent_id, name, name_fold, revision, created_at, updated_at)
         VALUES (?1, NULL, 'Reports', 'reports', 1, 1, 1)",
        params![FOLDER],
    )
    .unwrap();
    for n in 0..NOTES {
        let title = format!("Note {n}");
        let body = if is_report(n) {
            "quarterly report figures"
        } else {
            "shopping list"
        };
        // Through the table, so the search index triggers run as for any writer.
        tx.execute(
            "INSERT INTO notes(id, markdown, title, revision, created_at, updated_at, folder_id, pinned)
             VALUES (?1, ?2, ?3, 1, ?4, ?4, ?5, ?6)",
            params![
                id(n),
                format!("# {title}\n\n{body}"),
                title,
                n as i64 + 1,
                in_folder(n).then_some(FOLDER),
                pinned(n)
            ],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}

/// Walks every page of a query and returns the ids in order.
fn walk(store: &TaskStore, query: &NoteQuery) -> Vec<String> {
    let mut ids = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = store.query_notes(query, 100, cursor.as_deref()).unwrap();
        assert!(page.items.len() <= 100);
        ids.extend(page.items.into_iter().map(|item| item.id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            return ids;
        }
    }
}

/// Pinned first, then newest first: the order the core promises.
fn expected(matches: impl Fn(usize) -> bool) -> Vec<String> {
    let mut ns: Vec<usize> = (0..NOTES).filter(|&n| matches(n)).collect();
    ns.sort_by_key(|&n| (!pinned(n), std::cmp::Reverse(n)));
    ns.into_iter().map(id).collect()
}

#[test]
fn large_note_collections_list_and_search_in_bounded_pages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large-notes.db");
    seed(&path);
    let store = TaskStore::open(&path).unwrap();

    let started = Instant::now();
    let first = store.query_notes(&NoteQuery::default(), 50, None).unwrap();
    println!("first page of {NOTES} Notes: {:?}", started.elapsed());
    assert_eq!(first.items.len(), 50);

    let cases = [
        (NoteQuery::default(), expected(|_| true)),
        (
            NoteQuery {
                folder: NoteScope::Folder(FOLDER.into()),
                ..NoteQuery::default()
            },
            expected(in_folder),
        ),
        (
            NoteQuery {
                text: "report".into(),
                ..NoteQuery::default()
            },
            expected(is_report),
        ),
    ];
    for (query, want) in cases {
        let started = Instant::now();
        let got = walk(&store, &query);
        println!(
            "{} Notes for {:?} in {:?}",
            got.len(),
            query.text,
            started.elapsed()
        );
        assert_eq!(got.iter().collect::<HashSet<_>>().len(), got.len());
        assert_eq!(got, want);
    }

    let started = Instant::now();
    let search = store
        .query_notes(
            &NoteQuery {
                text: "quarterly figures".into(),
                ..NoteQuery::default()
            },
            50,
            None,
        )
        .unwrap();
    println!("first search page: {:?}", started.elapsed());
    assert_eq!(search.items.len(), 50);
    assert!(search.items.iter().all(|item| item.snippet.is_some()));
}

/// Snippets are cut only for the Notes on the page, not for every match:
/// with long bodies, cutting one per match made the first page take seconds.
#[test]
fn search_snippets_cost_the_page_not_every_match() {
    const LONG_NOTES: usize = 150;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long-notes.db");
    drop(TaskStore::open(&path).unwrap());
    // About 100 KB per Note, the matched word near the end of each.
    let body = format!(
        "{} lease renewal",
        "filler words for a long body ".repeat(3_500)
    );
    let mut conn = Connection::open(&path).unwrap();
    let tx = conn.transaction().unwrap();
    for n in 0..LONG_NOTES {
        tx.execute(
            "INSERT INTO notes(id, markdown, title, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?4)",
            params![
                id(n),
                format!("# Long {n}\n\n{body}"),
                format!("Long {n}"),
                n as i64 + 1
            ],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let store = TaskStore::open(&path).unwrap();
    let query = NoteQuery {
        text: "lease".into(),
        ..NoteQuery::default()
    };

    // Best of three runs, so a busy machine does not decide the result.
    let timed = |limit: u32| {
        (0..3)
            .map(|_| {
                let started = Instant::now();
                let page = store.query_notes(&query, limit, None).unwrap();
                (started.elapsed(), page)
            })
            .min_by_key(|(elapsed, _)| *elapsed)
            .unwrap()
    };
    let (page_of_one, one) = timed(1);
    let (page_of_hundred, hundred) = timed(100);
    println!("1 hit: {page_of_one:?}, 100 hits: {page_of_hundred:?} of {LONG_NOTES} matches");
    assert_eq!(one.items.len(), 1);
    assert_eq!(hundred.items.len(), 100);
    assert!(
        hundred.items[99]
            .snippet
            .as_deref()
            .unwrap()
            .contains("\u{2}lease\u{3}")
    );
    // When a snippet is cut for every match, a page of one costs as much as a
    // page of a hundred; cut for the page only, it is many times cheaper.
    assert!(
        page_of_one * 3 < page_of_hundred,
        "a 1-Note page took {page_of_one:?}, a 100-Note page {page_of_hundred:?}"
    );
}

/// A short prefix expands to thousands of words. Looking each Note on the page
/// up through the search index repeated that expansion once per Note, so one
/// typed letter cost seconds at 10,000 Notes. A page of 50 must cost about what
/// a page of one costs. A ratio holds in debug and release builds alike.
#[test]
fn a_page_of_snippets_repeats_no_search_for_a_short_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("tasknotes.db")).unwrap();
    for note in 0..400 {
        let words: Vec<String> = (0..100).map(|word| format!("b{note}x{word}")).collect();
        store
            .create_note(CreateNote {
                markdown: format!("# Synthetic {note}\n\n{}", words.join(" ")),
                folder_id: None,
            })
            .unwrap();
    }
    let query = NoteQuery {
        text: "b".into(),
        ..NoteQuery::default()
    };
    let fastest = |limit: u32| {
        (0..5)
            .map(|_| {
                let started = Instant::now();
                let page = store.query_notes(&query, limit, None).unwrap();
                assert_eq!(page.items.len(), limit as usize);
                assert!(page.items.iter().all(|n| n.snippet.is_some()));
                started.elapsed()
            })
            .min()
            .unwrap()
    };
    let one = fastest(1);
    let fifty = fastest(50);
    assert!(
        fifty < one * 8,
        "50 Notes took {fifty:?}, one took {one:?}: the search repeats per Note"
    );
}
