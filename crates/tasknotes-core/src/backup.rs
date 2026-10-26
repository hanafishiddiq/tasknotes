//! Local Backups, validated restore and the Restore Epoch (Ticket 13).
//!
//! A **Backup** is a consistent SQLite snapshot (`VACUUM INTO`, taken inside
//! one read transaction) kept in `backups/` next to the database, outside
//! any source repository. Its **Backup Manifest** (`<id>.json`) records when
//! and why it was taken, the schema version, the size and a SHA-256 of the
//! snapshot. A snapshot is written under a hidden partial name and renamed
//! only once complete, so an interrupted backup never looks like a backup.
//!
//! Automatic backups are taken at most once per UTC day while any Tasknotes
//! process (desktop, CLI, MCP) runs; Daily and Manual backups share the
//! 7 daily / 4 weekly / 3 monthly retention. Pruning removes backup *copies*
//! only, never live Tasks, Notes or Lists.
//!
//! **Restore** checks the snapshot against its manifest, copies it to a
//! staging file, runs `PRAGMA integrity_check` and `foreign_key_check`,
//! migrates it forward, takes a Pre-restore backup of the live data and
//! only then copies the staging file over the live database with SQLite's
//! online backup API, under SQLite's own write lock.
//!
//! **Restore Epoch**: a random token stored in the database. Each restore
//! writes a fresh one into the restored data, and every store remembers the
//! token it opened. A store from before the restore (another window, a CLI
//! or MCP process) is refused with `DatabaseRestored` on every read and on
//! every write transaction — the write check runs after the write lock is
//! taken — so a stale revision can never land on restored data.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::audit::Interface;
use crate::tasks::{BUSY_TIMEOUT, SCHEMA_VERSION, TaskError, TaskStore, current_millis};

/// The directory, next to the database, that holds Backups.
pub const BACKUP_DIR: &str = "backups";
const MANIFEST_VERSION: u32 = 1;
const DAY_MS: i64 = 86_400_000;
/// How many Pre-restore backups are kept; they are outside the 7/4/3 policy.
pub const PRE_RESTORE_KEPT: usize = 3;
/// Leftover partial or staging files older than this are from a process
/// that stopped mid-way and are cleaned up.
const ABANDONED_AFTER: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// Taken automatically, at most once per day, while Tasknotes runs.
    Daily,
    /// Asked for by the person or a trusted local client.
    Manual,
    /// The live data just before a restore replaced it.
    PreRestore,
}

impl BackupKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Manual => "manual",
            Self::PreRestore => "pre-restore",
        }
    }
}

/// A Backup as its manifest describes it. Holds no user content, so it is
/// safe to show to an AI-facing interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: String,
    pub kind: BackupKind,
    /// Unix milliseconds.
    pub created_at: i64,
    pub schema_version: i64,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub restored: Backup,
    /// The live data as it was just before the restore.
    pub pre_restore: Backup,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    manifest_version: u32,
    #[serde(flatten)]
    backup: Backup,
}

/// Schema v13: the Restore Epoch.
pub(crate) fn migrate_v13(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE IF NOT EXISTS database_epoch (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            epoch TEXT NOT NULL CHECK(length(epoch) BETWEEN 1 AND 64)
         );",
    )?;
    migration.execute(
        "INSERT OR IGNORE INTO database_epoch (id, epoch) VALUES (1, ?1)",
        params![Uuid::new_v4().to_string()],
    )?;
    migration.execute_batch("PRAGMA user_version = 13;")?;
    Ok(())
}

pub(crate) fn read_epoch(conn: &Connection) -> Result<String, TaskError> {
    Ok(
        conn.query_row("SELECT epoch FROM database_epoch WHERE id = 1", [], |row| {
            row.get(0)
        })?,
    )
}

impl TaskStore {
    /// Where this database's Backups are kept.
    pub fn backups_dir(&self) -> PathBuf {
        self.path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(BACKUP_DIR)
    }

    /// Take a Manual backup now, then apply the retention policy.
    pub fn create_backup(&self) -> Result<Backup, TaskError> {
        let backup = self.snapshot(BackupKind::Manual, current_millis()?)?;
        self.prune(&self.list_backups()?)?;
        Ok(backup)
    }

    /// Take today's automatic backup unless one (Daily or Manual) already
    /// exists for this UTC day; always applies retention. Safe to call from
    /// every process: two that race may both take one, and retention keeps
    /// the newest of the day.
    pub fn ensure_daily_backup(&self) -> Result<Option<Backup>, TaskError> {
        self.ensure_daily_backup_at(current_millis()?)
    }

