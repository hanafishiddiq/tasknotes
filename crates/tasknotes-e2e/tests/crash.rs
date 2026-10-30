//! Process death: the real MCP server and real CLI processes are killed with
//! SIGKILL while they write, while `backup create` runs and while a restore
//! runs, over and over on one database. Whatever the moment, the next process
//! finds a sound database in which every acknowledged write is present, each
//! write and its Audit Entry stand or fall together, and search still finds
//! what is there. Synthetic data only.
//!
//! SIGKILL ends the process, not the machine: the operating system still holds
//! what was written. These tests say nothing about power loss.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::{
    Poll, Server, World, assert_same_content, command_in, finish, initialize_params, text,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tasknotes_core::notes::{CreateNote, NoteQuery};
use tasknotes_core::tasks::{CreateTask, TaskQuery, TaskStore};

const WRITE_ROUNDS: u64 = 24;
const BACKUP_ROUNDS: u32 = 10;
const RESTORE_ROUNDS: u32 = 10;

/// A small fixed-seed generator: timings vary from round to round and repeat
/// from run to run, so a failure can be chased. No new dependency for it.
fn varied(seed: u64) -> u64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x2545_F491_4F6C_DD1D;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// When to kill in a round: some during start-up, most while writing.
fn kill_delay(seed: u64) -> Duration {
    let x = varied(seed);
    let ms = if x.is_multiple_of(4) {
        1 + x % 30
    } else {
        30 + x % 300
    };
    Duration::from_millis(ms)
}

