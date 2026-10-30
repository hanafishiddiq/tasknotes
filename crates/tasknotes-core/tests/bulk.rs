//! Ticket 12: Bulk Changes. An AI interface (CLI, MCP) can only *request* a
//! bulk change; it waits, persisted, until the person approves or declines it
//! in the desktop app (owner decision A, 2026-10-26). Removing a List or Note
//! Folder that holds anything goes through the same approval (decision B).
//! Synthetic data only.

use tasknotes_core::audit::{AuditEntry, Interface};
use tasknotes_core::bulk::{
    ApprovalKind, BULK_MAX_ITEMS, BULK_REQUEST_TTL_MS, BulkAction, BulkItem, BulkRequestStatus,
};
use tasknotes_core::note_folders::CreateNoteFolder;
use tasknotes_core::notes::{CreateNote, Note};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, PatchTask, RenameList, SetListRemoved, Task,
    TaskError, TaskStore, WorkStatus,
};

struct World {
    _dir: tempfile::TempDir,
    desktop: TaskStore,
    ai: TaskStore,
    cli: TaskStore,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasknotes.db");
    World {
        desktop: TaskStore::open(&path).unwrap(),
        ai: TaskStore::open_as(&path, Interface::Mcp).unwrap(),
        cli: TaskStore::open_as(&path, Interface::Cli).unwrap(),
        _dir: dir,
    }
}

fn tasks(store: &TaskStore, n: usize) -> Vec<Task> {
    (0..n)
        .map(|i| {
            store
                .create_task(CreateTask {
                    title: format!("Synthetic bulk task {i}"),
                    description: String::new(),
                })
                .unwrap()
        })
        .collect()
}

fn ids(tasks: &[Task]) -> Vec<String> {
    tasks.iter().map(|t| t.id.clone()).collect()
}

fn note(store: &TaskStore, markdown: &str, folder_id: Option<&str>) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: folder_id.map(str::to_owned),
        })
        .unwrap()
}

fn exclude(desktop: &TaskStore, id: &str) {
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: id.to_owned(),
            exclude_from_ai: true,
        })
        .unwrap();
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn audit(desktop: &TaskStore) -> Vec<AuditEntry> {
    desktop.list_audit_entries(None, 100, None).unwrap().items
}

fn task_now(w: &World, id: &str) -> Task {
    w.desktop.get_task(id).unwrap().unwrap()
}

#[test]
fn a_request_shows_the_change_waits_for_approval_and_changes_nothing() {
    let w = world();
    let all = tasks(&w.ai, 3);
    let requested =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap();
    assert_eq!(requested.items.len(), 3);
    for (item, task) in requested.items.iter().zip(&all) {
        let BulkItem::Task { before, after } = item else {
            panic!("a Task item")
        };
        assert_eq!(before, task);
        assert!(after.removed);
    }
    let request = &requested.request;
    assert_eq!(request.status, BulkRequestStatus::Pending);
    assert_eq!(request.count, Some(3));
    assert_eq!(request.requested_by, Interface::Mcp);
    assert_eq!(request.expires_at - request.created_at, BULK_REQUEST_TTL_MS);
    for task in &all {
        assert_eq!(task_now(&w, &task.id), *task, "nothing applied");
    }
    // Another process (the desktop, the CLI) sees the same request.
    assert_eq!(w.cli.bulk_request(&request.id).unwrap(), *request);
    let pending = w.desktop.pending_bulk_requests().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request, *request);
    assert!(!pending[0].stale);
    let titles: Vec<&str> = pending[0].items.iter().map(|l| l.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Synthetic bulk task 0",
            "Synthetic bulk task 1",
            "Synthetic bulk task 2"
        ]
    );
    assert_eq!(w.desktop.count_pending_bulk_requests().unwrap(), 1);
}

