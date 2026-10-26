//! Ticket 12: high-risk bulk changes are previewed, then applied all or
//! nothing only while every previewed item is unchanged. Synthetic data only.

use tasknotes_core::audit::Interface;
use tasknotes_core::bulk::{BULK_MAX_ITEMS, BULK_PREVIEW_TTL_MS, BulkAction, BulkItem};
use tasknotes_core::notes::{CreateNote, Note};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::{
    CreateList, CreateTask, PatchTask, Task, TaskError, TaskStore, WorkStatus,
};

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

fn note(store: &TaskStore, markdown: &str) -> Note {
    store
        .create_note(CreateNote {
            markdown: markdown.into(),
            folder_id: None,
        })
        .unwrap()
}

/// Items with each after-state's edit time cleared.
fn untimed(items: Vec<BulkItem>) -> Vec<BulkItem> {
    items
        .into_iter()
        .map(|item| match item {
            BulkItem::Task { before, mut after } => {
                after.updated_at = 0;
                BulkItem::Task { before, after }
            }
            note => note,
        })
        .collect()
}

#[test]
fn a_preview_shows_each_change_and_changes_nothing() {
    let w = world();
    let all = tasks(&w.ai, 3);
    let preview =
        w.ai.preview_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap();
    assert_eq!(preview.items.len(), 3);
    for (item, task) in preview.items.iter().zip(&all) {
        let BulkItem::Task { before, after } = item else {
            panic!("a Task item")
        };
        assert_eq!(before, task);
        assert!(after.removed);
        assert_eq!(after.revision, task.revision + 1);
    }
    assert_eq!(preview.plan.len(), 3);
    assert_eq!(preview.plan.action(), &BulkAction::RemoveTasks);
    // Nothing was written: no Task removed, no Audit Entry beyond the creates.
    for task in &all {
        assert_eq!(w.desktop.get_task(&task.id).unwrap().unwrap(), *task);
    }
    assert_eq!(
        w.desktop
            .list_audit_entries(None, 100, None)
            .unwrap()
            .items
            .len(),
        3
    );
}

#[test]
fn confirming_an_unchanged_preview_applies_every_item_with_audit_entries() {
    let w = world();
    let all = tasks(&w.ai, 3);
    let list =
        w.ai.create_list(CreateList {
            name: "Synthetic target".into(),
        })
        .unwrap();
    let preview =
        w.ai.preview_bulk(
            BulkAction::MoveTasks {
                list_id: Some(list.id.clone()),
            },
            ids(&all),
        )
        .unwrap();
    let applied = w.ai.apply_bulk(&preview.plan).unwrap();
    assert_eq!(
        untimed(applied),
        untimed(preview.items.clone()),
        "the preview was exactly what happened, apart from the edit time"
    );
    for task in &all {
        let now = w.desktop.get_task(&task.id).unwrap().unwrap();
        assert_eq!(now.list_id.as_deref(), Some(list.id.as_str()));
    }
    let audit = w.desktop.list_audit_entries(None, 100, None).unwrap().items;
    let moves = audit
        .iter()
        .filter(|e| e.action.as_str() == "task.move")
        .count();
    assert_eq!(moves, 3);
    // A plan is used once: its revisions are now stale.
    assert!(matches!(
        w.ai.apply_bulk(&preview.plan),
        Err(TaskError::StalePreview)
    ));
}

#[test]
fn a_change_after_the_preview_refuses_the_whole_batch() {
    let w = world();
    let all = tasks(&w.ai, 3);
    let preview =
        w.ai.preview_bulk(
            BulkAction::SetTaskStatus {
                status: WorkStatus::Completed,
            },
            ids(&all),
        )
        .unwrap();
    // The person edits the last Task in the window after the preview.
    w.desktop
        .patch_task(PatchTask {
            id: all[2].id.clone(),
            expected_revision: 1,
            title: Some("Edited by the person".into()),
            description: None,
        })
        .unwrap();
    let error = w.ai.apply_bulk(&preview.plan).unwrap_err();
    assert!(matches!(error, TaskError::StalePreview));
    assert_eq!(error.code(), "stale_preview");
    for task in &all {
        let now = w.desktop.get_task(&task.id).unwrap().unwrap();
        assert_eq!(now.status, WorkStatus::Ready, "nothing applied");
    }
    assert_eq!(
        w.desktop.get_task(&all[2].id).unwrap().unwrap().title,
        "Edited by the person"
    );
}

