# 13: Backup, validated restore, and export

**What to build:** Automatic local backup snapshots (7 daily, 4 weekly, 3 monthly), validated restoration coordinated with other writers, and user-controlled JSON/Markdown export. Test corrupted backups, conflict handling and preservation of Removed data.

**Blocked by:** 09 — Removed and restore, 11 — Private, AI-optimized Note CRUD in CLI

**Status:** ready-for-agent

**Implementation:** implemented and merged into `integration/tasknotes-v1-07-18` with Ticket 12 on 2026-10-30; `/code-review` done, two restore findings wait for the owner (see the Ticket 12 file's last comment). Native GUI check of the Settings section is open. Plan: `docs/plans/ticket-13-backup-restore-export-2026-10-26.md`.

## Acceptance criteria

- [x] While a Tasknotes process is active, consistent local SQLite backups follow the approved 7 daily / 4 weekly / 3 monthly retention policy. — `VACUUM INTO` snapshots once per local calendar day from the desktop (launch + hourly) and every CLI data command; `daily_backups_follow_the_7_daily_4_weekly_3_monthly_policy` (fake clock, 148 days → exactly 11 named survivors), `a_backup_leaves_out_a_write_still_in_progress`, CLI `any_data_command_takes_the_days_backup_…`.
- [x] Restoring a valid backup verifies integrity and coordinates with active UI, CLI and MCP writers to prevent stale writes. — manifest size/SHA-256, `integrity_check`, `foreign_key_check`, schema range, forward migration on a staging copy; Restore Epoch checked on every connection and under the write lock in `begin_write`; `restoring_a_backup_…_refuses_every_store_opened_before`, `writers_racing_a_restore_…`, `every_write_transaction_checks_the_restore_epoch`, CLI `a_confirmed_restore_…_stops_stale_writers`; desktop reloads via `src/lib/invoke.ts`.
- [x] A corrupted/incomplete snapshot cannot replace the live database; a pre-restore recovery snapshot is retained. — `a_damaged_or_truncated_snapshot_is_refused_…`, `a_damaged_snapshot_with_a_matching_manifest_fails_the_integrity_check` (fails with the check removed), `a_file_that_is_not_a_tasknotes_database_or_is_newer_is_refused`, `a_pre_restore_backup_keeps_the_data_a_restore_replaced`, `only_the_newest_pre_restore_backups_are_kept`.
- [x] Users can export Task and Note data to JSON and Markdown; removed items remain recoverable and export options are clearly labeled. — `tests/export.rs` (JSON folder `parent_id` round trip, Markdown directory tree, Removed labeled, no overwrite, empty DB); Settings "Export as JSON / Markdown" with "Include Notes marked Exclude from AI"; `tasknotes export json|markdown --out`.
- [x] An AI-facing export must respect Exclude from AI, whereas explicit human-facing export can include user-selected private Notes. — `an_ai_facing_export_never_contains_a_private_note` (CLI and MCP stores, both formats; asking is `desktop_only`; fails with the filter dropped), `the_person_can_include_private_notes_in_a_desktop_export`, CLI `the_cli_export_is_ai_facing_…`.
- [x] The recovery tests cover process restart, interrupted writes, locked files and empty databases without touching real user data. — `a_restarted_process_sees_the_restored_data_…`, `an_interrupted_backup_or_restore_leaves_nothing_that_looks_like_a_backup`, `a_backup_leaves_out_a_write_still_in_progress`, `a_restore_waits_for_a_writer_and_refuses_when_the_file_stays_locked`, `an_empty_database_can_be_backed_up_and_restored`; all in temporary directories.

## Verification

- [x] Run backup/restore integration tests with deliberate corruption, writer-contention and synthetic Removed/private items. — `tests/backup_restore.rs` 18 passed, `tests/export.rs` 6 passed, CLI `tests/backup.rs` 6 passed (implementer's runs, 2026-10-26).
- [x] Relevant tests pass and evidence is recorded here. — implementer's own runs on 2026-10-26 at the integration tip a05b6ec (unchanged, nothing to merge): `crates/tasknotes-core` cargo test 168 passed; `crates/tasknotes-cli` 39 passed; `src-tauri` 42 passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean in all three; `pnpm test` 241 passed; `pnpm build` ok. Not yet run: native GUI check of Settings → Backups and export (owner).
- [x] Run `/code-review` and address findings before marking complete. — review findings 1–10 addressed on 2026-10-26 (see below and the plan's "Review fixes"); native GUI check still open.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-22:** Note Folders can be nested (owner decision, see Ticket 07). Markdown export should keep the folder tree (for example as directories) and JSON export must keep each folder's parent, so a round trip does not flatten it.

**2026-10-26 — review fixes:** `/code-review` findings fixed on `feat/ticket-13-backup-restore-export`, each with a test written first (the reviewer's reproductions ported into the repo). Details and trade-offs: plan, section "Review fixes". **Coordinator decisions (the owner may revisit):** Audit Entries stay append-only across a restore (live entries the snapshot lacks are carried over, AI visibility frozen, and a `backup.restore` entry is recorded; Progress Entries return to the snapshot); Pre-restore backups are never auto-pruned (`backups/` grows by one full copy per restore; the person manages the folder); Manual backups are their own bucket of the 10 newest and never prune Daily or Pre-restore ones; day boundaries use the local time zone.

- Data loss fixed: restore fences every writer (fence epoch under `begin_write`, `backups/.restore.lock`) before the Pre-restore snapshot — `no_write_that_succeeds_during_a_restore_is_lost` (was 20 of 45 lost before the fix in my red run), `a_store_cannot_open_mid_restore_and_a_stopped_restores_fence_is_lifted`.
- Schema validation: `a_foreign_database_with_a_plausible_schema_version_is_refused`, `a_backup_from_an_older_schema_is_upgraded_on_restore`.
- Audit: `a_restore_keeps_the_audit_history_and_records_itself`, `a_refusal_racing_a_restore_is_kept_or_reported_unrecorded`.
- Retention / local days: `manual_backups_are_their_own_bucket_of_ten`, `backup_days_follow_the_local_calendar`, `pre_restore_backups_are_never_pruned`.
- Export: `notes_whose_titles_differ_only_in_unicode_form_get_two_files`, `exports_are_private_to_the_persons_account`, `a_destination_without_a_name_is_its_own_error` (all three red before the fix).
- Robustness: CLI `a_failed_automatic_backup_is_reported_in_the_envelope` (`warnings` in the envelope, stderr empty); orphan snapshot cleanup (test fails with the cleanup removed); `NewBackup.retention_error` so a pruning failure never fails a backup; `sync_dir` errors returned.
- Epoch guard: store connections are `query_only` until `begin_write` (`a_store_connection_writes_only_through_begin_write`); scan limits raw connections to `tasks.rs`/`backup.rs`.
- **For Ticket 12:** `ensure_daily_backup()` (now `Result<Option<NewBackup>>`) stays callable from any adapter; the MCP server must call it at start-up and periodically, without failing tools when it errors.
- Not covered by a test: a restore that fails *after* fencing (no seam to fail the snapshot or swap mid-way); the code puts the old epoch back, and the next open lifts a leftover fence.
- Verification (implementer's own runs, 2026-10-26; integration tip still a05b6ec, nothing to merge): `crates/tasknotes-core` cargo test 182 passed; `crates/tasknotes-cli` 40 passed; `src-tauri` 42 passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean in all three; `pnpm test` 241 passed; `pnpm build` ok. Native GUI check of Settings → Backups and export remains open (owner).