    /// `ensure_daily_backup` with the clock given, for tests.
    pub fn ensure_daily_backup_at(&self, now_ms: i64) -> Result<Option<Backup>, TaskError> {
        self.clean_abandoned();
        let today = now_ms.div_euclid(DAY_MS);
        let existing = self.list_backups()?;
        let taken = existing
            .iter()
            .any(|b| b.kind != BackupKind::PreRestore && b.created_at.div_euclid(DAY_MS) == today);
        let created = if taken {
            None
        } else {
            Some(self.snapshot(BackupKind::Daily, now_ms)?)
        };
        self.prune(&self.list_backups()?)?;
        Ok(created)
    }

    /// Every complete Backup with a readable manifest, newest first.
    pub fn list_backups(&self) -> Result<Vec<Backup>, TaskError> {
        let dir = self.backups_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut backups = Vec::new();
        for entry in entries {
            let name = entry?.file_name();
            let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            if valid_id(id)
                && let Some(backup) = read_manifest(&dir, id)
            {
                backups.push(backup);
            }
        }
        backups.sort_by(|a, b| (b.created_at, &b.id).cmp(&(a.created_at, &a.id)));
        Ok(backups)
    }

    /// Replace the live data with a validated Backup. Nothing live changes
    /// unless every check passes; a Pre-restore backup is kept first. Other
    /// stores opened before are then refused with `DatabaseRestored`; this
    /// one carries on with the restored data.
    ///
    /// Only the desktop and the CLI may restore; the CLI asks the person at
    /// the terminal first. An MCP client cannot.
    pub fn restore_backup(&self, id: &str) -> Result<RestoreOutcome, TaskError> {
        if self.interface() == Interface::Mcp {
            return Err(TaskError::RestoreNotAllowed);
        }
        // Refuse a store that is already stale before touching anything.
        drop(self.connect()?);
        let dir = self.backups_dir();
        let restored = valid_id(id)
            .then(|| read_manifest(&dir, id))
            .flatten()
            .ok_or(TaskError::BackupNotFound)?;
        verify_file(&dir.join(snapshot_name(id)), &restored)?;

        let staging = Staging::copy(&dir, &dir.join(snapshot_name(id)))?;
        let epoch = staging.validate_and_prepare()?;

        let pre_restore = self.snapshot(BackupKind::PreRestore, current_millis()?)?;
        self.replace_live_with(&staging.path)?;
        *self.epoch() = epoch;
        drop(staging);
        self.prune(&self.list_backups()?)?;
        Ok(RestoreOutcome {
            restored,
            pre_restore,
        })
    }

