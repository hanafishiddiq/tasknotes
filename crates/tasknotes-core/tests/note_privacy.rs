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

fn ids(page: &tasknotes_core::notes::NotePage) -> Vec<String> {
    page.items.iter().map(|n| n.id.clone()).collect()
}

#[test]
fn ai_listings_and_searches_leave_an_excluded_note_out_entirely() {
    use tasknotes_core::note_folders::CreateNoteFolder;
    use tasknotes_core::notes::{NoteQuery, NoteScope};
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let folder = desktop
        .create_note_folder(CreateNoteFolder {
            name: "Synthetic folder".into(),
            parent_id: None,
        })
        .unwrap();
    let in_folder = |markdown: &str| {
        desktop
            .create_note(CreateNote {
                markdown: markdown.into(),
                folder_id: Some(folder.id.clone()),
            })
            .unwrap()
    };
    let shared = in_folder("# Shared marmalade recipe");
    let private = in_folder("# Private marmalade recipe");
    exclude(&desktop, &private, true).unwrap();

    let searches = [
        NoteQuery::default(),
        NoteQuery {
            text: "marmalade".into(),
            folder: NoteScope::All,
        },
        NoteQuery {
            text: "private".into(),
            folder: NoteScope::All,
        },
        NoteQuery {
            text: String::new(),
            folder: NoteScope::Folder(folder.id.clone()),
        },
    ];
    let ai = fx.ai();
    for query in &searches {
        let page = ai.query_notes(query, 50, None).unwrap();
        assert!(!ids(&page).contains(&private.id), "{query:?}");
        assert!(page.items.iter().all(|n| !n.title.contains("Private")
            && !n.snippet.clone().unwrap_or_default().contains("Private")));
    }
    assert_eq!(
        ids(&ai.list_notes(50, None).unwrap()),
        vec![shared.id.clone()]
    );
    let desktop_all = desktop.list_notes(50, None).unwrap();
    assert_eq!(ids(&desktop_all).len(), 2, "the person still finds both");
    let desktop_hit = desktop.query_notes(&searches[2], 50, None).unwrap();
    assert_eq!(ids(&desktop_hit), vec![private.id.clone()]);
}

fn task(store: &TaskStore, title: &str) -> tasknotes_core::tasks::Task {
    store
        .create_task(tasknotes_core::tasks::CreateTask {
            title: title.into(),
            description: String::new(),
        })
        .unwrap()
}

#[test]
fn ai_link_lists_leave_out_an_excluded_note_and_its_links() {
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let chore = task(&desktop, "Synthetic chore");
    let shared = note(&desktop, "# Shared checklist");
    let private = note(&desktop, "# Private checklist");
    desktop.link_note(&chore.id, &shared.id).unwrap();
    let private_link = desktop.link_note(&chore.id, &private.id).unwrap();
    exclude(&desktop, &private, true).unwrap();

    let ai = fx.ai();
    let linked = ai.list_task_notes(&chore.id, 50, None).unwrap();
    assert_eq!(
        linked.items.iter().map(|n| &n.note_id).collect::<Vec<_>>(),
        vec![&shared.id]
    );
    assert_eq!(ai.get_link(&private_link.id).unwrap(), None);
    let never = "00000000-0000-4000-8000-000000000000";
    for id in [private.id.as_str(), never] {
        assert_eq!(
            ai.list_note_tasks(id, 50, None).unwrap_err().code(),
            "not_found",
            "an excluded Note's Tasks read like a missing Note's"
        );
    }
    assert_eq!(
        desktop
            .list_task_notes(&chore.id, 50, None)
            .unwrap()
            .items
            .len(),
        2
    );
    assert_eq!(
        desktop
            .list_note_tasks(&private.id, 50, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(desktop.get_link(&private_link.id).unwrap().is_some());
}

/// Every AI write that names a Note, run against `id`; each error message
/// and code is returned so the caller can compare an excluded Note with an
/// id that was never used.
fn ai_writes(
    ai: &TaskStore,
    task_id: &str,
    id: &str,
    link_id: &str,
    revision: i64,
) -> Vec<(String, String)> {
    use tasknotes_core::notes::{MoveNote, SetNotePinned, UpdateNote};
    use tasknotes_core::removed::SetRemoved;
    let results = vec![
        ai.update_note(UpdateNote {
            id: id.into(),
            markdown: "# Overwritten by AI".into(),
            expected_revision: revision,
        })
        .map(|_| ()),
        ai.update_note(UpdateNote {
            id: id.into(),
            markdown: "# Stale overwrite".into(),
            expected_revision: revision + 7,
        })
        .map(|_| ()),
        ai.move_note(MoveNote {
            id: id.into(),
            folder_id: None,
            expected_revision: Some(revision),
        })
        .map(|_| ()),
        ai.set_note_pinned(SetNotePinned {
            id: id.into(),
            pinned: true,
        })
        .map(|_| ()),
        ai.set_note_removed(SetRemoved {
            id: id.into(),
            expected_revision: revision,
            removed: true,
        })
        .map(|_| ()),
        ai.set_note_removed(SetRemoved {
            id: id.into(),
            expected_revision: revision,
            removed: false,
        })
        .map(|_| ()),
        ai.link_note(task_id, id).map(|_| ()),
        ai.set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: id.into(),
            exclude_from_ai: false,
        })
        .map(|_| ()),
        ai.unlink_note(link_id).map(|_| ()),
        ai.restore_link(link_id).map(|_| ()),
    ];
    results
        .into_iter()
        .map(|r| {
            let e = r.expect_err("an AI write on a hidden Note must fail");
            (e.code().to_owned(), e.to_string())
        })
        .collect()
}

