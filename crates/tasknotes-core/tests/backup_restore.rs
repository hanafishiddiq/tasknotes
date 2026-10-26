//! Ticket 13: local backups, validated restore and writer coordination.
//! Every test works in its own temporary data directory with synthetic data;
//! nothing here can reach a real Tasknotes database.

use std::path::PathBuf;

use tasknotes_core::audit::Interface;
use tasknotes_core::backup::{Backup, BackupKind};
use tasknotes_core::tasks::{CreateTask, Task, TaskError, TaskStore, UpdateTask};

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn db(&self) -> PathBuf {
        self.dir.path().join("tasknotes.db")
    }

    fn backups_dir(&self) -> PathBuf {
        self.dir.path().join("backups")
    }

    fn desktop(&self) -> TaskStore {
        TaskStore::open(&self.db()).unwrap()
    }

    fn cli(&self) -> TaskStore {
        TaskStore::open_as(&self.db(), Interface::Cli).unwrap()
    }
}

fn task(store: &TaskStore, title: &str) -> Task {
    store
        .create_task(CreateTask {
            title: title.into(),
            description: "Synthetic description".into(),
        })
        .unwrap()
}

fn rename(store: &TaskStore, task: &Task, title: &str) -> Result<Task, TaskError> {
    store.update_task(UpdateTask {
        id: task.id.clone(),
        title: title.into(),
        description: task.description.clone(),
        expected_revision: task.revision,
    })
}

fn titles(store: &TaskStore) -> Vec<String> {
    let mut titles: Vec<String> = store
        .list_tasks(100, None)
        .unwrap()
        .items
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    titles
}


#[test]
fn restoring_a_backup_brings_back_its_data_and_refuses_every_store_opened_before() {
    let fx = Fixture::new();
    let desktop = fx.desktop();
    let kept = task(&desktop, "Synthetic kept task");
    let backup: Backup = desktop.create_backup().unwrap();
    assert_eq!(backup.kind, BackupKind::Manual);

    rename(&desktop, &kept, "Synthetic renamed after backup").unwrap();
    task(&desktop, "Synthetic task added after backup");
    let other_process = fx.cli();

    let outcome = desktop.restore_backup(&backup.id).unwrap();
    assert_eq!(outcome.restored.id, backup.id);

    // The restoring store carries on with the restored data.
    assert_eq!(titles(&desktop), ["Synthetic kept task"]);

    // A store opened before the restore (another window, CLI or MCP) holds a
    // picture of the old data: every read and write is refused until it
    // reopens, so a stale revision can never land on the restored data.
    let stale = rename(&other_process, &kept, "Synthetic stale write");
    assert_eq!(stale.unwrap_err().code(), "database_restored");
    let stale_read = other_process.list_tasks(10, None);
    assert_eq!(stale_read.unwrap_err().code(), "database_restored");

    let reopened = fx.cli();
    assert_eq!(titles(&reopened), ["Synthetic kept task"]);
    assert!(fx.backups_dir().exists());
}