#[test]
fn no_ai_interface_can_approve_or_decline_a_request() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let id =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request
            .id;
    for store in [&w.ai, &w.cli] {
        assert!(matches!(
            store.approve_bulk_request(&id),
            Err(TaskError::DesktopOnly)
        ));
        assert!(matches!(
            store.decline_bulk_request(&id),
            Err(TaskError::DesktopOnly)
        ));
        assert!(matches!(
            store.pending_bulk_requests(),
            Err(TaskError::DesktopOnly)
        ));
    }
    assert!(!task_now(&w, &all[0].id).removed);
    assert_eq!(
        w.ai.bulk_request(&id).unwrap().status,
        BulkRequestStatus::Pending
    );
}

#[test]
fn approving_in_the_desktop_applies_every_item_and_audits_both_sides() {
    let w = world();
    let all = tasks(&w.ai, 3);
    let target = list(&w.ai, "Synthetic target");
    let id =
        w.ai.request_bulk(
            BulkAction::MoveTasks {
                list_id: Some(target.id.clone()),
            },
            ids(&all),
        )
        .unwrap()
        .request
        .id;
    let pending = w.desktop.pending_bulk_requests().unwrap();
    assert_eq!(pending[0].target.as_deref(), Some("Synthetic target"));
    let approved = w.desktop.approve_bulk_request(&id).unwrap();
    assert_eq!(approved.status, BulkRequestStatus::Applied);
    assert!(approved.decided_at.is_some());
    for task in &all {
        assert_eq!(
            task_now(&w, &task.id).list_id.as_deref(),
            Some(target.id.as_str())
        );
    }
    let entries = audit(&w.desktop);
    let moves: Vec<_> = entries
        .iter()
        .filter(|e| e.action.as_str() == "task.move")
        .collect();
    assert_eq!(moves.len(), 3);
    for entry in moves {
        assert_eq!(
            entry.interface,
            Interface::Desktop,
            "approved by the person"
        );
        assert_eq!(entry.requested_by, Some(Interface::Mcp), "asked by the AI");
    }
    let request = entries
        .iter()
        .find(|e| e.action.as_str() == "bulk.request")
        .unwrap();
    assert_eq!(
        (request.interface, request.entity_id.as_str()),
        (Interface::Mcp, id.as_str())
    );
    let approve = entries
        .iter()
        .find(|e| e.action.as_str() == "bulk.approve")
        .unwrap();
    assert_eq!(approve.interface, Interface::Desktop);
    assert_eq!(approve.outcome.as_str(), "applied");
    // The AI reads the outcome; a decided request cannot be decided again.
    assert_eq!(
        w.ai.bulk_request(&id).unwrap().status,
        BulkRequestStatus::Applied
    );
    assert!(w.desktop.pending_bulk_requests().unwrap().is_empty());
    assert_eq!(
        w.desktop.approve_bulk_request(&id).unwrap_err().code(),
        "unchanged"
    );
}

#[test]
fn declining_discards_the_request() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let id =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request
            .id;
    let declined = w.desktop.decline_bulk_request(&id).unwrap();
    assert_eq!(declined.status, BulkRequestStatus::Declined);
    assert!(!task_now(&w, &all[0].id).removed);
    assert_eq!(
        w.ai.bulk_request(&id).unwrap().status,
        BulkRequestStatus::Declined
    );
    assert!(
        audit(&w.desktop)
            .iter()
            .any(|e| e.action.as_str() == "bulk.decline" && e.interface == Interface::Desktop)
    );
    assert_eq!(
        w.desktop.approve_bulk_request(&id).unwrap_err().code(),
        "unchanged"
    );
}