#[test]
fn ai_writes_restores_and_links_on_an_excluded_note_fail_like_a_missing_note() {
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let chore = task(&desktop, "Synthetic chore");
    let private = note(&desktop, "# Private synthetic plan\n\nsecret-marker-123");
    let link = desktop.link_note(&chore.id, &private.id).unwrap();
    let private = exclude(&desktop, &private, true).unwrap();
    let before = desktop.get_note(&private.id).unwrap().unwrap();

    let ai = fx.ai();
    let hidden = ai_writes(&ai, &chore.id, &private.id, &link.id, private.revision);
    let never = ai_writes(
        &ai,
        &chore.id,
        "00000000-0000-4000-8000-000000000000",
        "00000000-0000-4000-8000-000000000001",
        private.revision,
    );
    assert_eq!(
        hidden, never,
        "indistinguishable from a Note that never existed"
    );
    for (code, message) in &hidden {
        assert_eq!(code, "not_found");
        assert!(!message.contains("secret") && !message.contains("Private"));
    }
    assert_eq!(desktop.get_note(&private.id).unwrap().unwrap(), before);
    assert!(
        desktop
            .get_link(&link.id)
            .unwrap()
            .unwrap()
            .unlinked_at
            .is_none()
    );

    // Removed as well as excluded: restoring it is still "not found".
    let removed = desktop
        .set_note_removed(tasknotes_core::removed::SetRemoved {
            id: private.id.clone(),
            expected_revision: before.revision,
            removed: true,
        })
        .unwrap();
    let hidden = ai_writes(&ai, &chore.id, &private.id, &link.id, removed.revision);
    assert!(
        hidden.iter().all(|(code, _)| code == "not_found"),
        "{hidden:?}"
    );
}

#[test]
fn only_the_desktop_changes_the_flag_and_the_desktop_still_edits_an_excluded_note() {
    use tasknotes_core::notes::UpdateNote;
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let shared = note(&desktop, "# Shared synthetic note");
    let refused = exclude(&fx.ai(), &shared, true).unwrap_err();
    assert_eq!(refused.code(), "desktop_only");
    assert!(
        !desktop
            .get_note(&shared.id)
            .unwrap()
            .unwrap()
            .exclude_from_ai
    );

    let private = exclude(&desktop, &shared, true).unwrap();
    let edited = desktop
        .update_note(UpdateNote {
            id: private.id.clone(),
            markdown: "# Still mine to edit".into(),
            expected_revision: private.revision,
        })
        .unwrap();
    assert!(edited.exclude_from_ai, "editing keeps the Note private");
    assert_eq!(fx.ai().get_note(&private.id).unwrap(), None);
}

