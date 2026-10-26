//! `backup` and `export`: thin mappings onto the core's backup, restore and
//! export operations (Ticket 13). The rules (validation, retention, Restore
//! Epoch, Exclude from AI) are all in `tasknotes-core`.

use std::path::PathBuf;

use serde_json::{Value, json};
use tasknotes_core::backup::Backup;
use tasknotes_core::export::{ExportFormat, ExportRequest, ExportSummary};
use tasknotes_core::tasks::TaskStore;

use crate::access::Environment;
use crate::args::parse;
use crate::contract::{self, CliError};
use crate::{NO_FLAGS, ONE_ID, USAGE, flags};

/// Shown at the terminal before a restore; the person types `restore`.
pub const RESTORE_PROMPT: &str = "Replace all current Tasknotes data with this backup? \
    A Pre-restore backup of the current data is kept. Type restore to confirm: ";

/// Take the day's automatic backup if it is due; a failure becomes a
/// warning, never an error of the command itself.
pub fn daily_backup(store: &TaskStore, warnings: &mut Vec<Value>) {
    match store.ensure_daily_backup() {
        Ok(taken) => {
            if let Some(error) = taken.and_then(|t| t.retention_error) {
                warnings.push(retention_warning(&error));
            }
        }
        Err(error) => warnings.push(contract::warning(
            "backup_failed",
            format!("Today's automatic backup could not be taken: {error}"),
        )),
    }
}

fn retention_warning(error: &str) -> Value {
    contract::warning(
        "backup_retention_failed",
        format!("The backup was taken, but an old backup could not be removed: {error}"),
    )
}

pub fn backup_command(
    store: &TaskStore,
    verb: &str,
    words: &[String],
    env: &Environment,
    confirm: impl FnOnce(&str) -> std::io::Result<String>,
    warnings: &mut Vec<Value>,
) -> Result<Value, CliError> {
    match verb {
        "list" => {
            parse(words, &NO_FLAGS)?;
            let backups = store.list_backups()?;
            Ok(contract::success_page(
                backups.iter().map(backup).collect(),
                None,
            ))
        }
        "create" => {
            parse(words, &NO_FLAGS)?;
            let taken = store.create_backup()?;
            if let Some(error) = &taken.retention_error {
                warnings.push(retention_warning(error));
            }
            Ok(contract::success_item(backup(&taken.backup)))
        }
        "restore" => {
            let p = parse(words, &ONE_ID)?;
            // Restore replaces everything, so, like `access grant`, it needs
            // the person at an interactive terminal; an AI client has none.
            // A deliberate step, not a security barrier (Q47).
            if !env.stdin_is_terminal {
                return Err(CliError::new(
                    "not_trusted",
                    "A restore can only be confirmed by the person using Tasknotes, \
                     at an interactive terminal",
                ));
            }
            let answer = confirm(RESTORE_PROMPT)
                .map_err(|e| CliError::new("storage_unavailable", e.to_string()))?;
            if answer.trim() != "restore" {
                return Err(CliError::new(
                    "not_trusted",
                    "The restore was not confirmed",
                ));
            }
            let outcome = store.restore_backup(p.id())?;
            Ok(contract::success_item(json!({
                "restored": backup(&outcome.restored),
                "pre_restore": backup(&outcome.pre_restore),
            })))
        }
        _ => Err(CliError::invalid(USAGE)),
    }
}

/// `export json|markdown --out PATH`. AI-facing: Notes marked Exclude from
/// AI are never included, and there is no option to ask for them.
pub fn export_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let format = match verb {
        "json" => ExportFormat::Json,
        "markdown" => ExportFormat::Markdown,
        _ => {
            return Err(CliError::invalid(
                "Usage: tasknotes export <json|markdown> --out PATH",
            ));
        }
    };
    let p = parse(words, &flags(&["out"], 0))?;
    let summary = store.export_data(ExportRequest {
        format,
        destination: PathBuf::from(p.required("out")?),
        include_private_notes: false,
    })?;
    Ok(contract::success_item(export_summary(&summary)))
}

fn backup(backup: &Backup) -> Value {
    json!({
        "id": backup.id,
        "kind": contract::to_value(&backup.kind),
        "created_at": backup.created_at,
        "schema_version": backup.schema_version,
        "size_bytes": backup.size_bytes,
        "sha256": backup.sha256,
    })
}

fn export_summary(summary: &ExportSummary) -> Value {
    json!({
        "destination": summary.destination.display().to_string(),
        "format": contract::to_value(&summary.format),
        "lists": summary.lists,
        "tasks": summary.tasks,
        "note_folders": summary.note_folders,
        "notes": summary.notes,
        "note_links": summary.note_links,
        "private_notes": summary.private_notes,
    })
}
