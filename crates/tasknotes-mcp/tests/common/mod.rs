//! Spawns the real `tasknotes-mcp` binary and speaks MCP (newline-delimited
//! JSON-RPC 2.0) over its stdin/stdout against a temporary data directory.
//! Synthetic data only.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

pub const PROTOCOL: &str = "2025-06-18";

pub struct Data {
    pub dir: tempfile::TempDir,
}

impl Data {
    /// A fresh data directory the person has not authorized MCP for.
    pub fn untrusted() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// A data directory where the person has authorized MCP (only MCP).
    pub fn trusted() -> Self {
        let data = Self::untrusted();
        data.write_access(r#"{"schema_version":1,"mcp":true}"#);
        data
    }

    pub fn write_access(&self, text: &str) {
        std::fs::write(self.dir.path().join("ai-access.json"), text).unwrap();
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn db(&self) -> PathBuf {
        self.dir.path().join("tasknotes.db")
    }

    /// The desktop's own store on the same file: the person in the window.
    pub fn desktop(&self) -> tasknotes_core::tasks::TaskStore {
        tasknotes_core::tasks::TaskStore::open(&self.db()).unwrap()
    }

    /// Start the server and complete the MCP handshake.
    pub fn server(&self) -> Server {
        let mut server = Server::spawn(self.path());
        server.initialize();
        server
    }
}

pub struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: i64,
    stderr_path: PathBuf,
    _stderr_dir: tempfile::TempDir,
}

impl Server {
    pub fn spawn(data_dir: &Path) -> Self {
        let stderr_dir = tempfile::tempdir().unwrap();
        let stderr_path = stderr_dir.path().join("stderr.txt");
        let mut child = Command::new(env!("CARGO_BIN_EXE_tasknotes-mcp"))
            .arg("--data-dir")
            .arg(data_dir)
            // Never reach a real data directory, whatever the environment.
            .env_remove("TASKNOTES_DATA_DIR")
            .env("HOME", data_dir.join("home"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(std::fs::File::create(&stderr_path).unwrap())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            stdin: child.stdin.take(),
            child,
            lines,
            next_id: 1,
            stderr_path,
            _stderr_dir: stderr_dir,
        }
    }

    pub fn send_line(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    /// The next message the server writes, as JSON; panics after 15 s.
    pub fn read(&mut self) -> Value {
        match self.lines.recv_timeout(Duration::from_secs(15)) {
            Ok(line) => serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line:?}")),
            Err(RecvTimeoutError::Timeout) => panic!("no reply from the MCP server"),
            Err(RecvTimeoutError::Disconnected) => panic!("the MCP server closed stdout"),
        }
    }

    /// Whether the server writes anything within `wait`.
    pub fn silent_for(&mut self, wait: Duration) -> bool {
        matches!(
            self.lines.recv_timeout(wait),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected)
        )
    }

    /// Send one request and return its full JSON-RPC response.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send_line(&message.to_string());
        let response = self.read();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], id, "reply to another request: {response}");
        response
    }

    pub fn notify(&mut self, method: &str) {
        self.send_line(&json!({"jsonrpc": "2.0", "method": method}).to_string());
    }

    pub fn initialize(&mut self) -> Value {
        let response = self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {},
                "clientInfo": {"name": "synthetic-test-client", "version": "0.0.1"},
            }),
        );
        assert!(
            response.get("result").is_some(),
            "initialize failed: {response}"
        );
        self.notify("notifications/initialized");
        response
    }

    pub fn tools(&mut self) -> Vec<Value> {
        let response = self.request("tools/list", json!({}));
        response["result"]["tools"].as_array().unwrap().clone()
    }

    /// Call a tool and return its envelope. Checks the result is the same
    /// envelope as text and as structured content, flagged as an error
    /// exactly when the envelope says `ok: false`.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        let result = response
            .get("result")
            .unwrap_or_else(|| panic!("{tool} returned a protocol error: {response}"));
        let envelope = result["structuredContent"].clone();
        assert_eq!(envelope["schema_version"], 1, "{tool}: {result}");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(
            serde_json::from_str::<Value>(text).unwrap(),
            envelope,
            "{tool}: text and structured content agree"
        );
        assert_eq!(
            result["isError"],
            envelope["ok"] == false,
            "{tool}: {result}"
        );
        envelope
    }

    /// Call and require `ok: true`.
    pub fn ok(&mut self, tool: &str, arguments: Value) -> Value {
        let envelope = self.call(tool, arguments.clone());
        assert_eq!(
            envelope["ok"], true,
            "{tool} {arguments} failed: {envelope}"
        );
        envelope
    }

    /// Call and require a refusal with `code`; returns the `error` object.
    pub fn refused(&mut self, tool: &str, arguments: Value, code: &str) -> Value {
        let envelope = self.call(tool, arguments.clone());
        assert_eq!(envelope["ok"], false, "{tool} {arguments}: {envelope}");
        assert_eq!(
            envelope["error"]["code"], code,
            "{tool} {arguments}: {envelope}"
        );
        envelope["error"].clone()
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn stderr(&self) -> String {
        std::fs::read_to_string(&self.stderr_path).unwrap_or_default()
    }

    /// Close stdin and wait for the server to exit; returns its status code.
    pub fn close(mut self) -> Option<i32> {
        drop(self.stdin.take());
        for _ in 0..150 {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("the MCP server did not exit after stdin closed");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