#[test]
fn the_ai_removed_view_leaves_out_an_excluded_note() {
    use tasknotes_core::removed::SetRemoved;
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let remove = |n: &Note| {
        desktop
            .set_note_removed(SetRemoved {
                id: n.id.clone(),
                expected_revision: n.revision,
                removed: true,
            })
            .unwrap()
    };
    let shared = note(&desktop, "# Shared old draft");
    let private = exclude(&desktop, &note(&desktop, "# Private old draft"), true).unwrap();
    remove(&shared);
    remove(&private);

    let ai = fx.ai();
    for text in ["", "draft", "private"] {
        let page = ai.list_removed(text, 50, None).unwrap();
        assert!(
            page.items.iter().all(|item| item.id != private.id),
            "{text:?}"
        );
    }
    assert_eq!(ai.list_removed("", 50, None).unwrap().items.len(), 1);
    assert_eq!(desktop.list_removed("", 50, None).unwrap().items.len(), 2);
}

#[test]
fn folder_paths_read_root_first_and_ai_counts_only_notes_it_may_see() {
    use tasknotes_core::note_folders::CreateNoteFolder;
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let folder = |name: &str, parent: Option<&str>| {
        desktop
            .create_note_folder(CreateNoteFolder {
                name: name.into(),
                parent_id: parent.map(str::to_owned),
            })
            .unwrap()
    };
    let work = folder("Work", None);
    let client = folder("Client A", Some(&work.id));
    let put = |markdown: &str, folder_id: &str| {
        desktop
            .create_note(CreateNote {
                markdown: markdown.into(),
                folder_id: Some(folder_id.into()),
            })
            .unwrap()
    };
    put("# Shared brief", &client.id);
    let private = put("# Private brief", &client.id);
    exclude(&desktop, &private, true).unwrap();
    let only_private = put("# Private note alone", &work.id);
    exclude(&desktop, &only_private, true).unwrap();

    let ai = fx.ai();
    assert_eq!(ai.note_folder_path(&client.id).unwrap(), "Work/Client A");
    assert_eq!(ai.note_folder_path(&work.id).unwrap(), "Work");
    assert_eq!(ai.count_folder_notes(Some(&client.id)).unwrap(), 1);
    assert_eq!(ai.count_folder_notes(Some(&work.id)).unwrap(), 0);
    assert_eq!(desktop.count_folder_notes(Some(&client.id)).unwrap(), 2);
    assert_eq!(desktop.count_folder_notes(Some(&work.id)).unwrap(), 1);
    assert_eq!(
        ai.note_folder_path("00000000-0000-4000-8000-000000000000")
            .unwrap_err()
            .code(),
        "not_found"
    );
}

fn actions(store: &TaskStore, entity: Option<&str>) -> Vec<(String, String, String)> {
    let mut items = store.list_audit_entries(entity, 100, None).unwrap().items;
    items.reverse();
    items
        .into_iter()
        .map(|e| {
            (
                e.action.as_str().to_owned(),
                e.interface.as_str().to_owned(),
                e.outcome.as_str().to_owned(),
            )
        })
        .collect()
}

fn triple(action: &str, interface: &str, outcome: &str) -> (String, String, String) {
    (action.into(), interface.into(), outcome.into())
}

