# Ticket 13 — Backup, validated restore, and export: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only, never the owner's real data directory). Built on `integration/tasknotes-v1-07-18` at a05b6ec (Tickets 07–11, 14–16; schema v12). Ticket 12 (MCP) is built in parallel and takes schema v14; this ticket uses v13.

## Scope

Automatic local backups with the 7 daily / 4 weekly / 3 monthly policy, a validated restore that no stale writer (window, CLI, MCP) can undo, and JSON/Markdown export that respects Exclude from AI. Surfaces: core operations, `tasknotes backup …` / `tasknotes export …`, and a small "Backups and export" section in desktop Settings.

## Test seams (pre-agreed in the spec, Q48)

1. **Core interface** against temporary SQLite files (seam 1 and seam 5, data safety): `tests/backup_restore.rs`, `tests/export.rs`.
2. **Adapter contract**: the built `tasknotes` binary as a separate process, plus the library `run` for the terminal-confirmed restore (a test cannot own a TTY): `crates/tasknotes-cli/tests/backup.rs`.
3. **Desktop**: the Settings section and the invoke wrapper behind the mocked Tauri seam: `src/data-settings.test.tsx`. Native GUI acceptance stays open (owner check).

## Decisions made by the implementer (owner may revisit)

- **Snapshot = `VACUUM INTO`.** Saya pilih ini karena ia mengambil snapshot konsisten di dalam satu read transaction WAL (writer lain tetap jalan, tulisan yang belum commit tidak ikut), hasilnya file ringkas, dan tidak butuh koneksi tujuan. Written to a hidden `.<id>.partial` (pre-created 0600), synced, then renamed; a manifest `<id>.json` (written tmp+rename) records `manifest_version`, `id`, `kind`, `created_at`, `schema_version`, `size_bytes`, `sha256`. A snapshot without a valid manifest is never listed. No user text in a manifest, so listing backups is safe for AI interfaces.
- **Location: `backups/` next to `tasknotes.db`** in the data directory (dir 0700, files 0600), outside any Git checkout.
- **When: at most one automatic backup per UTC day, while any process runs.** Desktop: at launch, then an hourly check on a background thread. CLI: every data command calls `ensure_daily_backup` after the trust check, best effort and silent (stderr stays empty per the CLI contract; the next process tries again). Two processes racing may both take one; retention keeps the newest of the day, so no lock file is needed. MCP (Ticket 12) should call `ensure_daily_backup` at start-up and periodically, like the desktop.
- **Retention: pure `backup::retained`, restic-style.** Each rule keeps the newest backup of each of its N most recent periods *that have a backup* (days; Monday–Sunday weeks; calendar months; all UTC), and a backup kept by any rule stays. Daily and Manual backups share the policy (a Manual backup is just another point in its day). Pre-restore backups are outside it: the newest 3 are kept. Saya pilih "periode yang punya backup" agar Tasknotes yang lama tidak dibuka tidak menghapus semua backup lama sekaligus. UTC rather than local days: the core has no time-zone data, and the policy only needs consistent buckets.
- **Restore validation, before anything live changes:** the id must be one this module made (no paths); size and SHA-256 must match the manifest; then on a private staging copy: SQLite must open it, `user_version` in 1..=13 (newer → `unsupported_schema`), `PRAGMA integrity_check` must be `ok`, `PRAGMA foreign_key_check` empty, and the forward migrations must succeed. Any failure is `invalid_backup` (exit 5) and the live file is untouched.
- **Restore swap: SQLite online backup API, one `step(-1)`,** from the staging copy into the live database. It holds SQLite's write lock on the live file for the whole copy, so other connections see the old data or the new, never a mix, and the file stays in WAL mode. A writer holding the lock is waited for up to the 5 s busy timeout, then the restore is refused (`storage_unavailable`, message "Try again"). Saya pilih ini, bukan rename file, karena proses lain bisa sedang membuka file lama (inode lama, -wal/-shm tidak cocok).
- **Writer coordination: Restore Epoch (schema v13, table `database_epoch`).** A random token in the database. Restore writes a fresh token into the staging copy before the swap, so the token changes atomically with the data. Every store remembers the token it opened with; `connect()` checks it on every operation and `begin_write()` checks it again *after* taking the write lock (`BEGIN IMMEDIATE`), so a restore cannot slip between check and write. A stale store gets `database_restored` (exit 4, like a revision conflict: reload and retry). Every write transaction in the core now goes through `begin_write`; a source-scan test fails if one is opened any other way. The restoring store carries on with the new token (shared by its clones).
- **Desktop after a restore made elsewhere:** one `invoke` wrapper (`src/lib/invoke.ts`) for all core calls; on `database_restored` it calls `adopt_restored_data` and reloads the window, so nothing cached is written back. A restore made in the window reloads it too.
- **Pre-restore backup** of the live data is taken after validation and before the swap; restoring it undoes the restore.
- **Who may restore:** the desktop, and the CLI only after the person types `restore` at an interactive terminal (same deliberate-step model as `access grant`; not a security barrier, Q47). MCP is refused in the core (`desktop_only`). The command is `tasknotes backup restore <id>` to keep the CLI's `<noun> <verb>` grammar.
- **Export (core `export_data`):** one read transaction; everything including Removed items (labeled, so they stay recoverable — export deletes nothing). JSON: one document (`format: "tasknotes-export"`, `format_version: 1`, lists, tasks with tags and progress history, note_folders with `parent_id`, notes, note_links). Markdown: `README.md`, `Tasks/<List>.md` (+ `Inbox.md`), `Notes/<folder tree>/<Note title>.md` with a JSON-quoted front-matter header; names sanitized and de-duplicated case-insensitively; Removed folders/Lists/Tasks get "(Removed)". Never overwrites: written under a hidden partial name, renamed into place.
- **Privacy in export:** the note filter goes through `privacy::shared_with_ai_sql` / `note_shown_sql`. An AI-facing store can never ask for private Notes (`desktop_only`), so its export is exactly what it may read; links to private Notes are left out with them. The desktop leaves them out by default; the person ticks "Include Notes marked Exclude from AI". The CLI has no such option (it is AI-facing, Ticket 10).
- **Desktop export destination:** the Downloads folder under `Tasknotes export YYYY-MM-DD HH.MM(.json)`, with ` (2)` etc. when taken. Saya pilih ini karena belum ada dialog plugin dan desain melarang dialog; Settings shows the saved path.

