//! `tasknotes-mcp`: the local Model Context Protocol server for Tasknotes.
//!
//! An MCP client (Claude Desktop, Claude Code, …) starts this binary and
//! speaks JSON-RPC over its stdin and stdout. It opens no network socket and
//! holds no business or privacy rule of its own: every tool calls the shared
//! core (ADR-0001), opened as `Interface::Mcp`, and answers with the CLI's
//! versioned JSON envelope. stdout carries only protocol messages.

mod server;
mod tools;
mod tools_bulk;

use std::process::ExitCode;

use rmcp::ServiceExt;
use tasknotes_cli::access::Environment;

const USAGE: &str = "Usage: tasknotes-mcp [--data-dir DIR]";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect()
    {
        Ok(args) => args,
        Err(_) => return fail("Every argument must be valid UTF-8 text"),
    };
    let flag = match args.as_slice() {
        [] => None,
        [flag, dir] if flag == "--data-dir" => Some(dir.as_str()),
        _ => return fail(USAGE),
    };
    let env = Environment {
        data_dir: std::env::var_os("TASKNOTES_DATA_DIR"),
        home: std::env::var_os("HOME"),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
        stdin_is_terminal: false,
    };
    let data_dir = match tasknotes_cli::access::data_dir(flag, &env) {
        Ok(dir) => dir,
        Err(error) => return fail(&error.message),
    };
    let service = match server::Tasknotes::new(data_dir)
        .serve(rmcp::transport::stdio())
        .await
    {
        Ok(service) => service,
        Err(error) => return fail(&format!("MCP handshake failed: {error}")),
    };
    match service.waiting().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => fail(&format!("MCP server stopped: {error}")),
    }
}

/// Diagnostics go to stderr; stdout belongs to the protocol.
fn fail(message: &str) -> ExitCode {
    eprintln!("tasknotes-mcp: {message}");
    ExitCode::from(2)
}