#[test]
fn note_folder_and_link_changes_are_audited_for_every_interface() {
    use tasknotes_core::note_folders::{CreateNoteFolder, MoveNoteFolder, RenameNoteFolder};
    use tasknotes_core::notes::{MoveNote, SetNotePinned, UpdateNote};
    use tasknotes_core::removed::SetRemoved;
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let ai = fx.ai();
    let chore = task(&desktop, "Synthetic chore");
    let start = desktop
        .list_audit_entries(None, 100, None)
        .unwrap()
        .items
        .len();

    let folder = ai
        .create_note_folder(CreateNoteFolder {
            name: "Inbox notes".into(),
            parent_id: None,
        })
        .unwrap();
    let folder = ai
        .rename_note_folder(RenameNoteFolder {
            id: folder.id.clone(),
            expected_revision: folder.revision,
            name: "Reading".into(),
        })
        .unwrap();
    let parent = desktop
        .create_note_folder(CreateNoteFolder {
            name: "Home".into(),
            parent_id: None,
        })
        .unwrap();
    let folder = ai
        .move_note_folder(MoveNoteFolder {
            id: folder.id.clone(),
            expected_revision: folder.revision,
            parent_id: Some(parent.id.clone()),
        })
        .unwrap();
    let removed = ai
        .set_note_folder_removed(SetRemoved {
            id: folder.id.clone(),
            expected_revision: folder.revision,
            removed: true,
        })
        .unwrap();
    ai.set_note_folder_removed(SetRemoved {
        id: folder.id.clone(),
        expected_revision: removed.revision,
        removed: false,
    })
    .unwrap();

    let n = ai
        .create_note(CreateNote {
            markdown: "# Synthetic reading list".into(),
            folder_id: None,
        })
        .unwrap();
    let n = ai
        .update_note(UpdateNote {
            id: n.id.clone(),
            markdown: "# Synthetic reading list, longer".into(),
            expected_revision: n.revision,
        })
        .unwrap();
    // A stale write is refused and recorded, like a Task's.
    let stale = ai.update_note(UpdateNote {
        id: n.id.clone(),
        markdown: "# Stale".into(),
        expected_revision: n.revision - 1,
    });
    assert_eq!(stale.unwrap_err().code(), "revision_conflict");
    ai.move_note(MoveNote {
        id: n.id.clone(),
        folder_id: Some(folder.id.clone()),
        expected_revision: Some(n.revision),
    })
    .unwrap();
    desktop
        .set_note_pinned(SetNotePinned {
            id: n.id.clone(),
            pinned: true,
        })
        .unwrap();
    let link = ai.link_note(&chore.id, &n.id).unwrap();
    ai.unlink_note(&link.id).unwrap();
    ai.restore_link(&link.id).unwrap();
    let gone = ai
        .set_note_removed(SetRemoved {
            id: n.id.clone(),
            expected_revision: n.revision,
            removed: true,
        })
        .unwrap();
    ai.set_note_removed(SetRemoved {
        id: n.id.clone(),
        expected_revision: gone.revision,
        removed: false,
    })
    .unwrap();
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: n.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();

    let all = actions(&desktop, None);
    assert_eq!(
        all[start..].to_vec(),
        vec![
            triple("folder.create", "cli", "applied"),
            triple("folder.rename", "cli", "applied"),
            triple("folder.create", "desktop", "applied"),
            triple("folder.move", "cli", "applied"),
            triple("folder.remove", "cli", "applied"),
            triple("folder.restore", "cli", "applied"),
            triple("note.create", "cli", "applied"),
            triple("note.update", "cli", "applied"),
            triple("note.update", "cli", "rejected"),
            triple("note.move", "cli", "applied"),
            triple("note.pin", "desktop", "applied"),
            triple("link.create", "cli", "applied"),
            triple("link.remove", "cli", "applied"),
            triple("link.restore", "cli", "applied"),
            triple("note.remove", "cli", "applied"),
            triple("note.restore", "cli", "applied"),
            triple("note.privacy", "desktop", "applied"),
        ]
    );

    // The Note is now excluded: to AI its entries, and its link's, are gone,
    // as if it had never existed.
    let ai_entries = ai.list_audit_entries(None, 100, None).unwrap().items;
    assert!(
        ai_entries
            .iter()
            .all(|e| e.entity_id != n.id && e.entity_id != link.id)
    );
    assert_eq!(ai_entries.len(), all.len() - 11);
    assert!(
        ai.list_audit_entries(Some(&n.id), 100, None)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        ai.list_audit_entries(Some(&link.id), 100, None)
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(actions(&desktop, Some(&n.id)).len(), 8);
}