/// Text long enough that a write spends real time in its transaction and in
/// the full-text index, within the limit on a Task's description.
fn filler() -> &'static str {
    static FILLER: OnceLock<String> = OnceLock::new();
    FILLER.get_or_init(|| {
        (0..1500)
            .map(|i| format!("word{:x}", varied(i) % 4096))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// A longer text for Notes, so a commit writes many pages to the log.
fn note_filler() -> &'static str {
    static FILLER: OnceLock<String> = OnceLock::new();
    FILLER.get_or_init(|| {
        (0..6000)
            .map(|i| format!("n{:x}", varied(i) % 0x10_0000))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// When a writer's process dies: at a set time, or as soon as the log is seen
/// changing, which is the moment some process is committing.
struct Fate {
    at: Instant,
    now: AtomicBool,
}

impl Fate {
    fn due(&self) -> bool {
        self.now.load(Ordering::Relaxed) || Instant::now() >= self.at
    }

    /// Watch the log from `from` until it changes; then every writer is due.
    fn strike_on_commit(&self, wal: &Path, from: Instant) {
        std::thread::sleep(from.saturating_duration_since(Instant::now()));
        let look = || {
            std::fs::metadata(wal)
                .ok()
                // A process opening the database creates an empty log.
                .filter(|m| m.len() > 0)
                .map(|m| (m.len(), m.modified().ok()))
        };
        let first = look();
        while Instant::now() < self.at {
            if look() != first {
                self.now.store(true, Ordering::Relaxed);
                return;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Task,
    Note,
}

impl Kind {
    fn table(self) -> &'static str {
        match self {
            Self::Task => "tasks",
            Self::Note => "notes",
        }
    }

    fn actions(self) -> &'static str {
        match self {
            Self::Task => "task.%",
            Self::Note => "note.%",
        }
    }
}

/// A write some interface said had succeeded.
#[derive(Clone, Debug)]
struct Ack {
    kind: Kind,
    id: String,
    revision: i64,
    token: String,
    interface: &'static str,
}

/// The same four writes, in a cycle, whichever interface runs them.
enum Op {
    CreateTask,
    UpdateTask(String, i64),
    CreateNote,
    UpdateNote(String, i64),
}

struct Plan {
    name: &'static str,
    prefix: char,
    round: u64,
    sequence: u64,
    task: Option<(String, i64)>,
    note: Option<(String, i64)>,
}

/// One planned write: what to do, and the word only this write contains.
struct Planned {
    op: Op,
    token: String,
}

impl Plan {
    fn new(name: &'static str, prefix: char, round: u64) -> Self {
        Self {
            name,
            prefix,
            round,
            sequence: 0,
            task: None,
            note: None,
        }
    }

    fn next(&mut self) -> Planned {
        let step = self.sequence % 4;
        // Same width throughout, so no token is a search prefix of another.
        let token = format!("zz{}{:03}{:05}q", self.prefix, self.round, self.sequence);
        self.sequence += 1;
        let op = match (step, &self.task, &self.note) {
            (1, Some((id, revision)), _) => Op::UpdateTask(id.clone(), *revision),
            (2, ..) => Op::CreateNote,
            (3, _, Some((id, revision))) => Op::UpdateNote(id.clone(), *revision),
            _ => Op::CreateTask,
        };
        Planned { op, token }
    }

    fn acknowledged(&mut self, planned: &Planned, item: &Value) -> Ack {
        let id = text(&item["id"]).to_owned();
        let revision = item["revision"].as_i64().unwrap();
        let kind = match planned.op {
            Op::CreateTask | Op::UpdateTask(..) => Kind::Task,
            Op::CreateNote | Op::UpdateNote(..) => Kind::Note,
        };
        let slot = match kind {
            Kind::Task => &mut self.task,
            Kind::Note => &mut self.note,
        };
        *slot = Some((id.clone(), revision));
        Ack {
            kind,
            id,
            revision,
            token: planned.token.clone(),
            interface: self.name,
        }
    }
}

fn task_title(token: &str) -> String {
    format!("Synthetic {token}")
}

fn note_markdown(token: &str) -> String {
    format!("# Synthetic {token}\n\n{}", note_filler())
}

fn cli_args(planned: &Planned) -> Vec<String> {
    let token = &planned.token;
    let strings = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    match &planned.op {
        Op::CreateTask => strings(&[
            "task",
            "create",
            "--title",
            &task_title(token),
            "--description",
            filler(),
        ]),
        Op::UpdateTask(id, revision) => strings(&[
            "task",
            "update",
            id,
            "--revision",
            &revision.to_string(),
            "--title",
            &task_title(token),
        ]),
        Op::CreateNote => strings(&["note", "create", "--markdown", &note_markdown(token)]),
        Op::UpdateNote(id, revision) => strings(&[
            "note",
            "update",
            id,
            "--revision",
            &revision.to_string(),
            "--markdown",
            &note_markdown(token),
        ]),
    }
}

fn mcp_call(planned: &Planned) -> Value {
    let token = &planned.token;
    let (name, arguments) = match &planned.op {
        Op::CreateTask => (
            "task_create",
            json!({"title": task_title(token), "description": filler()}),
        ),
        Op::UpdateTask(id, revision) => (
            "task_update",
            json!({"id": id, "revision": revision, "title": task_title(token)}),
        ),
        Op::CreateNote => ("note_create", json!({"markdown": note_markdown(token)})),
        Op::UpdateNote(id, revision) => (
            "note_update",
            json!({"id": id, "revision": revision, "markdown": note_markdown(token)}),
        ),
    };
    json!({"name": name, "arguments": arguments})
}

/// What one writer's round came to.
#[derive(Default)]
struct Report {
    acks: Vec<Ack>,
    /// The write it was waiting on when it was killed, by its token.
    in_flight: Option<String>,
    /// Anything refused: a killed neighbour must not cause any.
    refusals: Vec<Value>,
}

/// The CLI writer loop: one process per write, killed where the clock falls.
fn run_cli_writer(data_dir: PathBuf, round: u64, fate: &Fate) -> Report {
    let scratch = tempfile::tempdir().unwrap();
    let stdout_path = scratch.path().join("stdout");
    let mut plan = Plan::new("cli", 'c', round);
    let mut report = Report::default();
    loop {
        let planned = plan.next();
        let args = cli_args(&planned);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        // To a file: a pipe nobody reads yet could stall a process with a
        // long reply until its kill, hiding what it would have done.
        let mut child = command_in(&data_dir, &args)
            .stdout(File::create(&stdout_path).unwrap())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if fate.due() {
                child.kill().unwrap();
                child.wait().unwrap();
                report.in_flight = Some(planned.token);
                return report;
            }
            std::thread::sleep(Duration::from_micros(50));
        };
        let mut output = child.wait_with_output().unwrap();
        output.status = status;
        output.stdout = std::fs::read(&stdout_path).unwrap();
        let run = finish(output);
        assert!(run.stderr.is_empty(), "CLI stderr: {}", run.stderr);
        if run.code == 0 {
            report
                .acks
                .push(plan.acknowledged(&planned, &run.json["item"]));
        } else {
            report.refusals.push(run.json);
        }
    }
}

/// The next message, or `None` once the clock falls and the server is dead.
fn reply_or_kill(server: &mut Server, fate: &Fate) -> Option<Value> {
    loop {
        match server.poll(Duration::from_micros(50)) {
            Poll::Line(line) => {
                return Some(
                    serde_json::from_str(&line)
                        .unwrap_or_else(|e| panic!("MCP stdout line is not JSON ({e}): {line:?}")),
                );
            }
            Poll::Nothing => {
                if fate.due() {
                    server.kill();
                    return None;
                }
            }
            Poll::Closed => panic!("the MCP server closed stdout: {}", server.stderr()),
        }
    }
}

/// The real MCP server writing without pause, killed where the clock falls.
fn run_mcp_writer(data_dir: PathBuf, round: u64, fate: &Fate) -> Report {
    let mut server = Server::spawn(&data_dir);
    let mut plan = Plan::new("mcp", 'm', round);
    let mut report = Report::default();
    server.send_request("initialize", initialize_params());
    if reply_or_kill(&mut server, fate).is_none() {
        return report;
    }
    server.notify("notifications/initialized");
    loop {
        let planned = plan.next();
        server.send_request("tools/call", mcp_call(&planned));
        let Some(response) = reply_or_kill(&mut server, fate) else {
            report.in_flight = Some(planned.token);
            return report;
        };
        let envelope = &response["result"]["structuredContent"];
        if envelope["ok"] == true {
            report
                .acks
                .push(plan.acknowledged(&planned, &envelope["item"]));
        } else {
            report.refusals.push(envelope.clone());
        }
    }
}

/// What the WAL says the kill interrupted: `None` if there is no log left to
/// recover, else whether it ends in frames of a transaction that never
/// committed, the log's own record of a process dying part-way through one.
fn wal_state(db: &Path) -> Option<bool> {
    let bytes = std::fs::read(format!("{}-wal", db.display())).ok()?;
    if bytes.len() < 32 {
        return None;
    }
    let number = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    let page_size = number(8) as usize;
    let salts = &bytes[16..24];
    let mut torn = false;
    let mut frame = 32;
    while frame + 24 + page_size <= bytes.len() && &bytes[frame + 8..frame + 16] == salts {
        // The header's second field is the database size in a commit frame.
        torn = number(frame + 4) == 0;
        frame += 24 + page_size;
    }
    Some(torn)
}

/// Nothing an applied Audit Entry says differs from the rows, in either
/// direction: no write without its entry, no entry without its write.
fn assert_write_and_audit_stand_together(conn: &Connection, kind: Kind) {
    let table = kind.table();
    let actions = kind.actions();
    let mut mismatched = conn
        .prepare(&format!(
            "SELECT t.id, t.revision,
                    (SELECT count(*) FROM audit_entries a
                     WHERE a.entity_id = t.id AND a.outcome = 'applied' AND a.action LIKE ?1),
                    (SELECT max(a.revision) FROM audit_entries a
                     WHERE a.entity_id = t.id AND a.outcome = 'applied' AND a.action LIKE ?1)
             FROM {table} t"
        ))
        .unwrap();
    let rows = mismatched
        .query_map(params![actions], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .unwrap();
    for row in rows {
        let (id, revision, entries, newest) = row.unwrap();
        assert_eq!(entries, revision, "{table} {id}: one entry per revision");
        assert_eq!(newest, Some(revision), "{table} {id}: its newest change");
    }
    let orphans: i64 = conn
        .query_row(
            &format!(
                "SELECT count(*) FROM audit_entries a
                 WHERE a.outcome = 'applied' AND a.action LIKE ?1
                   AND NOT EXISTS (SELECT 1 FROM {table} t WHERE t.id = a.entity_id)"
            ),
            params![actions],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(orphans, 0, "an Audit Entry for a write that never landed");
}

/// Every acknowledged write is there, under the interface that made it.
fn assert_acknowledged_writes_present(conn: &Connection, acks: &[Ack]) {
    for ack in acks {
        let table = ack.kind.table();
        let revision: i64 = conn
            .query_row(
                &format!("SELECT revision FROM {table} WHERE id = ?1"),
                params![ack.id],
                |row| row.get(0),
            )
            .unwrap_or_else(|e| panic!("acknowledged {table} {} is gone: {e}", ack.id));
        assert!(revision >= ack.revision, "{ack:?} went back to {revision}");
        let interface: String = conn
            .query_row(
                "SELECT interface FROM audit_entries
                 WHERE entity_id = ?1 AND revision = ?2 AND outcome = 'applied'",
                params![ack.id, ack.revision],
                |row| row.get(0),
            )
            .unwrap_or_else(|e| panic!("no Audit Entry for {ack:?}: {e}"));
        assert_eq!(interface, ack.interface, "{ack:?}");
    }
}

fn find_task(store: &TaskStore, word: &str) -> Vec<String> {
    let query = TaskQuery {
        text: word.to_owned(),
        ..TaskQuery::default()
    };
    let page = store.query_tasks(&query, 10, None).unwrap();
    page.items.into_iter().map(|task| task.id).collect()
}

fn find_note(store: &TaskStore, word: &str) -> Vec<String> {
    let query = NoteQuery {
        text: word.to_owned(),
        ..NoteQuery::default()
    };
    let page = store.query_notes(&query, 10, None).unwrap();
    page.items.into_iter().map(|note| note.id).collect()
}

/// Every row changed since `since` is found by its own word through the
/// product's search, committed or not acknowledged alike; a word it no longer
/// holds no longer finds it.
fn assert_search_finds_what_is_there(
    store: &TaskStore,
    conn: &Connection,
    acks: &[Ack],
    since: i64,
) {
    let current: BTreeMap<(&str, String), String> = [(Kind::Task, "tasks"), (Kind::Note, "notes")]
        .iter()
        .flat_map(|(_, table)| {
            let mut rows = conn
                .prepare(&format!(
                    "SELECT id, title FROM {table} WHERE updated_at >= ?1"
                ))
                .unwrap();
            let found: Vec<(String, String)> = rows
                .query_map(params![since], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            found
                .into_iter()
                .map(|(id, title)| ((*table, id), title))
                .collect::<Vec<_>>()
        })
        .collect();
    for ((table, id), title) in &current {
        let word = title.rsplit(' ').next().unwrap();
        let found = if *table == "tasks" {
            find_task(store, word)
        } else {
            find_note(store, word)
        };
        assert_eq!(&found, std::slice::from_ref(id), "{table} found by {word}");
    }
    for ack in acks {
        let table = ack.kind.table();
        let Some(title) = current.get(&(table, ack.id.clone())) else {
            continue;
        };
        if !title.ends_with(&ack.token) {
            let found = match ack.kind {
                Kind::Task => find_task(store, &ack.token),
                Kind::Note => find_note(store, &ack.token),
            };
            assert!(found.is_empty(), "a replaced word still finds {}", ack.id);
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[test]
fn writers_killed_mid_write_lose_nothing_they_acknowledged() {
    let w = World::new();
    w.desktop();
    let started = Instant::now();
    let mut acks: Vec<Ack> = Vec::new();
    let (mut in_flight, mut committed_unacknowledged) = (0, 0);
    let (mut recovered, mut torn) = (0, 0);

    for round in 0..WRITE_ROUNDS {
        let round_started = now_ms();
        let now = Instant::now();
        let cli_fate = Fate {
            at: now + kill_delay(round * 2 + 1),
            now: AtomicBool::new(false),
        };
        let mcp_fate = Fate {
            at: now + kill_delay(round * 2 + 2),
            now: AtomicBool::new(false),
        };
        // Every other round both die the moment the log changes, to land
        // inside a commit; the others die at their own unrelated times.
        let aimed = Fate {
            at: now + kill_delay(round * 2 + 1) + Duration::from_millis(150),
            now: AtomicBool::new(false),
        };
        let wal = PathBuf::from(format!("{}-wal", w.db().display()));
        let (cli, mcp) = std::thread::scope(|scope| {
            let (cli_fate, mcp_fate) = if round % 2 == 1 {
                scope.spawn(|| aimed.strike_on_commit(&wal, now + Duration::from_millis(30)));
                (&aimed, &aimed)
            } else {
                (&cli_fate, &mcp_fate)
            };
            let cli = scope.spawn(|| run_cli_writer(w.path().to_path_buf(), round, cli_fate));
            let mcp = scope.spawn(|| run_mcp_writer(w.path().to_path_buf(), round, mcp_fate));
            (cli.join().unwrap(), mcp.join().unwrap())
        });

        // Both are dead. What did they leave for the next process to recover?
        match wal_state(&w.db()) {
            Some(true) => {
                recovered += 1;
                torn += 1;
            }
            Some(false) => recovered += 1,
            None => {}
        }

        // Opening works, and the product's own store is first in.
        let store = w.desktop();
        w.assert_sound();
        let conn = Connection::open(w.db()).unwrap();
        for report in [&cli, &mcp] {
            assert!(
                report.refusals.is_empty(),
                "round {round}: {:?}",
                report.refusals
            );
            acks.extend(report.acks.iter().cloned());
            if let Some(token) = &report.in_flight {
                in_flight += 1;
                let exists = |table: &str| -> bool {
                    conn.query_row(
                        &format!("SELECT 1 FROM {table} WHERE title = ?1"),
                        params![task_title(token)],
                        |_| Ok(()),
                    )
                    .is_ok()
                };
                if exists("tasks") || exists("notes") {
                    committed_unacknowledged += 1;
                }
            }
        }
        assert_write_and_audit_stand_together(&conn, Kind::Task);
        assert_write_and_audit_stand_together(&conn, Kind::Note);
        assert_acknowledged_writes_present(&conn, &acks);
        assert_search_finds_what_is_there(&store, &conn, &acks, round_started);

        // The person carries on at once, in the same store.
        let token = format!("zzd{round:03}00000q");
        let task = store
            .create_task(CreateTask {
                title: task_title(&token),
                description: String::new(),
            })
            .unwrap();
        assert_eq!(find_task(&store, &token), std::slice::from_ref(&task.id));
        acks.push(Ack {
            kind: Kind::Task,
            id: task.id,
            revision: 1,
            token,
            interface: "desktop",
        });
    }

    // The kills were real: they caught writes, and writes got through.
    let acknowledged = acks.len() as u64 - WRITE_ROUNDS;
    eprintln!(
        "{WRITE_ROUNDS} rounds in {:?}: {acknowledged} acknowledged by the CLI and MCP, \
         {in_flight} waiting when killed ({committed_unacknowledged} of those had committed), \
         {recovered} rounds left a log to recover, {torn} of them part-way through a transaction",
        started.elapsed(),
    );
    assert!(
        acknowledged >= WRITE_ROUNDS,
        "too few writes: {acknowledged}"
    );
    assert!(
        in_flight >= WRITE_ROUNDS,
        "too few kills mid-write: {in_flight}"
    );
    assert!(recovered > 0, "no kill left anything to recover");

    // After all of it: the whole history, through a fresh real CLI process.
    let store = w.desktop();
    let conn = Connection::open(w.db()).unwrap();
    assert_write_and_audit_stand_together(&conn, Kind::Task);
    assert_write_and_audit_stand_together(&conn, Kind::Note);
    assert_acknowledged_writes_present(&conn, &acks);
    assert_search_finds_what_is_there(&store, &conn, &acks, 0);
    let last = acks.last().unwrap();
    let found = w.ok(&["task", "list", "--text", &last.token]);
    assert_eq!(found["items"][0]["id"], last.id.as_str());
    w.assert_only_whole_backups(&mut BTreeSet::new());
}

/// Enough data that a backup or a restore takes a while, so a kill can land
/// inside it, and the Daily backup out of the way.
fn seed_large(w: &World) {
    let desktop = w.desktop();
    for i in 0..20 {
        let words: Vec<String> = (0..12_000)
            .map(|n| format!("w{:x}", varied(i * 100_000 + n) % 0x10_0000))
            .collect();
        desktop
            .create_note(CreateNote {
                markdown: format!("# Synthetic large note {i}\n\n{}", words.join(" ")),
                folder_id: None,
            })
            .unwrap();
    }
    desktop
        .create_task(CreateTask {
            title: "Synthetic task in the seed".into(),
            description: String::new(),
        })
        .unwrap();
    w.ok(&["backup", "list"]);
}

/// A leftover is not a Backup: it is not listed (the caller checked) and a
/// restore naming it is refused like an id that never existed, at the terminal
/// where a restore would otherwise run.
fn assert_cannot_restore(w: &World, id: &str) {
    let args = ["backup", "restore", id];
    let run = w.at_terminal(&args, "restore");
    common::assert_refused(&run, &args, 3, "not_found");
}

#[test]
fn a_backup_killed_part_way_leaves_nothing_that_looks_like_a_backup() {
    let w = World::new();
    seed_large(&w);
    let content = w.content();
    let mut checked = BTreeSet::new();
    let timer = Instant::now();
    w.backup();
    let whole = timer.elapsed();

    let mut killed = 0;
    let mut refused = BTreeSet::new();
    for round in 0..BACKUP_ROUNDS {
        let existing: BTreeSet<String> = common::file_names(&w.backups()).into_iter().collect();
        let mut child = w
            .command(&["backup", "create"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Wait until it has begun writing, then give it a varied share of the
        // time a whole backup takes.
        let writing = loop {
            if child.try_wait().unwrap().is_some() {
                break false;
            }
            let began = common::file_names(&w.backups())
                .iter()
                .any(|name| !existing.contains(name));
            if began {
                break true;
            }
            std::thread::sleep(Duration::from_micros(100));
        };
        if writing {
            std::thread::sleep(whole * round / BACKUP_ROUNDS);
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            killed += 1;
        }
        child.wait().unwrap();

        let leftovers: BTreeSet<String> = w.leftovers().into_iter().collect();
        let listed = w.assert_only_whole_backups(&mut checked);
        for id in leftovers.difference(&refused) {
            assert!(listed.iter().all(|b| &b.id != id), "{id} is listed");
            assert_cannot_restore(&w, id);
        }
        refused = leftovers;
        w.assert_sound();
        assert_same_content(&w.content(), &content, "live data after a killed backup");
    }
    eprintln!(
        "{BACKUP_ROUNDS} rounds, a whole backup takes {whole:?}: {killed} killed, \
         {} files left that are not Backups",
        refused.len(),
    );
    assert!(killed >= 3, "too few kills landed: {killed}");
    assert!(
        !refused.is_empty(),
        "no kill left a partial file to examine"
    );

    // The next backup works, and is a real one: the data comes back from it.
    let next = w.backup();
    w.assert_only_whole_backups(&mut checked);
    let expected = w.content();
    w.ok(&["task", "create", "--title", "Synthetic after the backup"]);
    let restored = w.restore(&next);
    assert_eq!(restored["item"]["restored"]["id"], next.as_str());
    assert_same_content(&w.content(), &expected, "restored from the next backup");
    w.assert_sound();
}

#[test]
fn a_restore_killed_part_way_leaves_the_old_data_or_the_new_whole() {
    let w = World::new();
    seed_large(&w);
    let backup = w.backup();
    let restored = w.content();
    let mut checked = BTreeSet::new();

    // A running client lives through all of it.
    let mut server = w.server();
    server.ok("task_list", json!({}));

    w.ok(&["task", "create", "--title", "Synthetic to be replaced"]);
    let timer = Instant::now();
    w.restore(&backup);
    let whole = timer.elapsed();
    server.refused("task_list", json!({}), "database_restored");

    let (mut killed, mut replaced, mut kept, mut fenced) = (0, 0, 0, 0);
    for round in 0..RESTORE_ROUNDS {
        w.ok(&[
            "task",
            "create",
            "--title",
            &format!("Synthetic round {round}"),
        ]);
        let live = w.content();
        let mut at_terminal = w.spawn_at_terminal(&["backup", "restore", &backup], "restore");
        // Wait until it has begun on its private copy, the first step that
        // leaves anything behind, then let it run a little further each round:
        // through validation, the fence, the Pre-restore backup and the swap.
        while at_terminal.child.try_wait().unwrap().is_none()
            && !common::file_names(&w.backups())
                .iter()
                .any(|name| name.starts_with(".restore-"))
        {
            std::thread::sleep(Duration::from_micros(100));
        }
        std::thread::sleep(whole * round / (RESTORE_ROUNDS * 6));
        if at_terminal.child.try_wait().unwrap().is_none() {
            at_terminal.child.kill().unwrap();
            killed += 1;
        }
        at_terminal.child.wait().unwrap();

        // Before anything else opens it: did it die with writers fenced off?
        let epoch: String = Connection::open(w.db())
            .unwrap()
            .query_row("SELECT epoch FROM database_epoch", [], |row| row.get(0))
            .unwrap();
        if epoch.starts_with("restoring-") {
            fenced += 1;
        }

        // Old or new, whole; never a mix.
        w.assert_sound();
        let now = w.content();
        if now == restored {
            replaced += 1;
        } else {
            assert_same_content(&now, &live, "a killed restore that did not finish");
            kept += 1;
        }
        // A fence the dead restore left is gone: the next process writes.
        w.ok(&[
            "task",
            "create",
            "--title",
            &format!("Synthetic after {round}"),
        ]);
        let epoch: String = Connection::open(w.db())
            .unwrap()
            .query_row("SELECT epoch FROM database_epoch", [], |row| row.get(0))
            .unwrap();
        assert!(!epoch.starts_with("restoring-"), "the fence was not lifted");
        w.assert_only_whole_backups(&mut checked);
        // The client is told if the data moved, then reads what is there.
        let call = server.call("task_list", json!({"limit": 100}));
        if call["ok"] == false {
            assert_eq!(call["error"]["code"], "database_restored", "{call}");
        }
        let listed = server.ok("task_list", json!({"limit": 100}));
        assert!(
            listed["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|task| task["title"] == format!("Synthetic after {round}")),
            "{listed}"
        );
    }
    eprintln!(
        "{RESTORE_ROUNDS} rounds, a whole restore takes {whole:?}: {killed} killed, \
         old data kept {kept} times ({fenced} with writers fenced off), \
         replaced {replaced} times"
    );
    assert!(killed >= 3, "too few kills landed: {killed}");
    assert!(kept >= 1, "no kill landed before the swap");

    // And a restore left to finish still works after all that.
    w.restore(&backup);
    assert_same_content(&w.content(), &restored, "the last restore");
    w.assert_sound();
    assert!(server.is_running());
    assert!(
        server.stderr().is_empty(),
        "MCP stderr: {}",
        server.stderr()
    );
}
