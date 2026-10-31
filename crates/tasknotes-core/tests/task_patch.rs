//! Partial Task edits for the AI adapters (CLI, MCP): a field that is not
//! given keeps its current value, and the caller's revision is still checked.
//! Synthetic data only.

use tasknotes_core::tasks::{
    CreateTask, PatchTask, PatchTaskDetails, Task, TaskError, TaskStore, UpdateTaskDetails,
};

fn fixture() -> (tempfile::TempDir, TaskStore, Task) {
    let dir = tempfile::tempdir().unwrap();
    let store = TaskStore::open(&dir.path().join("t.db")).unwrap();
    let task = store
        .create_task(CreateTask {
            title: "Water the plants".into(),
            description: "Balcony and kitchen".into(),
        })
        .unwrap();
    let task = store
        .update_task_details(UpdateTaskDetails {
            id: task.id,
            expected_revision: task.revision,
            tags: vec!["home".into()],
            due_date: Some("2030-05-01".into()),
        })
        .unwrap();
    (dir, store, task)
}

#[test]
fn a_text_patch_keeps_the_field_it_does_not_name() {
    let (_dir, store, task) = fixture();
    let patched = store
        .patch_task(PatchTask {
            id: task.id.clone(),
            expected_revision: task.revision,
            title: Some("Water every plant".into()),
            description: None,
        })
        .unwrap();
    assert_eq!(patched.title, "Water every plant");
    assert_eq!(patched.description, "Balcony and kitchen");
    assert_eq!(patched.revision, task.revision + 1);

    let only_description = store
        .patch_task(PatchTask {
            id: task.id.clone(),
            expected_revision: patched.revision,
            title: None,
            description: Some(String::new()),
        })
        .unwrap();
    assert_eq!(only_description.title, "Water every plant");
    assert_eq!(only_description.description, "");
}

#[test]
fn a_stale_or_empty_text_patch_changes_nothing() {
    let (_dir, store, task) = fixture();
    store
        .patch_task(PatchTask {
            id: task.id.clone(),
            expected_revision: task.revision,
            title: Some("First writer".into()),
            description: None,
        })
        .unwrap();
    let stale = store.patch_task(PatchTask {
        id: task.id.clone(),
        expected_revision: task.revision,
        title: None,
        description: Some("Second writer".into()),
    });
    assert!(matches!(stale, Err(TaskError::RevisionConflict)));
    let empty = store.patch_task(PatchTask {
        id: task.id.clone(),
        expected_revision: task.revision + 1,
        title: None,
        description: None,
    });
    assert_eq!(empty.unwrap_err().code(), "invalid_input");
    let missing = store.patch_task(PatchTask {
        id: "no-such-task".into(),
        expected_revision: 1,
        title: Some("Ghost".into()),
        description: None,
    });
    assert!(matches!(missing, Err(TaskError::NotFound)));
    let current = store.get_task(&task.id).unwrap().unwrap();
    assert_eq!(
        (current.title.as_str(), current.description.as_str()),
        ("First writer", "Balcony and kitchen")
    );
}

#[test]
fn a_details_patch_sets_clears_or_keeps_tags_and_deadline_independently() {
    let (_dir, store, task) = fixture();
    let tags_only = store
        .patch_task_details(PatchTaskDetails {
            id: task.id.clone(),
            expected_revision: task.revision,
            tags: Some(vec!["garden".into()]),
            due_date: None,
        })
        .unwrap();
    assert_eq!(tags_only.tags, vec!["garden"]);
    assert_eq!(tags_only.due_date.as_deref(), Some("2030-05-01"));

    let cleared = store
        .patch_task_details(PatchTaskDetails {
            id: task.id.clone(),
            expected_revision: tags_only.revision,
            tags: None,
            due_date: Some(None),
        })
        .unwrap();
    assert_eq!(cleared.tags, vec!["garden"]);
    assert_eq!(cleared.due_date, None);

    let empty = store.patch_task_details(PatchTaskDetails {
        id: task.id.clone(),
        expected_revision: cleared.revision,
        tags: None,
        due_date: None,
    });
    assert_eq!(empty.unwrap_err().code(), "invalid_input");
    let stale = store.patch_task_details(PatchTaskDetails {
        id: task.id.clone(),
        expected_revision: task.revision,
        tags: Some(Vec::new()),
        due_date: None,
    });
    assert!(matches!(stale, Err(TaskError::RevisionConflict)));
}