#[test]
fn a_change_after_the_request_makes_it_stale_with_a_generic_audit_entry() {
    let w = world();
    let all = tasks(&w.ai, 3);
    let id =
        w.ai.request_bulk(
            BulkAction::SetTaskStatus {
                status: WorkStatus::Completed,
            },
            ids(&all),
        )
        .unwrap()
        .request
        .id;
    w.desktop
        .patch_task(PatchTask {
            id: all[2].id.clone(),
            expected_revision: 1,
            title: Some("Edited by the person".into()),
            description: None,
        })
        .unwrap();
    assert!(w.desktop.pending_bulk_requests().unwrap()[0].stale);
    let error = w.desktop.approve_bulk_request(&id).unwrap_err();
    assert_eq!(error.code(), "stale_preview");
    for task in &all {
        assert_eq!(
            task_now(&w, &task.id).status,
            WorkStatus::Ready,
            "nothing applied"
        );
    }
    assert_eq!(
        w.ai.bulk_request(&id).unwrap().status,
        BulkRequestStatus::Stale
    );
    // One generic refusal about the request itself, naming no item.
    let refused: Vec<_> = audit(&w.desktop)
        .into_iter()
        .filter(|e| e.outcome.as_str() == "rejected")
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert_eq!(refused[0].action.as_str(), "bulk.approve");
    assert_eq!(refused[0].entity_id, id);
    assert_eq!(refused[0].error_code.as_deref(), Some("stale_preview"));
    assert_eq!(refused[0].revision, None);
}

#[test]
fn renaming_or_removing_the_target_list_makes_a_move_stale() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let renamed = list(&w.ai, "Will be renamed");
    let removed = list(&w.ai, "Will be removed");
    let request = |target: &List| {
        w.ai.request_bulk(
            BulkAction::MoveTasks {
                list_id: Some(target.id.clone()),
            },
            ids(&all),
        )
        .unwrap()
        .request
        .id
    };
    let first = request(&renamed);
    let second = request(&removed);
    w.desktop
        .rename_list(RenameList {
            id: renamed.id.clone(),
            expected_revision: renamed.revision,
            name: "Renamed by the person".into(),
        })
        .unwrap();
    w.desktop
        .set_list_removed(SetListRemoved {
            id: removed.id.clone(),
            expected_revision: removed.revision,
            removed: true,
        })
        .unwrap();
    for id in [first, second] {
        assert_eq!(
            w.desktop.approve_bulk_request(&id).unwrap_err().code(),
            "stale_preview"
        );
    }
    assert!(task_now(&w, &all[0].id).list_id.is_none());
}

#[test]
fn a_request_expires_after_a_day_with_a_clear_status() {
    let w = world();
    let all = tasks(&w.ai, 1);
    let request =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request;
    let late = request.expires_at + 1;
    assert_eq!(
        w.ai.bulk_request_at(&request.id, late).unwrap().status,
        BulkRequestStatus::Expired
    );
    assert_eq!(
        w.ai.bulk_request_at(&request.id, request.expires_at)
            .unwrap()
            .status,
        BulkRequestStatus::Pending
    );
    let error = w
        .desktop
        .approve_bulk_request_at(&request.id, late)
        .unwrap_err();
    assert!(matches!(error, TaskError::PreviewExpired));
    assert!(!task_now(&w, &all[0].id).removed);
    assert_eq!(
        w.ai.bulk_request(&request.id).unwrap().status,
        BulkRequestStatus::Expired
    );
    assert!(w.desktop.pending_bulk_requests_at(late).unwrap().is_empty());
}

#[test]
fn a_request_is_refused_when_any_item_would_be_refused() {
    let w = world();
    let all = tasks(&w.ai, 3);
    w.ai.set_task_removed(SetRemoved {
        id: all[1].id.clone(),
        expected_revision: 1,
        removed: true,
    })
    .unwrap();
    let error =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap_err();
    assert_eq!(error.code(), "unchanged");
    assert!(error.to_string().starts_with("Item 2: "), "{error}");
    let missing =
        w.ai.request_bulk(BulkAction::RemoveTasks, vec!["no-such-task".into()])
            .unwrap_err();
    assert_eq!(missing.code(), "not_found");
    assert_eq!(w.desktop.count_pending_bulk_requests().unwrap(), 0);
}

