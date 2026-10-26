//! Ticket 13: local backups, validated restore and writer coordination.
//! Every test works in its own temporary data directory with synthetic data;
//! nothing here can reach a real Tasknotes database.

use std::path::PathBuf;

use tasknotes_core::audit::Interface;
use tasknotes_core::backup::{Backup, BackupKind};
use tasknotes_core::notes::CreateNote;
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{CreateTask, Task, TaskError, TaskStore, UpdateTask};

const DAY_MS: i64 = 86_400_000;
/// 2026-10-26 00:00 UTC, a Monday.
const MONDAY_2026_10_26: i64 = 20_752 * DAY_MS;

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

fn day(date: &str) -> i64 {
    // Days from 2026-10-26 counted on the calendar, for readable fixtures.
    let (y, m, d) = (&date[0..4], &date[5..7], &date[8..10]);
    let (y, m, d): (i64, i64, i64) = (y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap());
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * DAY_MS
}

fn snapshot_path(fx: &Fixture, backup: &Backup) -> PathBuf {
    fx.backups_dir().join(format!("{}.sqlite3", backup.id))
}

fn bytes_of(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

#[test]
fn the_fake_clock_agrees_with_the_calendar() {
    assert_eq!(day("2026-10-26"), MONDAY_2026_10_26);
}

#[test]
fn daily_backups_follow_the_7_daily_4_weekly_3_monthly_policy() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic task for retention");
    let hour = 3_600_000;
    let mut now = day("2026-06-01") + 9 * hour;
    while now <= day("2026-10-26") + 9 * hour {
        let first = store.ensure_daily_backup_at(now).unwrap();
        assert!(first.is_some(), "one automatic backup per day");
        // Idempotent for the rest of the day, in this or any other process.
        assert!(
            store
                .ensure_daily_backup_at(now + 5 * hour)
                .unwrap()
                .is_none()
        );
        assert!(
            fx.cli()
                .ensure_daily_backup_at(now + 6 * hour)
                .unwrap()
                .is_none()
        );
        now += DAY_MS;
    }
    let mut kept: Vec<i64> = store
        .list_backups()
        .unwrap()
        .iter()
        .inspect(|b| assert_eq!(b.kind, BackupKind::Daily))
        .map(|b| b.created_at - 9 * hour)
        .collect();
    kept.sort();
    let expected: Vec<i64> = [
        "2026-08-31", // monthly: newest of August
        "2026-09-30", // monthly: newest of September
        "2026-10-11", // weekly: Sunday ending the week of 5 October
        "2026-10-18", // weekly: Sunday ending the week of 12 October
        // daily: the last seven days; 10-25 and 10-26 are also the newest
        // of their weeks and 10-26 of October.
        "2026-10-20",
        "2026-10-21",
        "2026-10-22",
        "2026-10-23",
        "2026-10-24",
        "2026-10-25",
        "2026-10-26",
    ]
    .iter()
    .map(|d| day(d))
    .collect();
    assert_eq!(kept, expected);
    // Pruned backups leave no files behind: one snapshot and one manifest each.
    assert_eq!(std::fs::read_dir(fx.backups_dir()).unwrap().count(), 22);
}

#[test]
fn a_damaged_or_truncated_snapshot_is_refused_and_the_live_data_is_untouched() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic live task");
    let backup = store.create_backup().unwrap();
    let path = snapshot_path(&fx, &backup);
    let original = bytes_of(&path);
    let live_before = titles(&store);

    // Truncated: half the file is gone.
    std::fs::write(&path, &original[..original.len() / 2]).unwrap();
    let refused = store.restore_backup(&backup.id).unwrap_err();
    assert_eq!(refused.code(), "invalid_backup");

    // Bit rot: same size, one byte flipped.
    let mut flipped = original.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 0xFF;
    std::fs::write(&path, &flipped).unwrap();
    assert_eq!(
        store.restore_backup(&backup.id).unwrap_err().code(),
        "invalid_backup"
    );

    // The live store was never replaced: same data, still writable, and no
    // other store was forced to reload.
    assert_eq!(titles(&store), live_before);
    assert_eq!(titles(&fx.cli()), live_before);
    task(&store, "Synthetic task after refused restores");
    // Nothing but the backup itself is left: no staging copy, no
    // Pre-restore backup of an unchanged database.
    assert_eq!(store.list_backups().unwrap(), [backup]);
    assert_eq!(std::fs::read_dir(fx.backups_dir()).unwrap().count(), 2);
}

