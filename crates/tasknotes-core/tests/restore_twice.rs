//! A row can leave the data at one restore and come back at the next. Nothing
//! live remembers it then, so what must survive has to come from the Audit
//! Entries, which every restore keeps. Synthetic data only.

use tasknotes_core::audit::Interface;
use tasknotes_core::bulk::{BulkAction, BulkRequestStatus};
use tasknotes_core::notes::{CreateNote, UpdateNote};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::{CreateTask, PatchTask, TaskStore};

struct World {
    _dir: tempfile::TempDir,
    desktop: TaskStore,
    ai: TaskStore,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasknotes.db");
    World {
        desktop: TaskStore::open(&path).unwrap(),
        ai: TaskStore::open_as(&path, Interface::Mcp).unwrap(),
        _dir: dir,
    }
}

impl World {
    fn backup(&self) -> String {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let id = self.desktop.create_backup().unwrap().backup.id;
        std::thread::sleep(std::time::Duration::from_millis(5));
        id
    }

    fn restore(&self, backup: &str) {
        self.desktop.restore_backup(backup).unwrap();
        // The AI's store was opened before: as a new process would, it starts over.
        self.ai.adopt_restored_data().unwrap();
    }

    fn task(&self) -> String {
        self.desktop
            .create_task(CreateTask {
                title: "Synthetic target".into(),
                description: String::new(),
            })
            .unwrap()
            .id
    }
}

#[test]
fn a_task_revision_is_not_used_again_after_the_task_left_and_returned() {
    let w = world();
    let without = w.backup();
    let id = w.task();
    let with = w.backup();
    let patch = |revision, title: &str| {
        w.desktop.patch_task(PatchTask {
            id: id.clone(),
            expected_revision: revision,
            title: Some(title.into()),
            description: None,
        })
    };
    let client_holds = patch(1, "Synthetic the client read this").unwrap().revision;

    w.restore(&without);
    assert!(w.desktop.get_task(&id).unwrap().is_none());
    w.restore(&with);

    let back = w.desktop.get_task(&id).unwrap().unwrap();
    assert!(back.revision > client_holds, "{}", back.revision);
    patch(back.revision, "Synthetic the persons edit").unwrap();
    let stale = w.ai.patch_task(PatchTask {
        id: id.clone(),
        expected_revision: client_holds,
        title: Some("Synthetic stale overwrite".into()),
        description: None,
    });
    assert_eq!(stale.unwrap_err().code(), "revision_conflict");
}

#[test]
fn a_note_revision_is_not_used_again_after_the_note_left_and_returned() {
    let w = world();
    let without = w.backup();
    let note = w
        .desktop
        .create_note(CreateNote {
            markdown: "# Synthetic first".into(),
            folder_id: None,
        })
        .unwrap();
    let with = w.backup();
    let client_holds = w
        .desktop
        .update_note(UpdateNote {
            id: note.id.clone(),
            expected_revision: note.revision,
            markdown: "# Synthetic the client read this".into(),
        })
        .unwrap()
        .revision;

    w.restore(&without);
    w.restore(&with);

    let back = w.desktop.get_note(&note.id).unwrap().unwrap();
    assert!(back.revision > client_holds, "{}", back.revision);
}

#[test]
fn an_unchanged_row_keeps_its_revision_through_restores() {
    let w = world();
    let id = w.task();
    let revision = w.desktop.get_task(&id).unwrap().unwrap().revision;
    let backup = w.backup();
    w.restore(&backup);
    w.restore(&backup);
    assert_eq!(w.desktop.get_task(&id).unwrap().unwrap().revision, revision);
}

#[test]
fn a_private_note_stays_private_after_it_left_and_returned() {
    let w = world();
    let without = w.backup();
    let note = w
        .desktop
        .create_note(CreateNote {
            markdown: "# Synthetic secret".into(),
            folder_id: None,
        })
        .unwrap();
    let with = w.backup();
    w.desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: note.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();

    w.restore(&without);
    w.restore(&with);

    assert!(w.ai.get_note(&note.id).unwrap().is_none());
    assert!(
        w.desktop
            .get_note(&note.id)
            .unwrap()
            .unwrap()
            .exclude_from_ai
    );
}

