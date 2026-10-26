//! Where the data lives, and the terminal step by which the person grants
//! CLI access. Reading and checking the access configuration is the core's
//! (`tasknotes_core::ai_access`), so the MCP adapter applies the same rule.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use tasknotes_core::ai_access;
use tasknotes_core::audit::Interface;

use crate::contract::CliError;

/// The desktop app's identifier; its OS data directory holds `tasknotes.db`.
pub const APP_IDENTIFIER: &str = "store.adamaja.tasknotes";

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

/// Record the person's decision to authorize the CLI, but only from an
/// interactive terminal where they type `allow`.
///
/// This is an explicit configuration step, not a security barrier: it keeps
/// an AI client from being authorized by accident, but any process with a
/// terminal, or with same-user access to the data directory, can bypass it
/// (Q47, ADR-0004). The desktop window has no switch for it yet.
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
    Ok(ai_access::set_authorized(data_dir, Interface::Cli, true)?)
}

pub fn revoke(data_dir: &Path) -> Result<(), CliError> {
    Ok(ai_access::set_authorized(data_dir, Interface::Cli, false)?)
}