    /// Copy the staging file over the live database in one backup step,
    /// which holds SQLite's write lock on the live file for the whole copy;
    /// other connections see the old data or the new, never a mix.
    fn replace_live_with(&self, staging: &Path) -> Result<(), TaskError> {
        let source = Connection::open_with_flags(staging, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut live = Connection::open(self.path())?;
        live.busy_timeout(BUSY_TIMEOUT)?;
        let copy = rusqlite::backup::Backup::new(&source, &mut live)?;
        let started = Instant::now();
        loop {
            match copy.step(-1)? {
                rusqlite::backup::StepResult::Done => return Ok(()),
                rusqlite::backup::StepResult::More => {}
                _ if started.elapsed() >= BUSY_TIMEOUT => return Err(TaskError::RestoreBusy),
                _ => std::thread::sleep(Duration::from_millis(25)),
            }
        }
    }

    /// Write a consistent snapshot and its manifest. The snapshot is taken
    /// under a hidden partial name and renamed once complete and synced.
    fn snapshot(&self, kind: BackupKind, now_ms: i64) -> Result<Backup, TaskError> {
        let conn = self.connect()?;
        let dir = self.backups_dir();
        create_private_dir(&dir)?;
        let id = backup_id(kind, now_ms);
        let partial = dir.join(format!(".{id}.partial"));
        // VACUUM INTO accepts an existing empty file, which lets the
        // snapshot be private (0600) from its first byte.
        drop(private_file(&partial, true)?);
        let result = (|| {
            conn.execute("VACUUM INTO ?1", params![partial.to_string_lossy()])?;
            File::open(&partial)?.sync_all()?;
            let (size_bytes, sha256) = digest(&partial)?;
            let schema_version: i64 =
                conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
            let backup = Backup {
                id: id.clone(),
                kind,
                created_at: now_ms,
                schema_version,
                size_bytes,
                sha256,
            };
            fs::rename(&partial, dir.join(snapshot_name(&id)))?;
            write_manifest(&dir, &backup)?;
            sync_dir(&dir);
            Ok(backup)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&partial);
        }
        result
    }

    /// Apply the 7 daily / 4 weekly / 3 monthly policy to Daily and Manual
    /// backups and keep the newest Pre-restore backups.
    fn prune(&self, backups: &[Backup]) -> Result<(), TaskError> {
        let regular: Vec<&Backup> = backups
            .iter()
            .filter(|b| b.kind != BackupKind::PreRestore)
            .collect();
        let times: Vec<i64> = regular.iter().map(|b| b.created_at).collect();
        let keep = retained(&times);
        let mut doomed: Vec<&Backup> = regular
            .iter()
            .zip(keep)
            .filter(|(_, keep)| !keep)
            .map(|(b, _)| *b)
            .collect();
        doomed.extend(
            backups
                .iter()
                .filter(|b| b.kind == BackupKind::PreRestore)
                .skip(PRE_RESTORE_KEPT),
        );
        let dir = self.backups_dir();
        for backup in doomed {
            // The manifest goes first: a backup without one is not listed.
            remove_if_present(&dir.join(format!("{}.json", backup.id)))?;
            remove_if_present(&dir.join(snapshot_name(&backup.id)))?;
        }
        Ok(())
    }

    /// Best effort: remove partial snapshots and staging files a stopped
    /// process left behind. Recent ones may belong to a running process.
    fn clean_abandoned(&self) {
        let Ok(entries) = fs::read_dir(self.backups_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let leftover = name.starts_with('.')
                && (name.ends_with(".partial") || name.starts_with(".restore-"));
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age >= ABANDONED_AFTER);
            if leftover && old {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// A private copy of a snapshot that restore checks and prepares; removed
/// (with its journal files) however restore ends.
struct Staging {
    path: PathBuf,
}

impl Staging {
    fn copy(dir: &Path, snapshot: &Path) -> Result<Self, TaskError> {
        let path = dir.join(format!(".restore-{}.sqlite3", Uuid::new_v4()));
        let staging = Self { path };
        let mut target = private_file(&staging.path, false)?;
        std::io::copy(&mut File::open(snapshot)?, &mut target)?;
        target.sync_all()?;
        Ok(staging)
    }

    /// Check the copy is a whole, consistent Tasknotes database this core
    /// can read, migrate it forward and give it a fresh Restore Epoch.
    fn validate_and_prepare(&self) -> Result<String, TaskError> {
        {
            let conn = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(|_| TaskError::InvalidBackup("it is not a Tasknotes database"))?;
            let version: i64 = conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .map_err(|_| TaskError::InvalidBackup("it is not a Tasknotes database"))?;
            if version > SCHEMA_VERSION {
                return Err(TaskError::UnsupportedSchema(version));
            }
            if version < 1 {
                return Err(TaskError::InvalidBackup("it is not a Tasknotes database"));
            }
            let damaged = || TaskError::InvalidBackup("the snapshot is damaged");
            let mut check = conn
                .prepare("PRAGMA integrity_check")
                .map_err(|_| damaged())?;
            let report: Vec<String> = check
                .query_map([], |row| row.get(0))
                .and_then(|rows| rows.collect())
                .map_err(|_| damaged())?;
            if report != ["ok"] {
                return Err(damaged());
            }
            let broken_reference: bool = conn
                .query_row("PRAGMA foreign_key_check", [], |_| Ok(true))
                .optional()
                .map_err(|_| damaged())?
                .unwrap_or(false);
            if broken_reference {
                return Err(damaged());
            }
        }
        // Forward-only migrations, exactly as on launch.
        drop(
            TaskStore::open(&self.path)
                .map_err(|_| TaskError::InvalidBackup("it could not be upgraded"))?,
        );
        let conn = Connection::open(&self.path)?;
        let epoch = Uuid::new_v4().to_string();
        conn.execute(
            "UPDATE database_epoch SET epoch = ?1 WHERE id = 1",
            params![epoch],
        )?;
        // Fold the write-ahead log into the file before it is copied.
        conn.pragma_update_and_check(None, "journal_mode", "DELETE", |_| Ok(()))?;
        Ok(epoch)
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.path.clone().into_os_string();
            path.push(suffix);
            let _ = fs::remove_file(path);
        }
    }
}

/// Maps a day (counted from 1970-01-01) to the period it belongs to.
type PeriodOf = fn(i64) -> i64;

/// Which of the backups (created-at times, any order) the 7 daily /
/// 4 weekly / 3 monthly policy keeps. Each rule keeps the newest backup of
/// each of its most recent periods that have a backup; a backup kept by
/// any rule stays. Days and weeks (Monday to Sunday) are UTC.
pub(crate) fn retained(created_at: &[i64]) -> Vec<bool> {
    let mut order: Vec<usize> = (0..created_at.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(created_at[i]));
    let mut keep = vec![false; created_at.len()];
    let rules: [(usize, PeriodOf); 3] = [
        (7, |day| day),
        (4, |day| (day + 3).div_euclid(7)),
        (3, |day| {
            let (year, month, _) = civil_from_days(day);
            year * 12 + month
        }),
    ];
    for (count, period_of) in rules {
        let mut last = None;
        let mut kept = 0;
        for &i in &order {
            let period = period_of(created_at[i].div_euclid(DAY_MS));
            if kept < count && last != Some(period) {
                keep[i] = true;
                kept += 1;
            }
            last = Some(period);
        }
    }
    keep
}

/// (year, month 1–12, day 1–31) of a day counted from 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn backup_id(kind: BackupKind, now_ms: i64) -> String {
    let (year, month, day) = civil_from_days(now_ms.div_euclid(DAY_MS));
    let seconds = now_ms.rem_euclid(DAY_MS) / 1000;
    let suffix = &Uuid::new_v4().simple().to_string()[..8];
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z-{}-{suffix}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        kind.as_str()
    )
}

/// Ids are made here; anything else (a path, `..`) is not a backup.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn snapshot_name(id: &str) -> String {
    format!("{id}.sqlite3")
}

