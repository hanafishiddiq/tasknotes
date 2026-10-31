//! Local Backups, validated restore and the Restore Epoch (Ticket 13).
//!
//! A **Backup** is a consistent SQLite snapshot (`VACUUM INTO`, taken inside
//! one read transaction) kept in `backups/` next to the database, outside
//! any source repository. Its **Backup Manifest** (`<id>.json`) records when
//! and why it was taken, the schema version, the size and a SHA-256 of the
//! snapshot. Both are written under hidden partial names, synced, and
//! renamed into place snapshot first, manifest last: only a snapshot with a
//! manifest is a Backup, and an orphan left by a stopped process is cleaned
//! up an hour later.
//!
//! **Retention**, by kind (each kind is its own bucket, so one never prunes
//! another): Daily backups, taken at most once per *local* calendar day
//! while any Tasknotes process (desktop, CLI, MCP) runs, follow the
//! 7 daily / 4 weekly / 3 monthly policy; the 10 newest Manual backups are
//! kept; Pre-restore backups are never removed automatically (the person
//! can manage the `backups` folder). Pruning removes backup *copies* only,
//! never live Tasks, Notes or Lists.
//!
//! **Restore** checks the snapshot against its manifest, copies it to a
//! staging file, runs `PRAGMA integrity_check` and `foreign_key_check`,
//! migrates it forward and compares its schema with a freshly created one.
//! Only then does it touch the live data, in this order:
//!
//! 1. **Fence**: under the write lock (`begin_write`), the live Restore
//!    Epoch is replaced by a fence token no store holds, so every other
//!    writer, in any process, is refused from that commit on.
//! 2. **Pre-restore backup** of the live data, which now holds every write
//!    that succeeded.
//! 3. **Carry over**: Audit Entries are append-only (ADR-0005), so every
//!    live entry the snapshot lacks is appended to the staging copy, with a
//!    `backup.restore` entry. Progress Entries are user state and come back
//!    as they were in the snapshot.
//! 4. **Swap**: the staging copy, with a fresh Restore Epoch, replaces the
//!    live database through SQLite's online backup API, under its write lock.
//!
//! **Restore Epoch**: a random token stored in the database; every store
//! remembers the token it opened. A store from before a restore is refused
//! with `DatabaseRestored` on every read and on every write transaction —
//! the write check runs after the write lock is taken. A restore holds
//! `backups/.restore.lock` while the fence is up; a store opened meanwhile
//! is refused with `RestoreInProgress`, and a fence left by a restore that
//! stopped (its lock is free, the live data unchanged) is lifted on open.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::audit::{AuditAction, Interface};
use crate::calendar::{DAY_MS, TimeZone, civil_from_days};
use crate::tasks::{
    BUSY_TIMEOUT, SCHEMA_VERSION, TaskError, TaskStore, current_millis, migrate, open_connection,
};

/// The directory, next to the database, that holds Backups.
pub const BACKUP_DIR: &str = "backups";
/// How many Manual backups are kept; they are outside the 7/4/3 policy.
pub const MANUAL_KEPT: usize = 10;
const MANIFEST_VERSION: u32 = 1;
/// Leftover partial, staging or orphan files older than this are from a
/// process that stopped mid-way and are cleaned up.
const ABANDONED_AFTER: Duration = Duration::from_secs(60 * 60);
/// Held by a restore while its fence is up.
const RESTORE_LOCK: &str = ".restore.lock";
/// The Restore Epoch while a restore fences writers. No store adopts it.
const FENCE_PREFIX: &str = "restoring-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// Taken automatically, at most once per local day, while Tasknotes runs.
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

/// A Backup just taken. The backup is complete even when retention then
/// failed to remove an old copy; the next backup tries again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBackup {
    pub backup: Backup,
    pub retention_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub restored: Backup,
    /// The live data as it was just before the restore.
    pub pre_restore: Backup,
}

/// When a backup is taken, and which wall clock decides its day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub now_ms: i64,
    pub zone: TimeZone,
}