#[test]
fn a_decision_stands_after_the_request_left_and_returned() {
    for (approve, status) in [
        (true, BulkRequestStatus::Applied),
        (false, BulkRequestStatus::Declined),
    ] {
        let w = world();
        let without = w.backup();
        let target = w.task();
        let id =
            w.ai.request_bulk(BulkAction::RemoveTasks, vec![target])
                .unwrap()
                .request
                .id;
        let with = w.backup();
        if approve {
            w.desktop.approve_bulk_request(&id).unwrap();
        } else {
            w.desktop.decline_bulk_request(&id).unwrap();
        }

        w.restore(&without);
        w.restore(&with);

        assert_eq!(w.desktop.count_pending_bulk_requests().unwrap(), 0);
        let request = w.desktop.bulk_request(&id).unwrap();
        assert_eq!(request.status, status);
        assert!(request.decided_at.is_some());
    }
}

#[test]
fn a_request_nobody_decided_still_waits_after_it_left_and_returned() {
    let w = world();
    let without = w.backup();
    let target = w.task();
    w.ai.request_bulk(BulkAction::RemoveTasks, vec![target])
        .unwrap();
    let with = w.backup();
    w.restore(&without);
    w.restore(&with);
    assert_eq!(w.desktop.count_pending_bulk_requests().unwrap(), 1);
}

/// What a client claims is not a revision anyone issued. Reading after a
/// restore must still work, and nothing it named may move a row's number.
#[test]
fn a_refused_write_naming_any_revision_changes_nothing_through_a_restore() {
    for claimed in [i64::MAX, i64::MAX - 1, 1000] {
        let w = world();
        let id = w.task();
        let waiting =
            w.ai.request_bulk(BulkAction::RemoveTasks, vec![id.clone()])
                .unwrap()
                .request
                .id;
        let backup = w.backup();
        let refused = w.ai.patch_task(PatchTask {
            id: id.clone(),
            expected_revision: claimed,
            title: Some("Synthetic".into()),
            description: None,
        });
        assert_eq!(refused.unwrap_err().code(), "revision_conflict");

        w.restore(&backup);

        assert_eq!(w.desktop.get_task(&id).unwrap().unwrap().revision, 1);
        assert_eq!(w.desktop.list_tasks(10, None).unwrap().items.len(), 1);
        w.desktop.approve_bulk_request(&waiting).unwrap();
    }
}

/// A bump is written to the row only. When the row leaves, the number must not
/// be forgotten: `Sa` restored gives a new number, which a client reads; the row
/// leaves; `Sb` brings it back with an older number; an edit must not reach the
/// number the client holds.
#[test]
fn a_number_given_by_a_restore_is_not_used_again_for_other_content() {
    let w = world();
    let empty = w.backup();
    let id = w.task();
    let patch = |store: &TaskStore, revision, title: &str| {
        store.patch_task(PatchTask {
            id: id.clone(),
            expected_revision: revision,
            title: Some(title.into()),
            description: None,
        })
    };
    let a = patch(&w.desktop, 1, "Synthetic A").unwrap();
    let sa = w.backup();
    patch(&w.desktop, a.revision, "Synthetic B").unwrap();
    let sb = w.backup();

    w.restore(&sa);
    let client_holds = w.desktop.get_task(&id).unwrap().unwrap().revision;
    w.restore(&empty);
    w.restore(&sb);
    let back = w.desktop.get_task(&id).unwrap().unwrap();
    assert!(back.revision > client_holds, "{}", back.revision);
    patch(&w.desktop, back.revision, "Synthetic the persons edit").unwrap();

    assert_eq!(
        patch(&w.ai, client_holds, "Synthetic stale overwrite")
            .unwrap_err()
            .code(),
        "revision_conflict"
    );
}

/// An AI cannot decide a request, so nothing it tries may count as a decision.
#[test]
fn an_ai_trying_to_approve_does_not_count_as_a_decision() {
    let w = world();
    let without = w.backup();
    let target = w.task();
    let id =
        w.ai.request_bulk(BulkAction::RemoveTasks, vec![target])
            .unwrap()
            .request
            .id;
    let with = w.backup();
    assert_eq!(
        w.ai.approve_bulk_request(&id).unwrap_err().code(),
        "desktop_only"
    );

    w.restore(&without);
    w.restore(&with);

    assert_eq!(w.desktop.count_pending_bulk_requests().unwrap(), 1);
}
