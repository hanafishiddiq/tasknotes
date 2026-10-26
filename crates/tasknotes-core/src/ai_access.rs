//! AI-access configuration (Q47): which AI-facing interfaces (CLI, MCP) the
//! person has explicitly authorized for the data directory.
//!
//! This is an explicit user configuration step and an interface authorization
//! boundary. It keeps an AI client from using Tasknotes until the person has
//! set it up; it is not a security barrier. Any process running as the same
//! user with a terminal, or with access to the data directory, can write this
//! file itself (ADR-0004, Q47).
//!
//! The file is `ai-access.json` in the data directory:
//! `{"schema_version":1,"cli":true,"mcp":false}`. Anything else, including a
//! damaged file or another schema version, authorizes nothing.

use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value};

use crate::audit::Interface;
use crate::tasks::{TaskError, TaskStore};

pub const ACCESS_FILE: &str = "ai-access.json";
pub const DATABASE_FILE: &str = "tasknotes.db";
const SCHEMA_VERSION: u64 = 1;

/// Whether the person has authorized this interface. The desktop window is
/// the person themself and is always authorized.
pub fn is_authorized(data_dir: &Path, interface: Interface) -> bool {
    interface == Interface::Desktop
        || read(data_dir)
            .is_some_and(|flags| flags.get(interface.as_str()) == Some(&Value::Bool(true)))
}

/// Open the data directory's database for an AI-facing interface, refusing
/// with `NotAuthorized` before the database is opened, created or migrated.
pub fn open_authorized(data_dir: &Path, interface: Interface) -> Result<TaskStore, TaskError> {
    if !is_authorized(data_dir, interface) {
        return Err(TaskError::NotAuthorized);
    }
    TaskStore::open_as(&data_dir.join(DATABASE_FILE), interface)
}

/// Record the person's decision for one interface, keeping the others. The
/// file is readable only by the person's account. Callers decide how the
/// person confirms; the desktop needs no entry and is left untouched.
pub fn set_authorized(
    data_dir: &Path,
    interface: Interface,
    allowed: bool,
) -> Result<(), TaskError> {
    if interface == Interface::Desktop {
        return Ok(());
    }
    let mut flags = read(data_dir).unwrap_or_default();
    flags.insert("schema_version".into(), SCHEMA_VERSION.into());
    flags.insert(interface.as_str().into(), allowed.into());
    std::fs::create_dir_all(data_dir)?;
    // Write a private temporary file, then rename it over the old one, so a
    // crash never leaves a half-written configuration.
    let temporary = data_dir.join(format!("{ACCESS_FILE}.tmp"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&temporary)?;
    file.write_all(Value::Object(flags).to_string().as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temporary, data_dir.join(ACCESS_FILE))?;
    Ok(())
}

fn read(data_dir: &Path) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(data_dir.join(ACCESS_FILE)).ok()?;
    match serde_json::from_str::<Value>(&text).ok()? {
        Value::Object(flags) if flags.get("schema_version") == Some(&SCHEMA_VERSION.into()) => {
            Some(flags)
        }
        _ => None,
    }
}