## Not claimed / open

- A write landing between the Pre-restore snapshot and the swap is replaced by the restore and is not in the Pre-restore backup (window of milliseconds; the swap itself is atomic).
- A refusal's Audit Entry is written in autocommit after `connect()`'s epoch check, not under `begin_write`; in a race with a restore it can land in the restored data. Harmless (no content), noted for review.
- Restores are not recorded as Audit Entries (`entity_table` has no table for a backup); the manifest and the Pre-restore backup are the record.
- Backups are plain SQLite files readable by the person's account, like the live database (ADR-0004 scope).
- No import of exports; no scheduler when every Tasknotes process is closed (spec).
- Native GUI check of the Settings section (owner).

## Vertical TDD slices

1. Core tracer: backup → change → restore brings the data back; a store opened before is refused on read and write; a fresh one works.
2. Core: fake-clock retention over 2026-06-01..10-26 keeps exactly 11 named days; damaged/truncated/foreign/newer snapshots refused with live data untouched; integrity_check catches damage behind a resealed manifest (mutation: removing the check makes it fail); Pre-restore kept and limited to 3; Removed/private preserved; empty DB; uncommitted write excluded; leftovers of interrupted runs ignored and cleaned; locked file waited for, then refused; writer racing a restore; process restart; MCP refused; file modes.
3. Core: export JSON/Markdown, folder tree round trip, Removed labeled, private only on desktop request, AI never (mutation: dropping the filter fails 2 tests), no overwrite, empty DB.
4. CLI: `backup list|create|restore`, `export json|markdown --out`, trust, terminal confirmation, damaged backup, stale desktop store after a CLI restore.
5. Desktop: Settings section, inline confirm, reload, export opt-in, missing commands; invoke wrapper reloads on `database_restored`.
6. Guard: every write transaction goes through `begin_write`.

## Evidence (implementer's own runs, 2026-10-26)

See the ticket's Verification section for the final counts after merging the integration tip.
