//! Ticket 11: Exclude from AI (ADR-0004). The core enforces the policy for
//! every AI-facing interface (CLI, MCP); the desktop is the person themself.
//! Synthetic data only.

use std::path::PathBuf;

use tasknotes_core::audit::Interface;
use tasknotes_core::notes::{CreateNote, Note};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::{TaskError, TaskStore};

struct Fixture {
    _dir: tempfile::TempDir,
    db: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("privacy.db");
        Self { _dir: dir, db }
    }

    fn desktop(&self) -> TaskStore {
        TaskStore::open(&self.db).unwrap()
    }

    fn ai(&self) -> TaskStore {
        TaskStore::open_as(&self.db, Interface::Cli).unwrap()
    }
}

fn note(store: &TaskStore, markdown: &str) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: None,
        })
        .unwrap()
}

fn exclude(store: &TaskStore, note: &Note, exclude: bool) -> Result<Note, TaskError> {
    store.set_note_exclude_from_ai(SetNoteExcludeFromAi {
        id: note.id.clone(),
        exclude_from_ai: exclude,
    })
}

#[test]
fn an_excluded_note_reads_as_missing_to_ai_but_stays_open_on_the_desktop() {
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let private = note(&desktop, "# Synthetic diary\n\nNothing real here");
    let marked = exclude(&desktop, &private, true).unwrap();
    assert!(marked.exclude_from_ai);
    // A setting, like a pin: the text and revision stay, so an open editor
    // keeps saving without a conflict.
    assert_eq!(marked.revision, private.revision);
    assert_eq!(marked.markdown, private.markdown);

    assert_eq!(
        desktop.get_note(&private.id).unwrap().unwrap(),
        marked,
        "the person still sees their Note"
    );
    assert_eq!(fx.ai().get_note(&private.id).unwrap(), None);

    let shared = exclude(&desktop, &marked, false).unwrap();
    assert!(!shared.exclude_from_ai);
    assert_eq!(fx.ai().get_note(&private.id).unwrap(), Some(shared));
}

#[test]
fn a_v10_database_keeps_its_notes_visible_to_ai_after_gaining_the_flag() {
    let fx = Fixture::new();
    let kept = note(&fx.desktop(), "# Existing synthetic Note");
    let conn = rusqlite::Connection::open(&fx.db).unwrap();
    conn.execute_batch("ALTER TABLE notes DROP COLUMN exclude_from_ai; PRAGMA user_version = 10;")
        .unwrap();
    drop(conn);

    let ai = fx.ai();
    let version: i64 = rusqlite::Connection::open(&fx.db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, tasknotes_core::tasks::SCHEMA_VERSION);
    let read = ai.get_note(&kept.id).unwrap().unwrap();
    assert!(!read.exclude_from_ai, "nothing becomes private by itself");
    assert_eq!(read.markdown, kept.markdown);
}