#[test]
fn a_damaged_snapshot_with_a_matching_manifest_fails_the_integrity_check() {
    let fx = Fixture::new();
    let store = fx.desktop();
    for i in 0..200 {
        task(
            &store,
            &format!("Synthetic task {i} with enough text to fill pages"),
        );
    }
    let backup = store.create_backup().unwrap();
    let path = snapshot_path(&fx, &backup);
    // Scribble over the cells at the end of one index page, then make the
    // manifest agree, as a careless copy tool or a hand edit might. Table
    // reads still work; only the integrity check notices.
    let root: usize = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = 'tasks_created_order'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut bytes = bytes_of(&path);
    let page_end = root * 4096;
    for byte in &mut bytes[page_end - 64..page_end] {
        *byte = 0x5A;
    }
    std::fs::write(&path, &bytes).unwrap();
    let manifest_path = fx.backups_dir().join(format!("{}.json", backup.id));
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["sha256"] = sha256_hex(&bytes).into();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();

    let before = titles(&store);
    let refused = store.restore_backup(&backup.id).unwrap_err();
    assert_eq!(refused.code(), "invalid_backup");
    assert!(
        refused.to_string().contains("the snapshot is damaged."),
        "{refused}"
    );
    assert_eq!(titles(&store), before);
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn a_file_that_is_not_a_tasknotes_database_or_is_newer_is_refused() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic live task");

    for (contents, code) in [
        (Contents::Text, "invalid_backup"),
        (Contents::ForeignSqlite, "invalid_backup"),
        (Contents::NewerSchema, "unsupported_schema"),
    ] {
        let backup = store.create_backup().unwrap();
        let path = snapshot_path(&fx, &backup);
        match contents {
            Contents::Text => std::fs::write(&path, b"synthetic, not a database").unwrap(),
            Contents::ForeignSqlite => {
                std::fs::remove_file(&path).unwrap();
                rusqlite::Connection::open(&path)
                    .unwrap()
                    .execute_batch("CREATE TABLE other(x); PRAGMA user_version = 0;")
                    .unwrap();
            }
            Contents::NewerSchema => {
                rusqlite::Connection::open(&path)
                    .unwrap()
                    .execute_batch("PRAGMA user_version = 99;")
                    .unwrap();
            }
        }
        reseal(&fx, &backup);
        assert_eq!(store.restore_backup(&backup.id).unwrap_err().code(), code);
    }
    assert_eq!(titles(&store), ["Synthetic live task"]);
    assert_eq!(
        store.restore_backup("../tasknotes").unwrap_err().code(),
        "not_found"
    );
}

enum Contents {
    Text,
    ForeignSqlite,
    NewerSchema,
}

/// Make a manifest match its (altered) snapshot again.
fn reseal(fx: &Fixture, backup: &Backup) {
    let bytes = bytes_of(&snapshot_path(fx, backup));
    let manifest_path = fx.backups_dir().join(format!("{}.json", backup.id));
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["sha256"] = sha256_hex(&bytes).into();
    manifest["sizeBytes"] = bytes.len().into();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
}

#[test]
fn a_pre_restore_backup_keeps_the_data_a_restore_replaced() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic before backup");
    let backup = store.create_backup().unwrap();
    task(&store, "Synthetic only in the live data");

    let outcome = store.restore_backup(&backup.id).unwrap();
    assert_eq!(outcome.pre_restore.kind, BackupKind::PreRestore);
    assert_eq!(titles(&store), ["Synthetic before backup"]);
    let listed = store.list_backups().unwrap();
    assert!(listed.contains(&outcome.pre_restore));
    assert!(listed.contains(&backup));

    // Restoring the Pre-restore backup undoes the restore.
    store.restore_backup(&outcome.pre_restore.id).unwrap();
    assert_eq!(
        titles(&store),
        ["Synthetic before backup", "Synthetic only in the live data"]
    );
}

#[test]
fn only_the_newest_pre_restore_backups_are_kept() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic task");
    let backup = store.create_backup().unwrap();
    let mut pre = Vec::new();
    for _ in 0..5 {
        pre.push(store.restore_backup(&backup.id).unwrap().pre_restore);
        // Distinct creation times.
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let kept: Vec<Backup> = store
        .list_backups()
        .unwrap()
        .into_iter()
        .filter(|b| b.kind == BackupKind::PreRestore)
        .collect();
    let newest: Vec<Backup> = pre.iter().rev().take(3).cloned().collect();
    assert_eq!(kept, newest);
}

