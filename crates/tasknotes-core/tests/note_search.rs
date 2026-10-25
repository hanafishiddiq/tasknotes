use std::path::Path;

use tasknotes_core::note_folders::{CreateNoteFolder, NoteFolder};
use tasknotes_core::notes::{
    CreateNote, Note, NotePage, NoteQuery, NoteScope, SetNotePinned, UpdateNote,
};
use tasknotes_core::tasks::{TaskError, TaskStore};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn folder(store: &TaskStore, name: &str, parent: Option<&NoteFolder>) -> NoteFolder {
    store
        .create_note_folder(CreateNoteFolder {
            name: name.into(),
            parent_id: parent.map(|p| p.id.clone()),
        })
        .unwrap()
}

fn note_in(store: &TaskStore, markdown: &str, folder: Option<&NoteFolder>) -> Note {
    store
        .create_note_in_folder(
            CreateNote {
                markdown: markdown.into(),
            },
            folder.map(|f| f.id.as_str()),
        )
        .unwrap()
}

fn text(value: &str) -> NoteQuery {
    NoteQuery {
        text: value.into(),
        ..NoteQuery::default()
    }
}

fn page(store: &TaskStore, query: &str) -> NotePage {
    store.query_notes(&text(query), 100, None).unwrap()
}

fn found(store: &TaskStore, query: &str) -> Vec<String> {
    page(store, query)
        .items
        .into_iter()
        .map(|item| item.title)
        .collect()
}

fn edit(store: &TaskStore, note: &Note, markdown: &str) -> Note {
    let current = store.get_note(&note.id).unwrap().unwrap();
    store
        .update_note(UpdateNote {
            id: note.id.clone(),
            markdown: markdown.into(),
            expected_revision: current.revision,
        })
        .unwrap()
}

#[test]
fn search_finds_titles_and_markdown_content_in_every_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("search.db"));
    let work = folder(&store, "Work", None);
    let deep = folder(&store, "Archive", Some(&folder(&store, "Old", Some(&work))));
    note_in(
        &store,
        "# Café opening\n\nCall the **landlord** on Monday",
        None,
    );
    note_in(
        &store,
        "Release planning\n\n- [ ] Draft the schema",
        Some(&work),
    );
    note_in(
        &store,
        "Holiday\n\nBook a [ferry](https://example.com/ferry)",
        Some(&deep),
    );
    note_in(&store, "Groceries", None);

    // Titles, word prefixes, accents and case are all forgiving.
    assert_eq!(found(&store, "cafe"), vec!["Café opening"]);
    assert_eq!(found(&store, "PLAN"), vec!["Release planning"]);
    // Body text, inside any folder however deep.
    assert_eq!(found(&store, "landlord"), vec!["Café opening"]);
    assert_eq!(found(&store, "schema"), vec!["Release planning"]);
    assert_eq!(found(&store, "ferry"), vec!["Holiday"]);
    // Every word has to match.
    assert_eq!(found(&store, "landlord monday"), vec!["Café opening"]);
    assert!(found(&store, "landlord schema").is_empty());

    // A hit carries a short plain-text snippet, not the Note.
    let hit = &page(&store, "landlord").items[0];
    let snippet = hit.snippet.as_deref().unwrap();
    assert!(
        snippet.contains("\u{2}landlord\u{3}"),
        "snippet {snippet:?}"
    );
    assert!(!snippet.contains("**"), "snippet {snippet:?}");
    // Text with no words in it matches nothing, rather than listing every Note.
    for nothing in ["!!!", " *** ", "#"] {
        let page = store.query_notes(&text(nothing), 100, None).unwrap();
        assert!(
            page.items.is_empty(),
            "{nothing:?} matched {:?}",
            page.items
        );
        assert!(page.next_cursor.is_none());
    }
    // Blank text is no search at all: every Note, without snippets.
    let listed = store.query_notes(&text("   "), 100, None).unwrap();
    assert_eq!(listed.items.len(), 4);
    assert!(listed.items.iter().all(|item| item.snippet.is_none()));
}

#[test]
fn search_reflects_edits_and_never_matches_empty_notes() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("edits.db");
    let store = store_at(&db);
    let note = note_in(&store, "Trip\n\nTake the train", None);
    let empty = note_in(&store, "", None);
    assert_eq!(empty.title, "New Note");

    edit(&store, &note, "Trip\n\nTake the ferry");
    assert!(found(&store, "train").is_empty());
    assert_eq!(found(&store, "ferry"), vec!["Trip"]);

    // The placeholder title of an empty Note is not its content.
    assert!(found(&store, "new").is_empty());
    // A Note emptied by an edit leaves the results.
    edit(&store, &note, "");
    assert!(found(&store, "ferry").is_empty());
    assert!(found(&store, "trip").is_empty());

    // The index survives reopening.
    edit(&store, &note, "Trip\n\nTake the bus");
    assert_eq!(found(&store_at(&db), "bus"), vec!["Trip"]);
}

#[test]
fn search_results_keep_the_list_order_in_bounded_pages() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("order.db"));
    let mut expected = Vec::new();
    for n in 0..9 {
        let note = note_in(&store, &format!("Plan {n}"), None);
        if n == 2 {
            store
                .set_note_pinned(SetNotePinned {
                    id: note.id.clone(),
                    pinned: true,
                })
                .unwrap();
        } else {
            expected.insert(0, note.title);
        }
    }
    note_in(&store, "Unrelated", None);
    expected.insert(0, "Plan 2".into());

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = store
            .query_notes(&text("plan"), 4, cursor.as_deref())
            .unwrap();
        assert!(page.items.len() <= 4);
        seen.extend(page.items.into_iter().map(|item| item.title));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, expected);

    // A cursor belongs to its search text.
    let first = store.query_notes(&text("plan"), 4, None).unwrap();
    assert!(matches!(
        store.query_notes(&text("plans"), 4, first.next_cursor.as_deref()),
        Err(TaskError::InvalidNoteCursor)
    ));
    let words: Vec<String> = (1..=17).map(|n| format!("word{n}")).collect();
    assert!(matches!(
        store.query_notes(&text(&words.join(" ")), 4, None),
        Err(TaskError::TooManySearchWords)
    ));
    // A search can also be narrowed to one folder.
    let work = folder(&store, "Work", None);
    note_in(&store, "Plan for work", Some(&work));
    let narrowed = store
        .query_notes(
            &NoteQuery {
                text: "plan".into(),
                folder: NoteScope::Folder(work.id.clone()),
            },
            10,
            None,
        )
        .unwrap();
    assert_eq!(narrowed.items.len(), 1);
}
