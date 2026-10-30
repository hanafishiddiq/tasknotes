//! One temporary data directory, the real `tasknotes` and `tasknotes-mcp`
//! binaries running against it as separate processes, and the desktop: the
//! core store opened on the same file, as the Tauri commands open it.
//! Synthetic data only, and never the real data directory.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tasknotes_core::backup::Backup;
use tasknotes_core::tasks::TaskStore;

pub const PROTOCOL: &str = "2025-06-18";
/// Ids no Task, Note or link ever had.
pub const NEVER: &str = "00000000-0000-4000-8000-00000000beef";
pub const NEVER_LINK: &str = "00000000-0000-4000-8000-00000000cafe";

pub struct World {
    pub dir: tempfile::TempDir,
}

/// What one CLI process wrote. `stdout` is kept byte for byte.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub json: Value,
    pub stderr: String,
}

impl World {
    /// A data directory where the person has authorized both AI interfaces.
    pub fn new() -> Self {
        let world = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::write(
            world.path().join("ai-access.json"),
            r#"{"schema_version":1,"cli":true,"mcp":true}"#,
        )
        .unwrap();
        world
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn db(&self) -> PathBuf {
        self.path().join("tasknotes.db")
    }

    pub fn backups(&self) -> PathBuf {
        self.path().join("backups")
    }

    /// The person in the window.
    pub fn desktop(&self) -> TaskStore {
        TaskStore::open(&self.db()).unwrap()
    }

    pub fn command(&self, args: &[&str]) -> Command {
        command_in(self.path(), args)
    }

    /// Run the real CLI; stdout must be exactly one JSON document.
    pub fn run(&self, args: &[&str]) -> Run {
        finish(self.command(args).output().unwrap())
    }

    /// Run and require success and an empty stderr; returns the envelope.
    pub fn ok(&self, args: &[&str]) -> Value {
        let run = self.run(args);
        assert_eq!(run.code, 0, "{args:?} failed: {}", run.stdout);
        assert_eq!(run.json["ok"], true, "{args:?}: {}", run.stdout);
        assert!(run.stderr.is_empty(), "{args:?} stderr: {}", run.stderr);
        run.json
    }

    /// Run and require a refusal with this exit code and error code.
    pub fn refused(&self, args: &[&str], exit: i32, code: &str) -> Run {
        let run = self.run(args);
        assert_refused(&run, args, exit, code);
        assert!(run.stderr.is_empty(), "{args:?} stderr: {}", run.stderr);
        run
    }

    /// Start the real CLI with a terminal on its stdin and `answer` typed at
    /// it, as when the person runs it themself. The terminal must stay open
    /// until the process has been waited for.
    pub fn spawn_at_terminal(&self, args: &[&str], answer: &str) -> AtTerminal {
        let (master, slave) = open_terminal();
        let child = self
            .command(args)
            .stdin(Stdio::from(slave))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut terminal = File::from(master);
        terminal.write_all(answer.as_bytes()).unwrap();
        terminal.write_all(b"\n").unwrap();
        AtTerminal {
            child,
            _terminal: terminal,
        }
    }

    /// Run the real CLI at a terminal where the person answers `answer`.
    pub fn at_terminal(&self, args: &[&str], answer: &str) -> Run {
        let at_terminal = self.spawn_at_terminal(args, answer);
        finish(at_terminal.child.wait_with_output().unwrap())
    }

    /// Restore through the real CLI, confirmed by the person; the envelope.
    pub fn restore(&self, id: &str) -> Value {
        let run = self.at_terminal(&["backup", "restore", id], "restore");
        assert_eq!(run.code, 0, "restore failed: {}", run.stdout);
        assert_eq!(run.json["ok"], true, "{}", run.stdout);
        run.json
    }

    /// A Manual backup through the real CLI; its id.
    pub fn backup(&self) -> String {
        text(&self.ok(&["backup", "create"])["item"]["id"]).to_owned()
    }

    /// Start the MCP server and complete the handshake.
    pub fn server(&self) -> Server {
        let mut server = Server::spawn(self.path());
        server.initialize();
        server
    }

    /// Every Task and Note table, row by row, as stored: what a restore must
    /// bring back. Audit Entries and Bulk Change decisions are left out because
    /// a restore keeps those on purpose, and revisions because it retires those
    /// of whatever changed, so a client holding an old one is refused.
    pub fn content(&self) -> BTreeMap<String, Vec<String>> {
        let conn = Connection::open(self.db()).unwrap();
        let mut content = BTreeMap::new();
        for table in [
            "tasks",
            "task_tags",
            "tags",
            "lists",
            "progress_entries",
            "notes",
            "note_folders",
            "task_note_links",
        ] {
            content.insert(table.to_owned(), rows(&conn, table));
        }
        content
    }

    /// Whether the database, as a fresh process finds it, is sound.
    pub fn assert_sound(&self) {
        assert_sound_file(&self.db());
    }

    /// Everything in `backups/` that claims to be a Backup is whole, and both
    /// the desktop and the real CLI list exactly those. `checked` holds the
    /// ids already read in full, so a loop reads each one once.
    pub fn assert_only_whole_backups(&self, checked: &mut BTreeSet<String>) -> Vec<Backup> {
        // The CLI first: every command applies retention, and two processes
        // that raced for the day's backup may leave one for it to prune.
        let shown: Vec<String> = self.ok(&["backup", "list"])["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| text(&b["id"]).to_owned())
            .collect();
        let listed = self.desktop().list_backups().unwrap();
        let ids: Vec<String> = listed.iter().map(|b| b.id.clone()).collect();
        assert_eq!(shown, ids, "the CLI and the desktop list the same Backups");
        for backup in &listed {
            if checked.insert(backup.id.clone()) {
                let snapshot = self.backups().join(format!("{}.sqlite3", backup.id));
                let bytes = std::fs::read(&snapshot).unwrap();
                assert_eq!(bytes.len() as u64, backup.size_bytes, "{}", backup.id);
                assert_eq!(sha256_hex(&bytes), backup.sha256, "{}", backup.id);
                assert_sound_file(&snapshot);
            }
        }
        // A manifest is written last, so none can point at nothing.
        for name in file_names(&self.backups()) {
            if let Some(id) = name.strip_suffix(".json") {
                assert!(ids.iter().any(|b| b == id), "unlisted manifest {name}");
            }
        }
        listed
    }

    /// The ids of what a stopped process left in `backups/` that is not a
    /// Backup: partial snapshots and manifests, staging copies and snapshots
    /// without a manifest.
    pub fn leftovers(&self) -> Vec<String> {
        file_names(&self.backups())
            .into_iter()
            .filter_map(|name| {
                if name == ".restore.lock" {
                    return None;
                }
                if let Some(hidden) = name.strip_prefix('.') {
                    return Some(
                        hidden
                            .trim_end_matches(".partial")
                            .trim_end_matches(".json")
                            .trim_end_matches(".sqlite3")
                            .to_owned(),
                    );
                }
                let id = name.strip_suffix(".sqlite3")?;
                let manifest = self.backups().join(format!("{id}.json"));
                (!manifest.exists()).then(|| id.to_owned())
            })
            .collect()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

/// The real CLI, running at a terminal.
pub struct AtTerminal {
    pub child: Child,
    _terminal: File,
}

pub fn command_in(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tasknotes"));
    command
        .arg("--data-dir")
        .arg(dir)
        .args(args)
        // Never reach a real data directory, whatever the environment.
        .env_remove("TASKNOTES_DATA_DIR")
        .env("HOME", dir.join("home"))
        .stdin(Stdio::null());
    command
}

pub fn finish(output: std::process::Output) -> Run {
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {stdout:?}"));
    assert_eq!(
        json["schema_version"], 1,
        "envelope carries its schema version"
    );
    Run {
        code: output.status.code().unwrap(),
        stdout,
        json,
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

pub fn assert_refused(run: &Run, args: &[&str], exit: i32, code: &str) {
    assert_eq!(run.code, exit, "{args:?}: {}", run.stdout);
    assert_eq!(run.json["ok"], false, "{args:?}: {}", run.stdout);
    assert_eq!(run.json["error"]["code"], code, "{args:?}: {}", run.stdout);
}

pub fn text(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("not text: {value}"))
}

/// The id of the item in an envelope.
pub fn id_of(envelope: &Value) -> String {
    text(&envelope["item"]["id"]).to_owned()
}

pub fn file_names(dir: &Path) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `PRAGMA integrity_check` is ok, `foreign_key_check` is empty and every
/// full-text index agrees with what it indexes.
pub fn assert_sound_file(path: &Path) {
    let conn = Connection::open(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let report: Vec<String> = conn
        .prepare("PRAGMA integrity_check")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(report, ["ok"], "integrity_check of {path:?}");
    let broken: Vec<String> = conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        broken.is_empty(),
        "foreign_key_check of {path:?}: {broken:?}"
    );
    for table in ["task_fields_fts", "progress_fts", "notes_fts"] {
        conn.execute(
            // A non-zero rank also compares an external-content index
            // (`notes_fts`) with the rows it indexes.
            &format!("INSERT INTO {table}({table}, rank) VALUES('integrity-check', 1)"),
            [],
        )
        .unwrap_or_else(|e| panic!("{table} of {path:?}: {e}"));
    }
}

fn rows(conn: &Connection, table: &str) -> Vec<String> {
    let mut statement = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
        .unwrap();
    let kept: Vec<usize> = statement
        .column_names()
        .iter()
        .enumerate()
        .filter(|(_, name)| **name != "revision")
        .map(|(index, _)| index)
        .collect();
    statement
        .query_map([], |row| {
            kept.iter()
                .map(|&i| {
                    Ok(match row.get_ref(i)? {
                        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                        other => format!("{other:?}"),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|values| values.join("|"))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// A JSON record or list of them without its revisions, which a restore retires.
pub fn without_revisions(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(name, _)| *name != "revision")
                .map(|(name, field)| (name.clone(), without_revisions(field)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_revisions).collect()),
        other => other.clone(),
    }
}

/// A new terminal: its controlling end, and the end a process has as stdin.
fn open_terminal() -> (OwnedFd, OwnedFd) {
    let (mut master, mut slave) = (-1, -1);
    // SAFETY: the out-pointers are valid; null asks for the default name,
    // settings and size. The two descriptors are owned from here on.
    let (master, slave) = unsafe {
        let opened = libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(opened, 0, "openpty: {}", std::io::Error::last_os_error());
        // Not inherited by whatever else a parallel test starts.
        libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(slave, libc::F_SETFD, libc::FD_CLOEXEC);
        (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave))
    };
    (master, slave)
}

/// A line the server wrote, or why there is none yet.
pub enum Poll {
    Line(String),
    Nothing,
    Closed,
}

/// The real `tasknotes-mcp` process, spoken to over stdin and stdout.
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
            .stderr(File::create(&stderr_path).unwrap())
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

    /// Send a request without waiting; returns its id.
    pub fn send_request(&mut self, method: &str, params: Value) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send_line(&message.to_string());
        id
    }

    /// The next line, if one arrives within `wait`.
    pub fn poll(&mut self, wait: Duration) -> Poll {
        match self.lines.recv_timeout(wait) {
            Ok(line) => Poll::Line(line),
            Err(RecvTimeoutError::Timeout) => Poll::Nothing,
            Err(RecvTimeoutError::Disconnected) => Poll::Closed,
        }
    }

    /// The next message the server writes, as JSON; panics after 15 s, or if
    /// stdout carries anything that is not JSON.
    pub fn read(&mut self) -> Value {
        match self.lines.recv_timeout(Duration::from_secs(15)) {
            Ok(line) => serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line:?}")),
            Err(RecvTimeoutError::Timeout) => panic!("no reply from the MCP server"),
            Err(RecvTimeoutError::Disconnected) => panic!("the MCP server closed stdout"),
        }
    }

    /// Send one request and return its full JSON-RPC response.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params);
        let response = self.read();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], id, "reply to another request: {response}");
        response
    }

    pub fn notify(&mut self, method: &str) {
        self.send_line(&json!({"jsonrpc": "2.0", "method": method}).to_string());
    }

    pub fn initialize(&mut self) -> Value {
        let response = self.request("initialize", initialize_params());
        assert!(
            response.get("result").is_some(),
            "initialize failed: {response}"
        );
        self.notify("notifications/initialized");
        response
    }

    /// Call a tool; the envelope and its exact text as the client got it.
    pub fn call_text(&mut self, tool: &str, arguments: Value) -> (Value, String) {
        let response = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        let result = response
            .get("result")
            .unwrap_or_else(|| panic!("{tool} returned a protocol error: {response}"));
        let envelope = result["structuredContent"].clone();
        assert_eq!(envelope["schema_version"], 1, "{tool}: {result}");
        let body = text(&result["content"][0]["text"]).to_owned();
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            envelope,
            "{tool}: text and structured content agree"
        );
        assert_eq!(
            result["isError"],
            envelope["ok"] == false,
            "{tool}: {result}"
        );
        (envelope, body)
    }

    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.call_text(tool, arguments).0
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

    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    pub fn stderr(&self) -> String {
        std::fs::read_to_string(&self.stderr_path).unwrap_or_default()
    }

    /// SIGKILL: no chance to finish, flush or close anything.
    pub fn kill(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }

    /// Close stdin and wait for the server to exit; returns its status code.
    /// Whatever it still wrote on the way out must be JSON too.
    pub fn close(mut self) -> Option<i32> {
        drop(self.stdin.take());
        for _ in 0..150 {
            if let Some(status) = self.child.try_wait().unwrap() {
                while let Poll::Line(line) = self.poll(Duration::from_millis(100)) {
                    serde_json::from_str::<Value>(&line)
                        .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line:?}"));
                }
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

pub fn initialize_params() -> Value {
    json!({
        "protocolVersion": PROTOCOL,
        "capabilities": {},
        "clientInfo": {"name": "synthetic-test-client", "version": "0.0.1"},
    })
}

/// Two `World::content` dumps are equal, row for row; a failure names the
/// first row that differs instead of printing all of them.
pub fn assert_same_content(
    actual: &BTreeMap<String, Vec<String>>,
    expected: &BTreeMap<String, Vec<String>>,
    what: &str,
) {
    for (table, rows) in expected {
        let found = &actual[table];
        for (a, b) in found.iter().zip(rows) {
            assert_eq!(a, b, "{what}: a row of {table} differs");
        }
        assert_eq!(found.len(), rows.len(), "{what}: rows of {table}");
    }
}

/// Whether this process holds any internet socket. `None` when `lsof` is not
/// installed, which the caller reports rather than treats as a pass.
pub fn internet_sockets(pid: u32) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-nP", "-a", "-p", &pid.to_string(), "-i"])
        .output()
        .ok()?;
    // lsof prints nothing (and exits 1) when there is none.
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