#[test]
fn a_selection_is_one_to_a_hundred_distinct_ids() {
    let w = world();
    let all = tasks(&w.ai, 1);
    for selection in [
        Vec::new(),
        vec![all[0].id.clone(), all[0].id.clone()],
        vec!["x".to_owned(); BULK_MAX_ITEMS + 1],
    ] {
        let error =
            w.ai.request_bulk(BulkAction::RemoveTasks, selection)
                .unwrap_err();
        assert_eq!(error.code(), "invalid_input");
    }
    let two_lists = vec![list(&w.ai, "A").id, list(&w.ai, "B").id];
    assert_eq!(
        w.ai.request_bulk(BulkAction::RemoveList, two_lists)
            .unwrap_err()
            .code(),
        "invalid_input",
        "a List or folder removal names exactly one"
    );
}

#[test]
fn bulk_status_changes_follow_the_same_rules_as_change_task() {
    let w = world();
    // Twins: one pair changed one Task at a time, one through approvals.
    let one = tasks(&w.ai, 2);
    let bulk = tasks(&w.ai, 2);
    for pair in [&one, &bulk] {
        w.ai.change_task(ChangeTask {
            id: pair[1].id.clone(),
            expected_revision: 1,
            status: None,
            priority: None,
            progress: Some(40),
        })
        .unwrap();
    }
    for status in [WorkStatus::Completed, WorkStatus::InProgress] {
        for task in &one {
            w.ai.change_task(ChangeTask {
                id: task.id.clone(),
                expected_revision: task_now(&w, &task.id).revision,
                status: Some(status),
                priority: None,
                progress: None,
            })
            .unwrap();
        }
        let id =
            w.ai.request_bulk(BulkAction::SetTaskStatus { status }, ids(&bulk))
                .unwrap()
                .request
                .id;
        w.desktop.approve_bulk_request(&id).unwrap();
        for (a, b) in one.iter().zip(&bulk) {
            let (a, b) = (task_now(&w, &a.id), task_now(&w, &b.id));
            assert_eq!((a.status, a.progress), (b.status, b.progress), "{status:?}");
            let history = |id: &str| {
                w.desktop
                    .list_progress_entries(id, 50, None)
                    .unwrap()
                    .items
                    .len()
            };
            assert_eq!(history(&a.id), history(&b.id), "same Progress Entries");
        }
    }
    // Back from Completed, In Progress restored 0% and 40%, as for one Task.
    assert_eq!(task_now(&w, &bulk[1].id).progress, 40);
    // Ready for started work, or a status a Task already has, is refused
    // exactly as for one Task.
    for status in [WorkStatus::Ready, WorkStatus::InProgress] {
        let single =
            w.ai.change_task(ChangeTask {
                id: one[1].id.clone(),
                expected_revision: task_now(&w, &one[1].id).revision,
                status: Some(status),
                priority: None,
                progress: None,
            })
            .unwrap_err();
        let error =
            w.ai.request_bulk(BulkAction::SetTaskStatus { status }, ids(&bulk))
                .unwrap_err();
        assert_eq!(error.code(), single.code(), "{status:?}");
    }
}

#[test]
fn bulk_notes_respect_exclude_from_ai_like_single_changes() {
    let w = world();
    let shared = note(&w.desktop, "# Shared synthetic", None);
    let private = note(&w.desktop, "# Private synthetic", None);
    exclude(&w.desktop, &private.id);
    let error =
        w.ai.request_bulk(
            BulkAction::RemoveNotes,
            vec![shared.id.clone(), private.id.clone()],
        )
        .unwrap_err();
    let never =
        w.ai.request_bulk(
            BulkAction::RemoveNotes,
            vec![shared.id.clone(), "never-used".into()],
        )
        .unwrap_err();
    assert_eq!(error.to_string(), never.to_string());
    assert_eq!(error.code(), "not_found");

    // Excluded between the request and the approval: stale, generically.
    let id =
        w.ai.request_bulk(BulkAction::RemoveNotes, vec![shared.id.clone()])
            .unwrap()
            .request
            .id;
    exclude(&w.desktop, &shared.id);
    assert_eq!(
        w.desktop.approve_bulk_request(&id).unwrap_err().code(),
        "stale_preview"
    );
    assert!(!w.desktop.get_note(&shared.id).unwrap().unwrap().removed);
    let refusal = audit(&w.desktop)
        .into_iter()
        .find(|e| e.outcome.as_str() == "rejected")
        .unwrap();
    assert_eq!(refusal.entity_id, id, "names the request, not the Note");
    // The AI sees the refusal about its request, and nothing about the Note.
    let ai_audit = w.ai.list_audit_entries(None, 100, None).unwrap().items;
    assert!(
        ai_audit
            .iter()
            .any(|e| e.entity_id == id && e.outcome.as_str() == "rejected")
    );
    assert!(ai_audit.iter().all(|e| e.entity_id != shared.id));
}