impl Clock {
    /// Now, on the Mac's own time zone.
    pub fn system() -> Result<Self, TaskError> {
        Ok(Self {
            now_ms: current_millis()?,
            zone: TimeZone::Local,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    manifest_version: u32,
    #[serde(flatten)]
    backup: Backup,
}

/// Schema v15: the highest revision each row had when a restore replaced it, so
/// a number is never used for another version, even after the row left the data.
pub(crate) fn migrate_v15(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(
        "CREATE TABLE retired_revisions (
            entity_id TEXT PRIMARY KEY NOT NULL,
            revision INTEGER NOT NULL
         ) WITHOUT ROWID;
         PRAGMA user_version = 15;",
    )?;
    Ok(())
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

/// The Restore Epoch a store opening (or adopting) the database at `db`
/// may hold. While a restore fences writers it is refused; a fence whose
/// restore stopped is lifted with a fresh epoch (the live data was never
/// replaced: the swap is the restore's last step).
pub(crate) fn settled_epoch(db: &Path, conn: &Connection) -> Result<String, TaskError> {
    let epoch = read_epoch(conn)?;
    if !epoch.starts_with(FENCE_PREFIX) {
        return Ok(epoch);
    }
    let _lock = RestoreLock::acquire(&backups_dir_of(db))?;
    conn.execute(
        "UPDATE database_epoch SET epoch = ?1 WHERE id = 1 AND epoch = ?2",
        params![Uuid::new_v4().to_string(), epoch],
    )?;
    read_epoch(conn)
}

fn backups_dir_of(db: &Path) -> PathBuf {
    db.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(BACKUP_DIR)
}

impl TaskStore {
    /// Where this database's Backups are kept.
    pub fn backups_dir(&self) -> PathBuf {
        backups_dir_of(self.path())
    }

    /// Take a Manual backup now. Retention then applies to Manual backups
    /// only: a Manual backup never removes a Daily or Pre-restore one.
    pub fn create_backup(&self) -> Result<NewBackup, TaskError> {
        self.create_backup_at(Clock::system()?)
    }

    /// `create_backup` with the clock given, for tests.
    pub fn create_backup_at(&self, clock: Clock) -> Result<NewBackup, TaskError> {
        let backup = self.snapshot(&self.connect()?, BackupKind::Manual, clock.now_ms)?;
        Ok(NewBackup {
            backup,
            retention_error: self
                .prune(BackupKind::Manual, clock.zone)
                .err()
                .map(|e| e.to_string()),
        })
    }

    /// Take today's automatic backup unless a Daily backup already exists
    /// for this local calendar day, then apply the 7/4/3 policy to Daily
    /// backups. Every adapter calls it: the desktop at launch and hourly,
    /// each CLI command, and MCP at start-up and periodically. Two
    /// processes that race may both take one; retention keeps the newest
    /// of the day.
    pub fn ensure_daily_backup(&self) -> Result<Option<NewBackup>, TaskError> {
        self.ensure_daily_backup_at(Clock::system()?)
    }

    /// `ensure_daily_backup` with the clock given, for tests.
    pub fn ensure_daily_backup_at(&self, clock: Clock) -> Result<Option<NewBackup>, TaskError> {
        self.clean_abandoned();
        let today = clock.zone.day_of(clock.now_ms);
        let taken = self
            .list_backups()?
            .iter()
            .any(|b| b.kind == BackupKind::Daily && clock.zone.day_of(b.created_at) == today);
        if taken {
            self.prune(BackupKind::Daily, clock.zone)?;
            return Ok(None);
        }
        let backup = self.snapshot(&self.connect()?, BackupKind::Daily, clock.now_ms)?;
        Ok(Some(NewBackup {
            backup,
            retention_error: self
                .prune(BackupKind::Daily, clock.zone)
                .err()
                .map(|e| e.to_string()),
        }))
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

    /// Replace the live data with a validated Backup (see the module notes
    /// for the order of steps). Nothing live changes unless every check
    /// passes; a Pre-restore backup holds every write that succeeded before.
    /// Other stores opened before are then refused with `DatabaseRestored`;
    /// this one (and its clones) carries on with the restored data.
    ///
    /// Only an interface that `may_restore` can: the desktop, and the CLI,
    /// which asks the person at the terminal first. An MCP client cannot.
    pub fn restore_backup(&self, id: &str) -> Result<RestoreOutcome, TaskError> {
        if !self.interface().may_restore() {
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
        staging.validate()?;

        let _lock = RestoreLock::acquire(&dir)?;
        let held = self.epoch().clone();
        let fence = format!("{FENCE_PREFIX}{}", Uuid::new_v4().simple());
        self.fence_writers(&fence)?;
        let swapped = (|| {
            let pre_restore = self.snapshot(
                &open_connection(self.path())?,
                BackupKind::PreRestore,
                current_millis()?,
            )?;
            let epoch = staging.carry_over(self.path(), &restored.id, self.interface())?;
            self.replace_live_with(&staging.path)?;
            Ok((pre_restore, epoch))
        })();
        match swapped {
            Ok((pre_restore, epoch)) => {
                *self.epoch() = epoch;
                Ok(RestoreOutcome {
                    restored,
                    pre_restore,
                })
            }
            Err(error) => {
                // Nothing live was replaced: lift the fence so every store,
                // this one included, carries on. If even that fails, the
                // next store to open lifts it (the lock is released).
                let _ = open_connection(self.path()).and_then(|conn| {
                    conn.execute(
                        "UPDATE database_epoch SET epoch = ?1 WHERE id = 1 AND epoch = ?2",
                        params![held, fence],
                    )?;
                    Ok(())
                });
                Err(error)
            }
        }
    }

    /// Step 1 of a restore: from this commit on, every store's write
    /// transaction is refused, because none holds the fence token.
    fn fence_writers(&self, fence: &str) -> Result<(), TaskError> {
        let mut conn = self.connect()?;
        // A writer that keeps the lock past the busy timeout: say so plainly.
        let tx = self.begin_write(&mut conn).map_err(|error| match error {
            TaskError::Sqlite(rusqlite::Error::SqliteFailure(failure, _))
                if failure.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                TaskError::RestoreBusy
            }
            error => error,
        })?;
        tx.execute(
            "UPDATE database_epoch SET epoch = ?1 WHERE id = 1",
            params![fence],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Copy the staging file over the live database in one backup step,
    /// which holds SQLite's write lock on the live file for the whole copy;
    /// other connections see the old data or the new, never a mix.
    fn replace_live_with(&self, staging: &Path) -> Result<(), TaskError> {
        let source = Connection::open_with_flags(staging, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut live = open_connection(self.path())?;
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

    /// Write a consistent snapshot of `conn`'s database and its manifest.
    /// Both are written under hidden partial names and synced; the snapshot
    /// is renamed into place first and the manifest last, so a stop in
    /// between leaves an orphan snapshot (cleaned later), never a manifest
    /// pointing at nothing.
    fn snapshot(
        &self,
        conn: &Connection,
        kind: BackupKind,
        now_ms: i64,
    ) -> Result<Backup, TaskError> {
        let dir = self.backups_dir();
        create_private_dir(&dir)?;
        let id = backup_id(kind, now_ms);
        let partial = dir.join(format!(".{id}.partial"));
        let manifest_partial = dir.join(format!(".{}.partial", manifest_name(&id)));
        // VACUUM INTO accepts an existing empty file, which lets the
        // snapshot be private (0600) from its first byte.
        drop(private_file(&partial, true)?);
        let result = (|| {
            // VACUUM INTO only reads this database, but SQLite counts it as
            // a write, which a store's read-only connection refuses.
            let read_only: bool = conn.pragma_query_value(None, "query_only", |r| r.get(0))?;
            conn.pragma_update(None, "query_only", false)?;
            let vacuumed = conn.execute("VACUUM INTO ?1", params![partial.to_string_lossy()]);
            conn.pragma_update(None, "query_only", read_only)?;
            vacuumed?;
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
            write_manifest(&manifest_partial, &backup)?;
            fs::rename(&partial, dir.join(snapshot_name(&id)))?;
            fs::rename(&manifest_partial, dir.join(manifest_name(&id)))?;
            sync_dir(&dir)?;
            Ok(backup)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&partial);
            let _ = fs::remove_file(&manifest_partial);
        }
        result
    }

    /// Apply one kind's retention: 7 daily / 4 weekly / 3 monthly for Daily
    /// backups, the newest `MANUAL_KEPT` for Manual ones; Pre-restore
    /// backups are never removed automatically.
    fn prune(&self, kind: BackupKind, zone: TimeZone) -> Result<(), TaskError> {
        let bucket: Vec<Backup> = self
            .list_backups()?
            .into_iter()
            .filter(|b| b.kind == kind)
            .collect();
        let keep = match kind {
            BackupKind::Daily => {
                let times: Vec<i64> = bucket.iter().map(|b| b.created_at).collect();
                retained(&times, zone)
            }
            // `list_backups` is newest first.
            BackupKind::Manual => (0..bucket.len()).map(|i| i < MANUAL_KEPT).collect(),
            BackupKind::PreRestore => return Ok(()),
        };
        let dir = self.backups_dir();
        for (backup, keep) in bucket.iter().zip(keep) {
            if !keep {
                // The manifest goes first: a backup without one is not listed.
                remove_if_present(&dir.join(manifest_name(&backup.id)))?;
                remove_if_present(&dir.join(snapshot_name(&backup.id)))?;
            }
        }
        Ok(())
    }

    /// Best effort: remove partial snapshots and manifests, staging files
    /// and orphan snapshots (no manifest) a stopped process left behind.
    /// Recent ones may belong to a running process and are left alone.
    fn clean_abandoned(&self) {
        let dir = self.backups_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let leftover = name.starts_with('.')
                && (name.ends_with(".partial") || name.starts_with(".restore-"));
            let orphan = name
                .strip_suffix(".sqlite3")
                .is_some_and(|id| valid_id(id) && !dir.join(manifest_name(id)).exists());
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age >= ABANDONED_AFTER);
            if (leftover || orphan) && old {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// `backups/.restore.lock`, held (an OS file lock, released when the
/// process ends however it ends) while a restore fences writers.
struct RestoreLock {
    _file: File,
}

impl RestoreLock {
    fn acquire(dir: &Path) -> Result<Self, TaskError> {
        create_private_dir(dir)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(dir.join(RESTORE_LOCK))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(TaskError::RestoreInProgress),
            Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
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
    /// can read, and migrate it forward. A file is a Tasknotes database
    /// only if, once migrated, it has exactly the tables, columns, indexes
    /// and triggers a freshly created one has: a `user_version` alone
    /// proves nothing.
    fn validate(&self) -> Result<(), TaskError> {
        let foreign = || TaskError::InvalidBackup("it is not a Tasknotes database");
        {
            let conn = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(|_| foreign())?;
            let version: i64 = conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .map_err(|_| foreign())?;
            if version > SCHEMA_VERSION {
                return Err(TaskError::UnsupportedSchema(version));
            }
            if version < 1 {
                return Err(foreign());
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
        let mut conn = open_connection(&self.path)?;
        migrate(&mut conn).map_err(|_| TaskError::InvalidBackup("it could not be upgraded"))?;
        let mut reference = Connection::open_in_memory()?;
        migrate(&mut reference)?;
        if schema_of(&conn).map_err(|_| foreign())? != schema_of(&reference)? {
            return Err(foreign());
        }
        Ok(())
    }

    /// What a restore must not undo, in the copy with live data attached and
    /// every Audit Entry already carried over. A row can have left at one
    /// restore and come back at this one, so nothing live remembers it.
    fn keep_what_must_survive(tx: &Transaction) -> Result<(), TaskError> {
        // A client still holding a revision must not overwrite what the person
        // writes next, so a number never names two versions of a row. Whatever
        // this restore replaces retires its number, in a table that outlives
        // restores and holds only numbers really issued: never Audit Entries,
        // where a refused write records whatever the client claimed. A row
        // older than its retired number gets a new one; the newest version keeps
        // its own, so a request waiting on it stays valid.
        tx.execute(
            "INSERT INTO main.retired_revisions (entity_id, revision)
             SELECT entity_id, revision FROM live.retired_revisions WHERE true
             ON CONFLICT (entity_id) DO UPDATE
               SET revision = max(revision, excluded.revision)",
            [],
        )?;
        for table in ["tasks", "notes", "lists", "note_folders"] {
            tx.execute(
                &format!(
                    "INSERT INTO main.retired_revisions (entity_id, revision)
                     SELECT l.id, l.revision FROM live.{table} AS l
                     LEFT JOIN main.{table} AS m ON m.id = l.id
                     WHERE m.id IS NULL OR m.revision <> l.revision
                     ON CONFLICT (entity_id) DO UPDATE
                       SET revision = max(revision, excluded.revision)"
                ),
                [],
            )?;
            let retired = format!(
                "(SELECT r.revision FROM main.retired_revisions AS r WHERE r.entity_id = {table}.id)"
            );
            tx.execute(
                &format!(
                    "UPDATE main.{table} SET revision = 1 + {retired} WHERE revision < {retired}"
                ),
                [],
            )?;
        }
        // Owner decision, 2026-10-30: a restore can never open a Note to AI. A
        // Note private now stays private. One absent from live may have been
        // made private before it left; the entries do not say which way the
        // last change went, so any change of privacy keeps it private, and the
        // person can share it again.
        tx.execute(
            "UPDATE main.notes SET exclude_from_ai = 1
             WHERE id IN (SELECT id FROM live.notes WHERE exclude_from_ai = 1)
                OR (id NOT IN (SELECT id FROM live.notes)
                    AND id IN (SELECT entity_id FROM main.audit_entries WHERE action = ?1))",
            params![AuditAction::NotePrivacy.as_str()],
        )?;
        // Owner decision, 2026-10-30: a person's decision stands. From the live
        // request when there is one, else from its last Audit Entry.
        tx.execute(
            "UPDATE main.bulk_requests
             SET (status, decided_at) = (SELECT l.status, l.decided_at
                                         FROM live.bulk_requests AS l
                                         WHERE l.id = bulk_requests.id)
             WHERE status = 'pending'
               AND EXISTS (SELECT 1 FROM live.bulk_requests AS l
                           WHERE l.id = bulk_requests.id AND l.status <> 'pending')",
            [],
        )?;
        tx.execute(
            "UPDATE main.bulk_requests
             SET (status, decided_at) = (
               SELECT CASE WHEN a.action = ?1 THEN 'declined'
                           WHEN a.outcome = 'applied' THEN 'applied'
                           ELSE 'stale' END,
                      a.created_at
               FROM main.audit_entries AS a
               WHERE a.entity_id = bulk_requests.id AND a.action IN (?1, ?2)
                 AND a.interface = 'desktop'
               ORDER BY a.sequence DESC LIMIT 1)
             WHERE status = 'pending'
               AND EXISTS (SELECT 1 FROM main.audit_entries AS a
                           WHERE a.entity_id = bulk_requests.id AND a.action IN (?1, ?2)
                             AND a.interface = 'desktop')",
            params![
                AuditAction::BulkDecline.as_str(),
                AuditAction::BulkApprove.as_str()
            ],
        )?;
        Ok(())
    }

    /// Step 3 of a restore, once writers are fenced: append every live
    /// Audit Entry the snapshot lacks (in append order, with its id, and
    /// hidden from AI if it is hidden from AI now) and a `backup.restore`
    /// entry, keep what must survive (`keep_what_must_survive`), give the copy a
    /// fresh Restore Epoch and fold its log into the file. Returns the
    /// epoch.
    fn carry_over(
        &self,
        live: &Path,
        backup_id: &str,
        interface: Interface,
    ) -> Result<String, TaskError> {
        let mut conn = open_connection(&self.path)?;
        conn.execute(
            "ATTACH DATABASE ?1 AS live",
            params![live.to_string_lossy()],
        )?;
        let epoch = Uuid::new_v4().to_string();
        let tx = conn.transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO main.audit_entries
                   (id, created_at, interface, action, entity_id, outcome, error_code,
                    revision, requested_by, hidden_from_ai)
                 SELECT id, created_at, interface, action, entity_id, outcome, error_code,
                        revision, requested_by, {hidden}
                 FROM live.audit_entries AS l
                 WHERE NOT EXISTS (SELECT 1 FROM main.audit_entries AS m WHERE m.id = l.id)
                 ORDER BY l.sequence",
                hidden = crate::privacy::hidden_now_sql(
                    "live",
                    "l.hidden_from_ai",
                    "l.action",
                    "l.entity_id"
                ),
            ),
            [],
        )?;
        // Owner decisions, 2026-10-30. A restore can never open a Note to AI:
        // one private now stays private. And a person's decision stands: a
        // request decided since the snapshot does not come back as waiting.
        Self::keep_what_must_survive(&tx)?;
        crate::audit::record_restore(&tx, interface, backup_id)?;
        tx.execute(
            "UPDATE main.database_epoch SET epoch = ?1 WHERE id = 1",
            params![epoch],
        )?;
        tx.commit()?;
        conn.execute_batch("DETACH DATABASE live;")?;
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

/// Each schema object with, for a table, its column names in order.
fn schema_of(conn: &Connection) -> Result<Vec<(String, String, String)>, TaskError> {
    let mut statement = conn.prepare(
        "SELECT m.type, m.name,
                coalesce((SELECT group_concat(c.name, ',') FROM pragma_table_xinfo(m.name) AS c), '')
         FROM sqlite_master AS m
         WHERE m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
         ORDER BY m.type, m.name",
    )?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Maps a day (counted from 1970-01-01) to the period it belongs to.
type PeriodOf = fn(i64) -> i64;

/// Which of the backups (created-at times, any order) the 7 daily /
/// 4 weekly / 3 monthly policy keeps. Each rule keeps the newest backup of
/// each of its most recent periods that have a backup; a backup kept by
/// any rule stays. Days, weeks (Monday to Sunday) and months are those of
/// the wall clock `zone`, so a backup at 00:30 local time belongs to that
/// local day even when it is still the previous day in UTC.
pub(crate) fn retained(created_at: &[i64], zone: TimeZone) -> Vec<bool> {
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
            let period = period_of(zone.day_of(created_at[i]));
            if kept < count && last != Some(period) {
                keep[i] = true;
                kept += 1;
            }
            last = Some(period);
        }
    }
    keep
}

/// `20261026T101500Z-daily-0a1b2c3d`: the UTC time it was taken, so ids
/// sort by time whatever the time zone.
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

fn manifest_name(id: &str) -> String {
    format!("{id}.json")
}

fn read_manifest(dir: &Path, id: &str) -> Option<Backup> {
    let text = fs::read_to_string(dir.join(manifest_name(id))).ok()?;
    let manifest: Manifest = serde_json::from_str(&text).ok()?;
    (manifest.manifest_version == MANIFEST_VERSION
        && manifest.backup.id == id
        && dir.join(snapshot_name(id)).is_file())
    .then_some(manifest.backup)
}

/// Write a manifest to its (hidden, partial) path and sync it; the caller
/// renames it into place.
fn write_manifest(partial: &Path, backup: &Backup) -> Result<(), TaskError> {
    let manifest = Manifest {
        manifest_version: MANIFEST_VERSION,
        backup: backup.clone(),
    };
    let mut file = private_file(partial, true)?;
    file.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(std::io::Error::other)?
            .as_bytes(),
    )?;
    file.sync_all()?;
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

pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// A file only the person's account can read. `fresh` refuses an existing one.
pub(crate) fn private_file(path: &Path, fresh: bool) -> std::io::Result<File> {
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

/// Make the renames in `dir` durable. A file system that cannot sync a
/// directory at all says so with `InvalidInput` (EINVAL), which is not a
/// failure of this write; every other error is reported.
pub(crate) fn sync_dir(dir: &Path) -> std::io::Result<()> {
    match File::open(dir)?.sync_all() {
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
        result => result,
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
    fn several_backups_in_one_day_keep_only_the_newest_for_that_day() {
        let monday = day_of(2026, 10, 26) * DAY;
        let keep = retained(
            &[monday + 1_000, monday + 9_000, monday + 5_000],
            TimeZone::UTC,
        );
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
        assert_eq!(retained(&times, TimeZone::UTC), [true, true, true]);
    }

    /// The Restore Epoch protects a write only when it is checked under the
    /// write lock (`begin_write`). A store's connection refuses every other
    /// write, so a deferred `transaction()` or an autocommit statement that
    /// skips `begin_write` fails in every test that reaches it.
    #[test]
    fn a_store_connection_writes_only_through_begin_write() {
        let dir = tempfile::tempdir().unwrap();
        let store = TaskStore::open(&dir.path().join("tasknotes.db")).unwrap();
        let insert = "INSERT INTO lists (id, name, revision, created_at, updated_at)
                      VALUES (lower(hex(randomblob(8))), 'Synthetic', 1, 0, 0)";
        let mut conn = store.connect().unwrap();
        assert!(conn.execute(insert, []).is_err(), "autocommit write");
        {
            let deferred = conn.transaction().unwrap();
            assert!(
                deferred.execute(insert, []).is_err(),
                "deferred transaction"
            );
        }
        // Reads and snapshots still work.
        let lists: i64 = conn
            .query_row("SELECT count(*) FROM lists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(lists, 0);
        store.create_backup().unwrap();
        let tx = store.begin_write(&mut conn).unwrap();
        tx.execute(insert, []).unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn ids_cannot_name_a_path() {
        assert!(valid_id("20261026T101500Z-daily-0a1b2c3d"));
        for bad in ["", "../tasknotes", "a/b", ".hidden", "x.sqlite3"] {
            assert!(!valid_id(bad), "{bad}");
        }
    }
}
