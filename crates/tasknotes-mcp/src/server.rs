//! The MCP handler: lists the tools and runs one call. Authorization is
//! checked on every call, before the arguments are read or the database is
//! opened, so a revoked client is refused at once and an untrusted one learns
//! nothing about the data.

use std::path::PathBuf;

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

use crate::tools::{Context, Previews, ToolDef};

const INSTRUCTIONS: &str = "Tasknotes: the person's local Tasks, Lists and Notes. \
Every tool answers with one JSON envelope {schema_version, ok, item | items + next_cursor | error}. \
Changes name the revision you last read; a revision_conflict means someone else changed it, \
so read again before retrying. Removed is reversible: nothing is ever deleted. \
Changing several items at once goes through bulk_preview, then bulk_confirm with its token.";

pub struct Tasknotes {
    data_dir: PathBuf,
    tools: Vec<ToolDef>,
    previews: Previews,
}

impl Tasknotes {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            tools: crate::tools::all(),
            previews: Previews::default(),
        }
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
        let store = ai_access::open_authorized(&self.data_dir, Interface::Mcp).map_err(
            |error| match error {
                TaskError::NotAuthorized => not_trusted(),
                error => CliError::from(error),
            },
        )?;
        tool.call(
            &Context {
                store: &store,
                previews: &self.previews,
            },
            arguments,
        )
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
        let envelope = self.run(tool, request.arguments);
        let result = if envelope["ok"] == Value::Bool(true) {
            CallToolResult::structured(envelope)
        } else {
            CallToolResult::structured_error(envelope)
        };
        Ok(result.into())
    }
}
