//! The MCP handler: lists the tools and runs one call. Authorization is
//! checked on every call, before the arguments are read or the database is
//! opened, so a revoked client is refused at once and an untrusted one learns
//! nothing about the data.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};
use tasknotes_cli::contract::{self, CliError};
use tasknotes_core::ai_access;
use tasknotes_core::audit::Interface;
use tasknotes_core::tasks::TaskError;

use crate::tools::{Context, ToolDef};

/// How often a running server checks whether the day's backup is due, like
/// the desktop.
const BACKUP_CHECK: Duration = Duration::from_secs(60 * 60);

const INSTRUCTIONS: &str = "Tasknotes: the person's local Tasks, Lists and Notes. \
Every tool answers with one JSON envelope {schema_version, ok, item | items + next_cursor | error}. \
Changes name the revision you last read; a revision_conflict means someone else changed it, \
so read again before retrying. Removed is reversible: nothing is ever deleted. \
Changing several items at once, or removing a List or Note Folder that holds anything, goes through \
bulk_preview: it waits for the person to approve it in the Tasknotes app (you cannot approve it); \
read its outcome with bulk_status.";

pub struct Tasknotes {
    data_dir: PathBuf,
    /// Opening the store runs one at a time: a client may send several calls
    /// at once, and the first may create and migrate the database, which
    /// another connection opening in the same moment would find locked.
    /// Once open, each call has its own connection and waits out SQLite's
    /// busy timeout like any other writer (the CLI, the desktop).
    opening: Arc<Mutex<()>>,
    tools: Vec<ToolDef>,
}

impl Tasknotes {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            opening: Arc::new(Mutex::new(())),
            tools: crate::tools::all(),
        }
    }

    /// Backups run while any Tasknotes process is active (spec): check now
    /// and then every hour, for as long as the server lives. A failure is
    /// reported on stderr and never reaches a tool.
    pub fn keep_daily_backups(&self) {
        let data_dir = self.data_dir.clone();
        let opening = Arc::clone(&self.opening);
        std::thread::spawn(move || {
            loop {
                if let Some(problem) = daily_backup(&data_dir, &opening) {
                    eprintln!("tasknotes-mcp: {problem}");
                }
                std::thread::sleep(BACKUP_CHECK);
            }
        });
    }

    /// Run one tool call to its envelope.
    fn run(&self, tool: &ToolDef, arguments: Option<Map<String, Value>>) -> Value {
        match self.run_inner(tool, arguments) {
            Ok(envelope) => envelope,
            Err(error) => contract::failure(&error),
        }
    }

    fn run_inner(
        &self,
        tool: &ToolDef,
        arguments: Option<Map<String, Value>>,
    ) -> Result<Value, CliError> {
        if !ai_access::is_authorized(&self.data_dir, Interface::Mcp) {
            return Err(not_trusted());
        }
        let arguments = Value::Object(arguments.unwrap_or_default());
        // The core checks authorization again and refuses before the
        // database is opened, created or migrated.
        let store = {
            let _one_at_a_time = self.opening.lock().unwrap_or_else(|e| e.into_inner());
            ai_access::open_authorized(&self.data_dir, Interface::Mcp)
        }
        .map_err(|error| match error {
            TaskError::NotAuthorized => not_trusted(),
            error => CliError::from(error),
        })?;
        tool.call(&Context { store: &store }, arguments)
    }
}

/// Take the day's backup if it is due. Before the person has authorized MCP,
/// or before a database exists, there is nothing to do and nothing is opened.
fn daily_backup(data_dir: &Path, opening: &Mutex<()>) -> Option<String> {
    let database = data_dir.join(ai_access::DATABASE_FILE);
    if !ai_access::is_authorized(data_dir, Interface::Mcp) || !database.exists() {
        return None;
    }
    let store = {
        let _one_at_a_time = opening.lock().unwrap_or_else(|e| e.into_inner());
        ai_access::open_authorized(data_dir, Interface::Mcp)
    };
    match store.and_then(|store| store.ensure_daily_backup()) {
        Ok(taken) => taken.and_then(|t| t.retention_error).map(|error| {
            format!("The backup was taken, but an old backup could not be removed: {error}")
        }),
        Err(error) => Some(format!(
            "Today's automatic backup could not be taken: {error}"
        )),
    }
}

fn not_trusted() -> CliError {
    CliError::new(
        "not_trusted",
        "Tasknotes MCP access has not been set up for this data directory. \
         The person using Tasknotes can run `tasknotes access grant --mcp` in a terminal.",
    )
}

impl ServerHandler for Tasknotes {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("tasknotes", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(
            self.tools.iter().map(|t| t.tool.clone()).collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let Some(tool) = self.tools.iter().find(|t| t.tool.name == request.name) else {
            return Err(McpError::invalid_params(
                format!("Unknown tool: {}", request.name),
                None,
            ));
        };
        // The core is synchronous SQLite; keep it off the async workers.
        let envelope = tokio::task::block_in_place(|| self.run(tool, request.arguments));
        let result = if envelope["ok"] == Value::Bool(true) {
            CallToolResult::structured(envelope)
        } else {
            CallToolResult::structured_error(envelope)
        };
        Ok(result.into())
    }
}