#[test]
fn removed_and_private_items_survive_backup_and_restore() {
    let fx = Fixture::new();
    let store = fx.desktop();
    let removed_task = task(&store, "Synthetic task in Removed");
    store
        .set_task_removed(SetRemoved {
            id: removed_task.id.clone(),
            expected_revision: removed_task.revision,
            removed: true,
        })
        .unwrap();
    let private = store
        .create_note(CreateNote {
            markdown: "# Synthetic private note".into(),
            folder_id: None,
        })
        .unwrap();
    store
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let removed_note = store
        .create_note(CreateNote {
            markdown: "# Synthetic removed note".into(),
            folder_id: None,
        })
        .unwrap();
    store
        .set_note_removed(SetRemoved {
            id: removed_note.id.clone(),
            expected_revision: removed_note.revision,
            removed: true,
        })
        .unwrap();
    let backup = store.create_backup().unwrap();
    // Afterwards, restore everything to visible and public.
    let current = store.get_task(&removed_task.id).unwrap().unwrap();
    store
        .set_task_removed(SetRemoved {
            id: current.id.clone(),
            expected_revision: current.revision,
            removed: false,
        })
        .unwrap();

    store.restore_backup(&backup.id).unwrap();
    let restored_task = store.get_task(&removed_task.id).unwrap().unwrap();
    assert!(
        restored_task.removed,
        "still in Removed, and still recoverable"
    );
    store
        .set_task_removed(SetRemoved {
            id: restored_task.id.clone(),
            expected_revision: restored_task.revision,
            removed: false,
        })
        .unwrap();
    let note = store.get_note(&private.id).unwrap().unwrap();
    assert!(note.exclude_from_ai);
    assert!(fx.cli().get_note(&private.id).unwrap().is_none());
    assert!(store.get_note(&removed_note.id).unwrap().unwrap().removed);
}

#[test]
fn an_empty_database_can_be_backed_up_and_restored() {
    let fx = Fixture::new();
    let store = fx.desktop();
    let empty = store.create_backup().unwrap();
    task(&store, "Synthetic task to be rolled back");
    store.restore_backup(&empty.id).unwrap();
    assert!(titles(&store).is_empty());
    // Still a working database in WAL mode.
    task(&store, "Synthetic task after restore");
    let mode: String = rusqlite::Connection::open(fx.db())
        .unwrap()
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn a_backup_leaves_out_a_write_still_in_progress() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic committed task");
    // Another process is half-way through a write when the backup runs.
    let mut writer = rusqlite::Connection::open(fx.db()).unwrap();
    let pending = writer.transaction().unwrap();
    pending
        .execute("UPDATE tasks SET title = 'Synthetic uncommitted title'", [])
        .unwrap();
    let backup = store.create_backup().unwrap();
    drop(pending); // the writer is interrupted: its transaction rolls back

    task(&store, "Synthetic later task");
    store.restore_backup(&backup.id).unwrap();
    assert_eq!(titles(&store), ["Synthetic committed task"]);
}

#[test]
fn an_interrupted_backup_or_restore_leaves_nothing_that_looks_like_a_backup() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic task");
    let backup = store.create_backup().unwrap();
    // What a process killed mid-way leaves behind.
    let dir = fx.backups_dir();
    let partial = dir.join(".20261026T000000Z-daily-deadbeef.partial");
    let staging = dir.join(".restore-0000.sqlite3");
    std::fs::write(&partial, b"half a snapshot").unwrap();
    std::fs::write(&staging, b"half a staging copy").unwrap();
    // A snapshot whose manifest was never written.
    std::fs::write(dir.join("20261026T000001Z-daily-cafebabe.sqlite3"), b"x").unwrap();
    assert_eq!(store.list_backups().unwrap(), std::slice::from_ref(&backup));

    // Leftovers an hour old belong to no running process and are cleaned up.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
    for path in [&partial, &staging] {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    store.ensure_daily_backup().unwrap();
    assert!(!partial.exists() && !staging.exists());
    store.restore_backup(&backup.id).unwrap();
    assert_eq!(titles(&store), ["Synthetic task"]);
}