#[test]
fn a_target_list_removed_after_the_preview_also_refuses_the_batch() {
    let w = world();
    let all = tasks(&w.ai, 2);
    let list =
        w.ai.create_list(CreateList {
            name: "Soon gone".into(),
        })
        .unwrap();
    let preview =
        w.ai.preview_bulk(
            BulkAction::MoveTasks {
                list_id: Some(list.id.clone()),
            },
            ids(&all),
        )
        .unwrap();
    w.desktop
        .set_list_removed(tasknotes_core::tasks::SetListRemoved {
            id: list.id,
            expected_revision: list.revision,
            removed: true,
        })
        .unwrap();
    assert!(matches!(
        w.ai.apply_bulk(&preview.plan),
        Err(TaskError::StalePreview)
    ));
    assert!(
        w.desktop
            .get_task(&all[0].id)
            .unwrap()
            .unwrap()
            .list_id
            .is_none()
    );
}

#[test]
fn an_old_preview_expires() {
    let w = world();
    let all = tasks(&w.ai, 1);
    let preview =
        w.ai.preview_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap();
    let late = preview.plan.expires_at() + 1;
    assert_eq!(
        preview.plan.expires_at() - preview.plan.made_at(),
        BULK_PREVIEW_TTL_MS
    );
    let error = w.ai.apply_bulk_at(&preview.plan, late).unwrap_err();
    assert!(matches!(error, TaskError::PreviewExpired));
    assert_eq!(error.code(), "stale_preview");
    assert!(!w.desktop.get_task(&all[0].id).unwrap().unwrap().removed);
    w.ai.apply_bulk_at(&preview.plan, preview.plan.expires_at())
        .unwrap();
}

#[test]
fn a_preview_is_refused_when_any_item_would_be_refused() {
    let w = world();
    let all = tasks(&w.ai, 3);
    w.ai.set_task_removed(tasknotes_core::removed::SetRemoved {
        id: all[1].id.clone(),
        expected_revision: 1,
        removed: true,
    })
    .unwrap();
    let error =
        w.ai.preview_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap_err();
    assert_eq!(error.code(), "unchanged");
    assert!(error.to_string().starts_with("Item 2: "), "{error}");
    let missing =
        w.ai.preview_bulk(BulkAction::RemoveTasks, vec!["no-such-task".into()])
            .unwrap_err();
    assert_eq!(missing.code(), "not_found");
    // Ready is refused for started work, as for one Task.
    let started =
        w.ai.change_task(tasknotes_core::tasks::ChangeTask {
            id: all[0].id.clone(),
            expected_revision: 1,
            status: None,
            priority: None,
            progress: Some(30),
        })
        .unwrap();
    let error =
        w.ai.preview_bulk(
            BulkAction::SetTaskStatus {
                status: WorkStatus::Ready,
            },
            vec![started.id, all[2].id.clone()],
        )
        .unwrap_err();
    assert_eq!(error.code(), "invalid_transition");
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
            w.ai.preview_bulk(BulkAction::RemoveTasks, selection)
                .unwrap_err();
        assert_eq!(error.code(), "invalid_input");
    }
}

#[test]
fn bulk_notes_respect_exclude_from_ai_like_single_changes() {
    let w = world();
    let shared = note(&w.desktop, "# Shared synthetic");
    let private = note(&w.desktop, "# Private synthetic");
    w.desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let error =
        w.ai.preview_bulk(
            BulkAction::RemoveNotes,
            vec![shared.id.clone(), private.id.clone()],
        )
        .unwrap_err();
    let never =
        w.ai.preview_bulk(
            BulkAction::RemoveNotes,
            vec![shared.id.clone(), "never-used".into()],
        )
        .unwrap_err();
    assert_eq!(error.to_string(), never.to_string());
    assert_eq!(error.code(), "not_found");

    // Excluded between preview and confirm: the generic stale refusal.
    let preview =
        w.ai.preview_bulk(BulkAction::RemoveNotes, vec![shared.id.clone()])
            .unwrap();
    let BulkItem::Note { after, .. } = &preview.items[0] else {
        panic!("a Note item")
    };
    assert!(after.removed);
    w.desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: shared.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    assert!(matches!(
        w.ai.apply_bulk(&preview.plan),
        Err(TaskError::StalePreview)
    ));
    assert!(!w.desktop.get_note(&shared.id).unwrap().unwrap().removed);
}

#[test]
fn a_plan_only_applies_through_the_interface_that_previewed_it() {
    let w = world();
    let all = tasks(&w.ai, 1);
    let preview =
        w.ai.preview_bulk(BulkAction::RemoveTasks, ids(&all))
            .unwrap();
    assert!(matches!(
        w.desktop.apply_bulk(&preview.plan),
        Err(TaskError::StalePreview)
    ));
    let restore =
        w.ai.preview_bulk(BulkAction::RestoreTasks, ids(&all))
            .unwrap_err();
    assert_eq!(restore.code(), "unchanged");
}