#[test]
fn an_ai_removes_an_empty_list_directly_but_one_holding_tasks_needs_approval() {
    let w = world();
    let empty = list(&w.ai, "Empty synthetic");
    w.ai.set_list_removed(SetListRemoved {
        id: empty.id.clone(),
        expected_revision: empty.revision,
        removed: true,
    })
    .unwrap();
    let full = list(&w.ai, "Full synthetic");
    for i in 0..2 {
        w.ai.create_task_in_list(
            CreateTask {
                title: format!("In the List {i}"),
                description: String::new(),
            },
            Some(&full.id),
        )
        .unwrap();
    }
    for store in [&w.ai, &w.cli] {
        let error = store
            .set_list_removed(SetListRemoved {
                id: full.id.clone(),
                expected_revision: full.revision,
                removed: true,
            })
            .unwrap_err();
        assert!(matches!(error, TaskError::NeedsApproval), "{error:?}");
        assert_eq!(error.code(), "needs_approval");
    }
    // The person in the window removes it directly, as before.
    let requested =
        w.ai.request_bulk(BulkAction::RemoveList, vec![full.id.clone()])
            .unwrap();
    assert_eq!(requested.request.count, None, "no counts for a container");
    assert!(requested.items.is_empty());
    let pending = &w.desktop.pending_bulk_requests().unwrap()[0];
    assert_eq!(pending.target.as_deref(), Some("Full synthetic"));
    assert_eq!(pending.total, 2);
    assert!(pending.items.iter().all(|l| l.kind == ApprovalKind::Task));
    w.desktop
        .approve_bulk_request(&requested.request.id)
        .unwrap();
    let now = w
        .desktop
        .list_lists(100, None, true)
        .unwrap()
        .items
        .into_iter()
        .find(|l| l.id == full.id)
        .unwrap();
    assert!(now.removed);
    let entry = audit(&w.desktop)
        .into_iter()
        .find(|e| e.action.as_str() == "list.remove" && e.entity_id == full.id)
        .unwrap();
    assert_eq!(entry.requested_by, Some(Interface::Mcp));
}