#[test]
fn a_restore_waits_for_a_writer_and_refuses_when_the_file_stays_locked() {
    let fx = Fixture::new();
    let store = fx.desktop();
    task(&store, "Synthetic backed-up task");
    let backup = store.create_backup().unwrap();
    task(&store, "Synthetic live task");

    // A short write by another process: the restore waits for it.
    let db = fx.db();
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let short = std::thread::spawn(move || {
        let mut writer = rusqlite::Connection::open(db).unwrap();
        let tx = writer
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        locked_tx.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        tx.commit().unwrap();
    });
    locked_rx.recv().unwrap();
    store.restore_backup(&backup.id).unwrap();
    short.join().unwrap();
    assert_eq!(titles(&store), ["Synthetic backed-up task"]);

    // A writer that never lets go: refused, nothing replaced.
    task(&store, "Synthetic live task again");
    let before = titles(&store);
    let mut stuck = rusqlite::Connection::open(fx.db()).unwrap();
    let hold = stuck
        .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
        .unwrap();
    let refused = store.restore_backup(&backup.id).unwrap_err();
    assert_eq!(refused.code(), "storage_unavailable");
    drop(hold);
    assert_eq!(titles(&store), before);
}

#[test]
fn writers_racing_a_restore_either_land_before_it_or_are_refused() {
    let fx = Fixture::new();
    let store = fx.desktop();
    let shared = task(&store, "Synthetic shared task");
    let backup = store.create_backup().unwrap();

    let db = fx.db();
    let writer = std::thread::spawn(move || {
        let other = TaskStore::open_as(&db, Interface::Cli).unwrap();
        let mut outcomes = Vec::new();
        for i in 0..200 {
            let result = other.create_task(CreateTask {
                title: format!("Synthetic racing task {i}"),
                description: String::new(),
            });
            outcomes.push(result.map(|t| t.id).map_err(|e| e.code()));
        }
        outcomes
    });
    std::thread::sleep(std::time::Duration::from_millis(20));
    store.restore_backup(&backup.id).unwrap();
    let outcomes = writer.join().unwrap();

    // Once refused, always refused: the stale store never writes again.
    let first_refusal = outcomes.iter().position(|o| o.is_err());
    if let Some(at) = first_refusal {
        assert!(
            outcomes[at..]
                .iter()
                .all(|o| *o == Err("database_restored"))
        );
    }
    // Nothing the stale writer did after the restore is in the data.
    let fresh = fx.cli();
    let landed: Vec<&String> = outcomes.iter().filter_map(|o| o.as_ref().ok()).collect();
    for id in &landed {
        assert!(
            fresh.get_task(id).unwrap().is_none(),
            "a write before the restore is replaced by the backup"
        );
    }
    assert!(fresh.get_task(&shared.id).unwrap().is_some());
}

#[test]
fn a_restarted_process_sees_the_restored_data_and_no_second_backup_that_day() {
    let fx = Fixture::new();
    {
        let first_run = fx.desktop();
        task(&first_run, "Synthetic task from the first run");
        assert!(first_run.ensure_daily_backup().unwrap().is_some());
    }
    let second_run = fx.desktop();
    assert!(second_run.ensure_daily_backup().unwrap().is_none());
    let backup = second_run.list_backups().unwrap().remove(0);
    task(&second_run, "Synthetic task from the second run");
    second_run.restore_backup(&backup.id).unwrap();
    drop(second_run);

    let third_run = fx.cli();
    assert_eq!(titles(&third_run), ["Synthetic task from the first run"]);
    task(&third_run, "Synthetic task from the third run");
}

#[test]
fn an_mcp_client_cannot_restore_but_the_cli_can() {
    let fx = Fixture::new();
    let desktop = fx.desktop();
    task(&desktop, "Synthetic task");
    let backup = desktop.create_backup().unwrap();
    let mcp = TaskStore::open_as(&fx.db(), Interface::Mcp).unwrap();
    assert_eq!(
        mcp.restore_backup(&backup.id).unwrap_err().code(),
        "desktop_only"
    );
    assert_eq!(mcp.list_backups().unwrap(), std::slice::from_ref(&backup));
    fx.cli().restore_backup(&backup.id).unwrap();
}

#[cfg(unix)]
#[test]
fn backups_are_private_to_the_persons_account() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    let store = fx.desktop();
    let backup = store.create_backup().unwrap();
    let mode = |p: PathBuf| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(fx.backups_dir()), 0o700);
    assert_eq!(mode(snapshot_path(&fx, &backup)), 0o600);
    assert_eq!(
        mode(fx.backups_dir().join(format!("{}.json", backup.id))),
        0o600
    );
}
