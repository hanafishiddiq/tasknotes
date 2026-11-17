//! Images in a Task's notes (owner, batch 4, 2026-11-18): an image pasted
//! into a Task's notes is kept in the database (so every backup holds it)
//! and the notes keep a short reference to it, `![image](tasknotes-image:ID)`.
//! An image is never deleted. Synthetic data only.

use std::path::Path;

use tasknotes_core::removed::SetRemoved;
use tasknotes_core::task_images::{AddTaskImage, image_reference};
use tasknotes_core::tasks::{CreateTask, Task, TaskStore};

/// The smallest PNG signature with a few bytes after it.
const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 16];

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn task(store: &TaskStore) -> Task {
    store
        .create_task(CreateTask {
            title: "Synthetic shelf".into(),
            description: String::new(),
        })
        .unwrap()
}

fn add(
    store: &TaskStore,
    task: &Task,
    media_type: &str,
    bytes: &[u8],
) -> Result<tasknotes_core::task_images::TaskImage, tasknotes_core::tasks::TaskError> {
    store.add_task_image(AddTaskImage {
        task_id: task.id.clone(),
        media_type: media_type.into(),
        bytes: bytes.to_vec(),
    })
}

#[test]
fn an_image_is_kept_and_read_back_whole() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let shelf = task(&store);
    let image = add(&store, &shelf, "image/png", PNG).unwrap();
    assert_eq!(
        (
            image.task_id.as_str(),
            image.media_type.as_str(),
            image.size
        ),
        (shelf.id.as_str(), "image/png", PNG.len() as i64)
    );
    let (read, bytes) = store.read_task_image(&image.id).unwrap();
    assert_eq!(read, image);
    assert_eq!(bytes, PNG);
    assert_eq!(
        image_reference(&image.id),
        format!("![image](tasknotes-image:{})", image.id)
    );
    // Adding an image writes nothing on the Task itself: the notes hold the reference.
    assert_eq!(
        store.get_task(&shelf.id).unwrap().unwrap().revision,
        shelf.revision
    );
}

#[test]
fn only_real_images_of_the_kinds_the_window_shows_and_at_most_10_mb() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let shelf = task(&store);
    assert!(add(&store, &shelf, "image/jpeg", JPEG).is_ok());
    for (media_type, bytes) in [
        ("image/png", JPEG),                   // says PNG, is not
        ("image/svg+xml", b"<svg/>" as &[u8]), // can carry scripts
        ("text/plain", b"hello" as &[u8]),
        ("image/png", &[] as &[u8]),
    ] {
        let refused = add(&store, &shelf, media_type, bytes).unwrap_err();
        assert_eq!(refused.code(), "invalid_input", "{media_type}");
    }
    let mut big = PNG.to_vec();
    big.resize(10 * 1024 * 1024 + 1, 0);
    assert_eq!(
        add(&store, &shelf, "image/png", &big).unwrap_err().code(),
        "invalid_input"
    );
}

#[test]
fn a_removed_task_takes_no_new_images_but_keeps_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let shelf = task(&store);
    let image = add(&store, &shelf, "image/png", PNG).unwrap();
    store
        .set_task_removed(SetRemoved {
            id: shelf.id.clone(),
            expected_revision: shelf.revision,
            removed: true,
        })
        .unwrap();
    assert_eq!(
        add(&store, &shelf, "image/png", PNG).unwrap_err().code(),
        "removed"
    );
    assert!(store.read_task_image(&image.id).is_ok());
    assert_eq!(
        store.read_task_image("no-such-image").unwrap_err().code(),
        "not_found"
    );
}

#[test]
fn adding_an_image_is_audited() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let shelf = task(&store);
    let image = add(&store, &shelf, "image/png", PNG).unwrap();
    let entries = store
        .list_audit_entries(Some(&image.id), 10, None)
        .unwrap()
        .items;
    assert_eq!(entries[0].action.as_str(), "task.image.add");
}

#[test]
fn exports_carry_the_images_and_markdown_points_at_their_files() {
    use tasknotes_core::export::{ExportFormat, ExportRequest};
    use tasknotes_core::tasks::UpdateTask;
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let shelf = task(&store);
    let image = add(&store, &shelf, "image/png", PNG).unwrap();
    store
        .update_task(UpdateTask {
            id: shelf.id.clone(),
            title: shelf.title.clone(),
            description: format!("Before\n\n{}\n\nAfter", image_reference(&image.id)),
            expected_revision: shelf.revision,
        })
        .unwrap();

    let json = dir.path().join("out.json");
    store
        .export_data(ExportRequest {
            format: ExportFormat::Json,
            destination: json.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(data["task_images"][0]["id"], image.id.as_str());
    // 12 bytes of PNG in Base64.
    assert_eq!(data["task_images"][0]["data_base64"], "iVBORw0KGgoAAAAN");

    let root = dir.path().join("md");
    store
        .export_data(ExportRequest {
            format: ExportFormat::Markdown,
            destination: root.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let file = root.join(format!("Tasks/images/{}.png", image.id));
    assert_eq!(std::fs::read(&file).unwrap(), PNG);
    let inbox = std::fs::read_to_string(root.join("Tasks/Inbox.md")).unwrap();
    assert!(
        inbox.contains(&format!("![image](images/{}.png)", image.id)),
        "{inbox}"
    );
}