fn read_manifest(dir: &Path, id: &str) -> Option<Backup> {
    let text = fs::read_to_string(dir.join(format!("{id}.json"))).ok()?;
    let manifest: Manifest = serde_json::from_str(&text).ok()?;
    (manifest.manifest_version == MANIFEST_VERSION
        && manifest.backup.id == id
        && dir.join(snapshot_name(id)).is_file())
    .then_some(manifest.backup)
}

fn write_manifest(dir: &Path, backup: &Backup) -> Result<(), TaskError> {
    let manifest = Manifest {
        manifest_version: MANIFEST_VERSION,
        backup: backup.clone(),
    };
    let temporary = dir.join(format!(".{}.json.partial", backup.id));
    let mut file = private_file(&temporary, true)?;
    file.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(std::io::Error::other)?
            .as_bytes(),
    )?;
    file.sync_all()?;
    fs::rename(&temporary, dir.join(format!("{}.json", backup.id)))?;
    Ok(())
}

/// The snapshot must be exactly the file its manifest describes.
fn verify_file(path: &Path, backup: &Backup) -> Result<(), TaskError> {
    let mismatch = TaskError::InvalidBackup("the snapshot is incomplete or damaged");
    let Ok((size, sha256)) = digest(path) else {
        return Err(mismatch);
    };
    if size != backup.size_bytes || sha256 != backup.sha256 {
        return Err(mismatch);
    }
    Ok(())
}

fn digest(path: &Path) -> std::io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let size = std::io::copy(&mut File::open(path)?, &mut hasher)?;
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((size, hex))
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// A file only the person's account can read. `fresh` refuses an existing one.
fn private_file(path: &Path, fresh: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if fresh {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Make the renames durable; not every system can sync a directory.
fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = DAY_MS;

    fn day_of(year: i64, month: i64, day: i64) -> i64 {
        (0..40_000)
            .find(|&d| civil_from_days(d) == (year, month, day))
            .unwrap()
    }

    #[test]
    fn calendar_dates_are_right() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_752), (2026, 10, 26));
        assert_eq!(civil_from_days(day_of(2024, 2, 29)), (2024, 2, 29));
    }

    #[test]
    fn several_backups_in_one_day_keep_only_the_newest_for_that_day() {
        let monday = day_of(2026, 10, 26) * DAY;
        let keep = retained(&[monday + 1_000, monday + 9_000, monday + 5_000]);
        assert_eq!(keep, [false, true, false]);
    }

    #[test]
    fn a_long_gap_keeps_older_backups_rather_than_none() {
        // Tasknotes unused for months: the policy counts periods that have a
        // backup, so the old ones are not all pruned at once.
        let times: Vec<i64> = [(2025, 1, 5), (2025, 3, 9), (2026, 10, 26)]
            .iter()
            .map(|&(y, m, d)| day_of(y, m, d) * DAY)
            .collect();
        assert_eq!(retained(&times), [true, true, true]);
    }

    #[test]
    fn ids_cannot_name_a_path() {
        assert!(valid_id("20261026T101500Z-daily-0a1b2c3d"));
        for bad in ["", "../tasknotes", "a/b", ".hidden", "x.sqlite3"] {
            assert!(!valid_id(bad), "{bad}");
        }
    }
}