#[test]
fn a_folder_holding_only_excluded_notes_needs_approval_without_revealing_them() {
    let w = world();
    let folder = |name: &str| {
        w.desktop
            .create_note_folder(CreateNoteFolder {
                name: name.into(),
                parent_id: None,
            })
            .unwrap()
    };
    let empty = folder("Empty synthetic folder");
    w.ai.set_note_folder_removed(SetRemoved {
        id: empty.id.clone(),
        expected_revision: empty.revision,
        removed: true,
    })
    .unwrap();
    let private_only = folder("Private synthetic folder");
    let secret = note(&w.desktop, "# Excluded synthetic", Some(&private_only.id));
    exclude(&w.desktop, &secret.id);
    assert_eq!(
        w.ai.count_folder_notes(Some(&private_only.id)).unwrap(),
        0,
        "the AI cannot count it"
    );
    let error =
        w.ai.set_note_folder_removed(SetRemoved {
            id: private_only.id.clone(),
            expected_revision: private_only.revision,
            removed: true,
        })
        .unwrap_err();
    assert_eq!(error.code(), "needs_approval");
    assert!(!error.to_string().contains('1'), "no count: {error}");
    let requested =
        w.ai.request_bulk(BulkAction::RemoveFolder, vec![private_only.id.clone()])
            .unwrap();
    assert_eq!(requested.request.count, None);
    assert!(requested.items.is_empty());
    // The person sees everything that would be hidden, private Notes too.
    let pending = &w.desktop.pending_bulk_requests().unwrap()[0];
    assert_eq!(pending.items.len(), 1);
    assert_eq!(pending.items[0].kind, ApprovalKind::Note);
    assert_eq!(pending.items[0].title, "Excluded synthetic");
    assert!(pending.items[0].excluded_from_ai);
    w.desktop
        .approve_bulk_request(&requested.request.id)
        .unwrap();
    assert!(
        w.desktop
            .get_note_folder(&private_only.id)
            .unwrap()
            .unwrap()
            .removed
    );
}

#[test]
fn a_request_lives_in_the_database_not_the_process() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasknotes.db");
    let id = {
        let ai = TaskStore::open_as(&path, Interface::Cli).unwrap();
        let all = tasks(&ai, 1);
        ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request
            .id
    };
    // The desktop opens later, in another process.
    let desktop = TaskStore::open(&path).unwrap();
    assert_eq!(desktop.pending_bulk_requests().unwrap()[0].request.id, id);
    assert_eq!(
        desktop.pending_bulk_requests().unwrap()[0]
            .request
            .requested_by,
        Interface::Cli
    );
    desktop.approve_bulk_request(&id).unwrap();
}

fn asked_by_an_ai(desktop: &TaskStore) -> usize {
    audit(desktop)
        .iter()
        .filter(|e| e.requested_by == Some(Interface::Mcp))
        .count()
}

#[test]
fn a_restore_keeps_who_asked_for_an_approved_bulk_change() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let backup = w.desktop.create_backup().unwrap().backup;
    let id =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request
            .id;
    w.desktop.approve_bulk_request(&id).unwrap();
    let before = asked_by_an_ai(&w.desktop);
    assert!(before > 0);

    w.desktop.restore_backup(&backup.id).unwrap();

    // Audit Entries stay append-only across a restore, whole.
    assert_eq!(asked_by_an_ai(&w.desktop), before);
}

#[test]
fn a_store_opened_before_a_restore_can_neither_ask_nor_approve() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let id =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request
            .id;
    let backup = w.desktop.create_backup().unwrap().backup;
    let path = w._dir.path().join("tasknotes.db");
    let other_window = TaskStore::open(&path).unwrap();
    other_window.restore_backup(&backup.id).unwrap();

    assert_eq!(
        w.desktop.approve_bulk_request(&id).unwrap_err().code(),
        "database_restored"
    );
    assert_eq!(
        w.desktop.decline_bulk_request(&id).unwrap_err().code(),
        "database_restored"
    );
    assert_eq!(
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap_err()
            .code(),
        "database_restored"
    );
    assert!(!task_now_in(&other_window, &all[0].id).removed);
}

#[test]
fn a_request_made_after_the_backup_does_not_come_back_from_a_restore() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let backup = w.desktop.create_backup().unwrap().backup;
    let id =
        w.ai.request_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap()
            .request
            .id;

    w.desktop.restore_backup(&backup.id).unwrap();

    assert_eq!(w.desktop.count_pending_bulk_requests().unwrap(), 0);
    assert_eq!(
        w.desktop.approve_bulk_request(&id).unwrap_err().code(),
        "not_found"
    );
    // Its Audit Entry remains: what was asked is history, not current data.
    assert!(audit(&w.desktop).iter().any(|e| e.entity_id == id));
}

fn task_now_in(store: &TaskStore, id: &str) -> Task {
    store.get_task(id).unwrap().unwrap()
}
