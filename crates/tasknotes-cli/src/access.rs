//! Where the data lives, and whether the person has granted CLI access (Q47).
//!
//! Access is an interface authorization boundary, not protection from a
//! same-user process that edits files directly (ADR-0004).

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::contract::CliError;

/// The desktop app's identifier; its OS data directory holds `tasknotes.db`.
const APP_IDENTIFIER: &str = "store.adamaja.tasknotes";
pub const DATABASE_FILE: &str = "tasknotes.db";
pub const TRUST_FILE: &str = "ai-access.json";

/// What the process was started with, so tests never depend on the real
/// environment.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    /// `TASKNOTES_DATA_DIR`.
    pub data_dir: Option<OsString>,
    pub home: Option<OsString>,
    pub xdg_data_home: Option<OsString>,
    pub stdin_is_terminal: bool,
}

/// `--data-dir`, else `TASKNOTES_DATA_DIR`, else the directory the desktop
/// app uses in a release build.
pub fn data_dir(flag: Option<&str>, env: &Environment) -> Result<PathBuf, CliError> {
    if let Some(dir) = flag {
        return Ok(PathBuf::from(dir));
    }
    if let Some(dir) = env.data_dir.as_ref().filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = env.home.as_ref().filter(|h| !h.is_empty()).ok_or_else(|| {
        CliError::new(
            "storage_unavailable",
            "No home directory; pass --data-dir or set TASKNOTES_DATA_DIR",
        )
    })?;
    let base = if cfg!(target_os = "macos") {
        Path::new(home).join("Library/Application Support")
    } else {
        env.xdg_data_home
            .as_ref()
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(home).join(".local/share"))
    };
    Ok(base.join(APP_IDENTIFIER))
}

/// Granted only by a well-formed file saying exactly `"cli": true`.
pub fn cli_trusted(data_dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(data_dir.join(TRUST_FILE)) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    value["schema_version"] == 1 && value["cli"] == true
}

pub fn require_trust(data_dir: &Path) -> Result<(), CliError> {
    if cli_trusted(data_dir) {
        Ok(())
    } else {
        Err(CliError::not_trusted())
    }
}

/// Grant access, but only to a person at an interactive terminal who types
/// `allow`. AI clients normally run commands without a terminal, so they
/// cannot grant themselves access through this command.
pub fn grant(
    data_dir: &Path,
    env: &Environment,
    confirm: impl FnOnce() -> std::io::Result<String>,
) -> Result<(), CliError> {
    if !env.stdin_is_terminal {
        return Err(CliError::new(
            "not_trusted",
            "Access can only be granted from an interactive terminal by the person using Tasknotes",
        ));
    }
    let answer = confirm().map_err(|e| CliError::new("storage_unavailable", e.to_string()))?;
    if answer.trim() != "allow" {
        return Err(CliError::new("not_trusted", "Access was not granted"));
    }
    std::fs::create_dir_all(data_dir)
        .map_err(|e| CliError::new("storage_unavailable", e.to_string()))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
        .open(data_dir.join(TRUST_FILE))
        .and_then(|mut file| file.write_all(br#"{"schema_version":1,"cli":true}"#))
        .map_err(|e| CliError::new("storage_unavailable", e.to_string()))
}

pub fn revoke(data_dir: &Path) -> Result<(), CliError> {
    match std::fs::remove_file(data_dir.join(TRUST_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CliError::new("storage_unavailable", e.to_string())),
    }
}
