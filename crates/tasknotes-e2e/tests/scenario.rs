//! One long scenario on one database: the real CLI, the real MCP server and
//! the desktop (the core store the Tauri commands call), in the order a person
//! and their AI clients would meet them. Each ticket is tested alone elsewhere;
//! this proves the interfaces agree with each other, and that one MCP server
//! stays correct through everything, restores included. Synthetic data only.

mod common;

use common::{
    NEVER, NEVER_LINK, Server, World, assert_same_content, id_of, internet_sockets, text,
};
use serde_json::{Value, json};
use tasknotes_cli::contract;
use tasknotes_core::note_folders::CreateNoteFolder;
use tasknotes_core::notes::{CreateNote, NoteQuery, UpdateNote};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{CreateTask, PatchTask, TaskQuery, TaskStore};

const SECRET: &str = "zzprivatemarker";
const PRIVATE_TITLE: &str = "Synthetic private diary";
const NEVER_WORD: &str = "zzwordnobodywrote";

struct Stage {
    w: World,
    desktop: TaskStore,
    mcp: Server,
}

/// One row of an entity's Audit trail, oldest first.
type Trail = Vec<(String, String, String, Option<i64>, Option<String>)>;

impl Stage {
    /// The Task as each interface reads it: the same shape, the same values.
    fn task(&mut self, id: &str) -> Value {
        let cli = self.w.ok(&["task", "get", id])["item"].clone();
        let mcp = self.mcp.ok("task_get", json!({"id": id}))["item"].clone();
        assert_eq!(cli, mcp, "the CLI and MCP read Task {id} alike");
        let desktop = self.desktop.get_task(id).unwrap().unwrap();
        assert_eq!(cli, contract::task(&desktop), "and so does the desktop");
        cli
    }

    fn revision(&mut self, id: &str) -> i64 {
        self.task(id)["revision"].as_i64().unwrap()
    }

    fn rename_on_desktop(&self, id: &str, revision: i64, title: &str) -> Result<(), String> {
        self.desktop
            .patch_task(PatchTask {
                id: id.to_owned(),
                expected_revision: revision,
                title: Some(title.to_owned()),
                description: None,
            })
            .map(|_| ())
            .map_err(|e| e.code().to_owned())
    }

    /// Everything the Audit log says about one entity, from the desktop, and
    /// the proof that both AI interfaces read the same entries.
    fn trail(&mut self, entity: &str) -> Trail {
        let entries = self
            .desktop
            .list_audit_entries(Some(entity), 100, None)
            .unwrap()
            .items;
        let as_json: Vec<Value> = entries.iter().map(contract::to_value).collect();
        let cli = self
            .w
            .ok(&["audit", "list", "--entity", entity, "--limit", "100"]);
        let mcp = self
            .mcp
            .ok("audit_list", json!({"entity_id": entity, "limit": 100}));
        assert_eq!(cli["items"], json!(as_json), "the CLI's audit of {entity}");
        assert_eq!(mcp["items"], json!(as_json), "MCP's audit of {entity}");
        entries
            .iter()
            .rev()
            .map(|e| {
                (
                    e.interface.as_str().to_owned(),
                    e.action.as_str().to_owned(),
                    e.outcome.as_str().to_owned(),
                    e.revision,
                    e.error_code.clone(),
                )
            })
            .collect()
    }

    /// Bulk Change status as all three interfaces read it.
    fn bulk_status(&mut self, request: &str) -> String {
        let cli = self.w.ok(&["bulk", "status", request])["item"].clone();
        let mcp = self.mcp.ok("bulk_status", json!({"id": request}))["item"].clone();
        assert_eq!(cli, mcp, "the CLI and MCP read request {request} alike");
        let desktop = self.desktop.bulk_request(request).unwrap();
        assert_eq!(cli["status"], contract::to_value(&desktop.status));
        assert_eq!(
            cli["requested_by"],
            contract::to_value(&desktop.requested_by)
        );
        assert_eq!(cli["decided_at"], json!(desktop.decided_at));
        text(&cli["status"]).to_owned()
    }

    fn active_titles(&mut self) -> Vec<String> {
        let cli = self.w.ok(&["task", "list", "--limit", "100"]);
        let mcp = self.mcp.ok("task_list", json!({"limit": 100}));
        assert_eq!(cli["items"], mcp["items"], "active Tasks agree");
        cli["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| text(&t["title"]).to_owned())
            .collect()
    }

    fn create_on_cli(&self, title: &str) -> String {
        id_of(&self.w.ok(&["task", "create", "--title", title]))
    }

    fn create_on_mcp(&mut self, title: &str) -> String {
        id_of(&self.mcp.ok("task_create", json!({"title": title})))
    }

    fn create_on_desktop(&self, title: &str) -> String {
        self.desktop
            .create_task(CreateTask {
                title: title.to_owned(),
                description: String::new(),
            })
            .unwrap()
            .id
    }

    fn assert_server_quiet_and_offline(&mut self) {
        assert!(
            self.mcp.is_running(),
            "the same MCP server is still running"
        );
        let stderr = self.mcp.stderr();
        assert!(stderr.is_empty(), "MCP stderr: {stderr}");
        match internet_sockets(self.mcp.pid()) {
            Some(sockets) => assert!(sockets.is_empty(), "MCP opened a socket: {sockets}"),
            None => eprintln!("lsof is not available; the socket check did not run"),
        }
    }
}

fn row(
    interface: &str,
    action: &str,
    outcome: &str,
    revision: i64,
    error: Option<&str>,
) -> (String, String, String, Option<i64>, Option<String>) {
    (
        interface.to_owned(),
        action.to_owned(),
        outcome.to_owned(),
        Some(revision),
        error.map(str::to_owned),
    )
}

struct Three {
    desktop: String,
    cli: String,
    mcp: String,
}

/// A Task created in each interface is, at once, the same Task in the others.
fn create_from_each_interface(s: &mut Stage) -> Three {
    let three = Three {
        desktop: s.create_on_desktop("Synthetic desktop errand"),
        cli: s.create_on_cli("Synthetic cli errand"),
        mcp: s.create_on_mcp("Synthetic mcp errand"),
    };
    for id in [&three.desktop, &three.cli, &three.mcp] {
        let task = s.task(id);
        assert_eq!(task["revision"], 1);
        assert_eq!(task["status"], "ready");
        assert_eq!(task["priority"], "medium");
        assert_eq!(task["progress"], 0);
        assert_eq!(task["removed"], false);
    }
    let mut titles = s.active_titles();
    titles.sort();
    assert_eq!(
        titles,
        [
            "Synthetic cli errand",
            "Synthetic desktop errand",
            "Synthetic mcp errand",
        ]
    );
    let desktop_page = s
        .desktop
        .query_tasks(&TaskQuery::default(), 100, None)
        .unwrap();
    let shown: Vec<Value> = desktop_page.items.iter().map(contract::task).collect();
    assert_eq!(
        json!(shown),
        s.w.ok(&["task", "list", "--limit", "100"])["items"],
        "the desktop lists them in the same order"
    );
    three
}

/// Each interface wins once. The other two hold the revision it replaced and
/// are refused with the same stable code, and the winner's edit stands.
fn stale_revisions_are_refused_across_interfaces(s: &mut Stage, three: &Three) {
    let stale = "Synthetic stale overwrite";

    s.mcp.ok(
        "task_update",
        json!({"id": three.cli, "revision": 1, "title": "Synthetic won by mcp"}),
    );
    s.w.refused(
        &[
            "task",
            "update",
            &three.cli,
            "--revision",
            "1",
            "--title",
            stale,
        ],
        4,
        "revision_conflict",
    );
    assert_eq!(
        s.rename_on_desktop(&three.cli, 1, stale).unwrap_err(),
        "revision_conflict"
    );
    assert_eq!(s.task(&three.cli)["title"], "Synthetic won by mcp");

    s.w.ok(&[
        "task",
        "update",
        &three.mcp,
        "--revision",
        "1",
        "--title",
        "Synthetic won by cli",
    ]);
    s.mcp.refused(
        "task_update",
        json!({"id": three.mcp, "revision": 1, "title": stale}),
        "revision_conflict",
    );
    assert_eq!(
        s.rename_on_desktop(&three.mcp, 1, stale).unwrap_err(),
        "revision_conflict"
    );
    assert_eq!(s.task(&three.mcp)["title"], "Synthetic won by cli");

    s.rename_on_desktop(&three.desktop, 1, "Synthetic won by desktop")
        .unwrap();
    s.w.refused(
        &[
            "task",
            "update",
            &three.desktop,
            "--revision",
            "1",
            "--title",
            stale,
        ],
        4,
        "revision_conflict",
    );
    s.mcp.refused(
        "task_update",
        json!({"id": three.desktop, "revision": 1, "title": stale}),
        "revision_conflict",
    );
    let task = s.task(&three.desktop);
    assert_eq!(task["title"], "Synthetic won by desktop");
    assert_eq!(task["revision"], 2, "a refused write bumps nothing");
}

/// The Audit log names who did what, the refusals included.
fn audit_names_the_right_interface(s: &mut Stage, three: &Three) {
    let conflict = Some("revision_conflict");
    assert_eq!(
        s.trail(&three.cli),
        [
            row("cli", "task.create", "applied", 1, None),
            row("mcp", "task.update", "applied", 2, None),
            row("cli", "task.update", "rejected", 1, conflict),
            row("desktop", "task.update", "rejected", 1, conflict),
        ]
    );
    assert_eq!(
        s.trail(&three.mcp),
        [
            row("mcp", "task.create", "applied", 1, None),
            row("cli", "task.update", "applied", 2, None),
            row("mcp", "task.update", "rejected", 1, conflict),
            row("desktop", "task.update", "rejected", 1, conflict),
        ]
    );
    assert_eq!(
        s.trail(&three.desktop),
        [
            row("desktop", "task.create", "applied", 1, None),
            row("desktop", "task.update", "applied", 2, None),
            row("cli", "task.update", "rejected", 1, conflict),
            row("mcp", "task.update", "rejected", 1, conflict),
        ]
    );
}

/// Removing keeps everything, shows nowhere but Removed, refuses every edit
/// from every interface, and restoring brings back exactly what was there.
fn remove_and_restore_across_interfaces(s: &mut Stage, three: &Three) {
    // Worth keeping: started, partly done, with a Next Step.
    let revision = s.revision(&three.desktop).to_string();
    s.w.ok(&[
        "task",
        "progress",
        &three.desktop,
        "--revision",
        &revision,
        "--progress",
        "40",
        "--next-step",
        "Synthetic next step",
    ]);
    let before = s.task(&three.desktop);
    assert_eq!(before["status"], "in_progress");

    // Removed by MCP.
    let revision = before["revision"].as_i64().unwrap();
    let removed = s.mcp.ok(
        "task_remove",
        json!({"id": three.desktop, "revision": revision}),
    )["item"]
        .clone();
    assert_eq!(removed["removed"], true);
    assert_eq!(removed["status"], "in_progress", "Removed is not a status");
    let held = removed["revision"].as_i64().unwrap();
    let held_text = held.to_string();

    assert!(
        !s.active_titles()
            .contains(&"Synthetic won by desktop".to_owned())
    );
    let cli_removed = s.w.ok(&["task", "list", "--removed"]);
    let mcp_removed = s.mcp.ok("task_list", json!({"removed": true}));
    assert_eq!(cli_removed["items"], mcp_removed["items"]);
    assert_eq!(cli_removed["items"][0]["id"], three.desktop.as_str());
    let cli_view = s.w.ok(&["removed", "list"]);
    assert_eq!(
        cli_view["items"],
        s.mcp.ok("removed_list", json!({}))["items"]
    );
    assert_eq!(cli_view["items"][0]["id"], three.desktop.as_str());
    assert_eq!(
        s.desktop.list_removed("", 100, None).unwrap().items.len(),
        1
    );

    // Read-only, for everyone, even with the current revision.
    s.w.refused(
        &[
            "task",
            "update",
            &three.desktop,
            "--revision",
            &held_text,
            "--title",
            "Synthetic edit",
        ],
        5,
        "removed",
    );
    s.w.refused(
        &[
            "task",
            "change",
            &three.desktop,
            "--revision",
            &held_text,
            "--priority",
            "high",
        ],
        5,
        "removed",
    );
    s.mcp.refused(
        "task_update",
        json!({"id": three.desktop, "revision": held, "title": "Synthetic edit"}),
        "removed",
    );
    s.mcp.refused(
        "task_progress",
        json!({"id": three.desktop, "revision": held, "progress": 90}),
        "removed",
    );
    assert_eq!(
        s.rename_on_desktop(&three.desktop, held, "Synthetic edit")
            .unwrap_err(),
        "removed"
    );
    assert_eq!(
        s.task(&three.desktop),
        removed,
        "nothing moved while Removed"
    );

    // A stale restore is refused too. Restored by the CLI, all as it was.
    s.w.refused(
        &["task", "restore", &three.desktop, "--revision", "1"],
        4,
        "revision_conflict",
    );
    s.w.ok(&["task", "restore", &three.desktop, "--revision", &held_text]);
    let restored = s.task(&three.desktop);
    for field in [
        "title",
        "description",
        "status",
        "progress",
        "next_step",
        "priority",
    ] {
        assert_eq!(restored[field], before[field], "{field} survives Removed");
    }
    assert_eq!(restored["removed"], false);
    s.rename_on_desktop(
        &three.desktop,
        restored["revision"].as_i64().unwrap(),
        "Synthetic edited after restore",
    )
    .unwrap();

    // The CLI removes and the desktop restores.
    let revision = s.revision(&three.cli).to_string();
    let held =
        s.w.ok(&["task", "remove", &three.cli, "--revision", &revision])["item"]["revision"]
            .as_i64()
            .unwrap();
    s.desktop
        .set_task_removed(SetRemoved {
            id: three.cli.clone(),
            expected_revision: held,
            removed: false,
        })
        .unwrap();
    assert_eq!(s.task(&three.cli)["removed"], false);

    // The desktop removes and MCP restores.
    let revision = s.revision(&three.mcp);
    let held = s
        .desktop
        .set_task_removed(SetRemoved {
            id: three.mcp.clone(),
            expected_revision: revision,
            removed: true,
        })
        .unwrap()
        .revision;
    assert_eq!(s.task(&three.mcp)["removed"], true);
    s.mcp
        .ok("task_restore", json!({"id": three.mcp, "revision": held}));
    assert_eq!(s.task(&three.mcp)["removed"], false);

    let actions = |trail: Trail| -> Vec<(String, String)> {
        trail
            .into_iter()
            .filter(|(_, action, outcome, ..)| {
                outcome == "applied" && (action == "task.remove" || action == "task.restore")
            })
            .map(|(interface, action, ..)| (interface, action))
            .collect()
    };
    let pair = |i: &str, a: &str| (i.to_owned(), a.to_owned());
    assert!(
        s.trail(&three.desktop).contains(&row(
            "cli",
            "task.restore",
            "rejected",
            1,
            Some("revision_conflict")
        )),
        "the stale restore is on record"
    );
    assert_eq!(
        actions(s.trail(&three.desktop)),
        [pair("mcp", "task.remove"), pair("cli", "task.restore")]
    );
    assert_eq!(
        actions(s.trail(&three.cli)),
        [pair("cli", "task.remove"), pair("desktop", "task.restore")]
    );
    assert_eq!(
        actions(s.trail(&three.mcp)),
        [pair("desktop", "task.remove"), pair("mcp", "task.restore")]
    );

    // A Note follows the same rule across interfaces.
    let note = s.w.ok(&[
        "note",
        "create",
        "--markdown",
        "# Synthetic shared plan\n\nordinary words",
    ]);
    let note_id = id_of(&note);
    let removed_note = s
        .mcp
        .ok("note_remove", json!({"id": note_id, "revision": 1}))["item"]
        .clone();
    let held = removed_note["revision"].as_i64().unwrap();
    assert_eq!(
        s.desktop
            .update_note(UpdateNote {
                id: note_id.clone(),
                markdown: "# Synthetic edit".into(),
                expected_revision: held,
            })
            .unwrap_err()
            .code(),
        "removed"
    );
    s.desktop
        .set_note_removed(SetRemoved {
            id: note_id.clone(),
            expected_revision: held,
            removed: false,
        })
        .unwrap();
    assert_eq!(
        s.w.ok(&["note", "get", &note_id])["item"]["markdown"],
        "# Synthetic shared plan\n\nordinary words"
    );
}

/// What each AI interface shows about the place a private Note lives, with
/// no id of its own in the question. (CLI arguments, MCP tool, MCP arguments.)
fn ai_views(task: &str, folder: &str) -> Vec<(Vec<String>, &'static str, Value)> {
    let cli = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    vec![
        (cli(&["note", "list"]), "note_list", json!({})),
        (
            cli(&["note", "list", "--limit", "1"]),
            "note_list",
            json!({"limit": 1}),
        ),
        (
            cli(&["note", "list", "--text", "ordinary"]),
            "note_list",
            json!({"text": "ordinary"}),
        ),
        (
            cli(&["note", "list", "--text", "synthetic"]),
            "note_list",
            json!({"text": "synthetic"}),
        ),
        (
            cli(&["note", "list", "--folder", folder]),
            "note_list",
            json!({"folder_id": folder}),
        ),
        (cli(&["folder", "list"]), "folder_list", json!({})),
        (
            cli(&["folder", "get", folder]),
            "folder_get",
            json!({"id": folder}),
        ),
        (
            cli(&["task", "notes", task]),
            "task_notes",
            json!({"id": task}),
        ),
        (cli(&["task", "get", task]), "task_get", json!({"id": task})),
        (
            cli(&["task", "list", "--text", "ordinary"]),
            "task_list",
            json!({"text": "ordinary"}),
        ),
        (cli(&["removed", "list"]), "removed_list", json!({})),
        (
            cli(&["removed", "list", "--text", "ordinary"]),
            "removed_list",
            json!({"text": "ordinary"}),
        ),
        (
            cli(&["audit", "list", "--limit", "100"]),
            "audit_list",
            json!({"limit": 100}),
        ),
        (
            cli(&["audit", "list", "--limit", "1"]),
            "audit_list",
            json!({"limit": 1}),
        ),
        (
            cli(&["audit", "list", "--entity", task]),
            "audit_list",
            json!({"entity_id": task}),
        ),
    ]
}

/// Every view, byte for byte, from each AI interface.
fn capture(s: &mut Stage, views: &[(Vec<String>, &'static str, Value)]) -> Vec<(String, String)> {
    views
        .iter()
        .map(|(args, tool, arguments)| {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let run = s.w.run(&args);
            assert_eq!(run.code, 0, "{args:?}: {}", run.stdout);
            let (envelope, body) = s.mcp.call_text(tool, arguments.clone());
            assert_eq!(run.json, envelope, "{args:?} and {tool} agree");
            (run.stdout, body)
        })
        .collect()
}

/// The same question about `id`, for each AI interface: every way to name a
/// Note or its link in a read, a search, a change or an audit query.
fn id_questions(note: &str, link: &str, task: &str) -> Vec<(Vec<String>, &'static str, Value)> {
    let cli = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    vec![
        (cli(&["note", "get", note]), "note_get", json!({"id": note})),
        (
            cli(&["note", "tasks", note]),
            "note_tasks",
            json!({"id": note}),
        ),
        (
            cli(&[
                "note",
                "update",
                note,
                "--revision",
                "1",
                "--markdown",
                "# AI text",
            ]),
            "note_update",
            json!({"id": note, "revision": 1, "markdown": "# AI text"}),
        ),
        (
            cli(&["note", "move", note, "--revision", "1", "--no-folder"]),
            "note_move",
            json!({"id": note, "revision": 1, "no_folder": true}),
        ),
        (
            cli(&["note", "remove", note, "--revision", "1"]),
            "note_remove",
            json!({"id": note, "revision": 1}),
        ),
        (
            cli(&["note", "restore", note, "--revision", "1"]),
            "note_restore",
            json!({"id": note, "revision": 1}),
        ),
        (
            cli(&["link", "create", "--task", task, "--note", note]),
            "link_create",
            json!({"task_id": task, "note_id": note}),
        ),
        (cli(&["link", "get", link]), "link_get", json!({"id": link})),
        (
            cli(&["link", "remove", link]),
            "link_remove",
            json!({"id": link}),
        ),
        (
            cli(&["link", "restore", link]),
            "link_restore",
            json!({"id": link}),
        ),
        (
            cli(&["audit", "list", "--entity", note]),
            "audit_list",
            json!({"entity_id": note}),
        ),
        (
            cli(&["audit", "list", "--entity", link]),
            "audit_list",
            json!({"entity_id": link}),
        ),
        (
            cli(&["bulk", "preview", "--action", "remove_notes", "--id", note]),
            "bulk_preview",
            json!({"action": "remove_notes", "ids": [note]}),
        ),
    ]
}

/// Both AI interfaces answer about the private `note` and `link` exactly as
/// about ids nobody ever used, down to the byte.
fn assert_same_as_unused(s: &mut Stage, note: &str, link: &str, task: &str) {
    let hidden = id_questions(note, link, task);
    let unused = id_questions(NEVER, NEVER_LINK, task);
    for ((hidden_args, tool, hidden_call), (unused_args, _, unused_call)) in
        hidden.iter().zip(&unused)
    {
        let hidden_args: Vec<&str> = hidden_args.iter().map(String::as_str).collect();
        let unused_args: Vec<&str> = unused_args.iter().map(String::as_str).collect();
        let a = s.w.run(&hidden_args);
        let b = s.w.run(&unused_args);
        assert_eq!(a.code, b.code, "{hidden_args:?}");
        assert_eq!(a.stdout, b.stdout, "{hidden_args:?}");
        assert_eq!(a.stderr, b.stderr, "{hidden_args:?}");
        let (_, a) = s.mcp.call_text(tool, hidden_call.clone());
        let (_, b) = s.mcp.call_text(tool, unused_call.clone());
        assert_eq!(a, b, "{tool} {hidden_call}");
    }
    // A search for what only the private Note says finds what a search for a
    // word nobody wrote finds.
    let found = s.w.run(&["note", "list", "--text", SECRET]);
    let nothing = s.w.run(&["note", "list", "--text", NEVER_WORD]);
    assert_eq!(found.stdout, nothing.stdout);
    let (_, found) = s.mcp.call_text("note_list", json!({"text": SECRET}));
    let (_, nothing) = s.mcp.call_text("note_list", json!({"text": NEVER_WORD}));
    assert_eq!(found, nothing);
    assert_not_in_any_ai_listing(s, &[note, link]);
}

/// The whole Audit log, every page, and the Note lists through both AI
/// interfaces never mention these ids.
fn assert_not_in_any_ai_listing(s: &mut Stage, ids: &[&str]) {
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["audit", "list", "--limit", "100"];
        if let Some(cursor) = &cursor {
            args.extend(["--cursor", cursor]);
        }
        let run = s.w.ok(&args);
        let mcp = s
            .mcp
            .ok("audit_list", json!({"limit": 100, "cursor": cursor}));
        assert_eq!(run["items"], mcp["items"], "both read the same Audit log");
        seen.push(run["items"].to_string());
        match run["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    seen.push(s.w.ok(&["note", "list", "--limit", "100"]).to_string());
    seen.push(s.mcp.ok("note_list", json!({"limit": 100})).to_string());
    seen.push(s.w.ok(&["removed", "list", "--limit", "100"]).to_string());
    for id in ids {
        assert!(seen.iter().all(|page| !page.contains(id)), "{id} leaked");
    }
}

/// The person's own view: they see the private Note everywhere.
fn assert_desktop_sees(s: &Stage, note: &str, task: &str) {
    let got = s.desktop.get_note(note).unwrap().unwrap();
    assert!(got.exclude_from_ai);
    assert!(got.markdown.contains(SECRET));
    let found = s
        .desktop
        .query_notes(
            &NoteQuery {
                text: SECRET.into(),
                ..NoteQuery::default()
            },
            10,
            None,
        )
        .unwrap();
    assert!(found.items.iter().any(|hit| hit.id == note));
    let linked = s.desktop.list_task_notes(task, 100, None).unwrap();
    assert!(linked.items.iter().any(|l| l.note_id == note));
    let audit = s
        .desktop
        .list_audit_entries(Some(note), 100, None)
        .unwrap()
        .items;
    assert!(audit.iter().any(|e| e.action.as_str() == "note.privacy"));
}

struct Private {
    note: String,
    link: String,
    task: String,
}

/// A Note marked Exclude from AI on the desktop: both AI interfaces show
/// exactly what they showed before it existed, and treat it as an unused id.
fn exclude_from_ai_holds_on_every_ai_surface(s: &mut Stage) -> Private {
    let task = s.create_on_cli("Synthetic linked errand with ordinary words");
    let folder = s
        .desktop
        .create_note_folder(CreateNoteFolder {
            name: "Synthetic vault".into(),
            parent_id: None,
        })
        .unwrap()
        .id;
    let views = ai_views(&task, &folder);
    let before = capture(s, &views);

    let note = s
        .desktop
        .create_note(CreateNote {
            markdown: format!("# {PRIVATE_TITLE}\n\n{SECRET} ordinary words"),
            folder_id: Some(folder.clone()),
        })
        .unwrap();
    let link = s.desktop.link_note(&task, &note.id).unwrap();
    let note = s
        .desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: note.id,
            exclude_from_ai: true,
        })
        .unwrap();
    // The person's own stale save leaves a refusal in the Audit log about it.
    assert_eq!(
        s.desktop
            .update_note(UpdateNote {
                id: note.id.clone(),
                markdown: "# Synthetic stale save".into(),
                expected_revision: note.revision + 5,
            })
            .unwrap_err()
            .code(),
        "revision_conflict"
    );

    let after = capture(s, &views);
    for (((args, ..), before), after) in views.iter().zip(&before).zip(&after) {
        assert_eq!(before.0, after.0, "the CLI's {args:?} changed");
        assert_eq!(before.1, after.1, "MCP's answer to {args:?} changed");
    }
    assert_same_as_unused(s, &note.id, &link.id, &task);
    // Asking changed nothing an AI can see, and left no trace of the Note.
    let again = capture(s, &views);
    assert_eq!(before, again);
    // Nor anything the person can see.
    assert_eq!(s.desktop.get_note(&note.id).unwrap().unwrap(), note);
    assert!(
        s.desktop
            .get_link(&link.id)
            .unwrap()
            .unwrap()
            .unlinked_at
            .is_none()
    );
    assert_desktop_sees(s, &note.id, &task);
    assert_eq!(s.desktop.count_folder_notes(Some(&folder)).unwrap(), 1);
    Private {
        note: note.id,
        link: link.id,
        task,
    }
}

fn request_id(envelope: &Value) -> String {
    text(&envelope["item"]["request_id"]).to_owned()
}

/// A Bulk Change waits for the person, whoever asked: neither AI interface
/// can apply one, the desktop's approval does, and a request that went stale
/// changes nothing at all.
fn bulk_changes_apply_only_on_desktop_approval(s: &mut Stage) {
    let a = s.create_on_mcp("Synthetic batch a");
    let b = s.create_on_cli("Synthetic batch b");
    let c = s.create_on_desktop("Synthetic batch c");
    let d = s.create_on_cli("Synthetic batch d");

    let from_mcp = request_id(&s.mcp.ok(
        "bulk_preview",
        json!({"action": "set_task_status", "ids": [a, b], "status": "completed"}),
    ));
    let from_cli = request_id(&s.w.ok(&[
        "bulk",
        "preview",
        "--action",
        "remove_tasks",
        "--id",
        &c,
        "--id",
        &d,
    ]));

    // Each interface reads the other's request; nothing has changed.
    assert_eq!(s.bulk_status(&from_mcp), "pending");
    assert_eq!(s.bulk_status(&from_cli), "pending");
    assert_eq!(s.task(&a)["status"], "ready");
    assert_eq!(s.task(&c)["removed"], false);
    assert_eq!(s.desktop.count_pending_bulk_requests().unwrap(), 2);
    let waiting = s.desktop.pending_bulk_requests().unwrap();
    assert_eq!(waiting.len(), 2);
    assert!(waiting.iter().all(|p| !p.stale && p.total == 2));

    // There is no way for an AI to approve its own request.
    s.w.refused(&["bulk", "approve", &from_mcp], 2, "invalid_input");
    let gone = s.mcp.request(
        "tools/call",
        json!({"name": "bulk_approve", "arguments": {"id": from_cli}}),
    );
    assert_eq!(gone["error"]["code"], -32602, "{gone}");
    assert_eq!(s.bulk_status(&from_mcp), "pending");

    // Approved: applied, and everyone reads the outcome.
    s.desktop.approve_bulk_request(&from_mcp).unwrap();
    s.desktop.approve_bulk_request(&from_cli).unwrap();
    assert_eq!(s.bulk_status(&from_mcp), "applied");
    assert_eq!(s.bulk_status(&from_cli), "applied");
    for id in [&a, &b] {
        let task = s.task(id);
        assert_eq!(task["status"], "completed");
        assert_eq!(task["progress"], 100);
    }
    for id in [&c, &d] {
        assert_eq!(s.task(id)["removed"], true);
    }

    // The items are the desktop's, asked for by whoever asked.
    let last = |s: &Stage, id: &str| {
        s.desktop
            .list_audit_entries(Some(id), 1, None)
            .unwrap()
            .items[0]
            .clone()
    };
    let by_mcp = last(s, &a);
    assert_eq!(by_mcp.interface.as_str(), "desktop");
    assert_eq!(by_mcp.requested_by.unwrap().as_str(), "mcp");
    let by_cli = last(s, &c);
    assert_eq!(by_cli.action.as_str(), "task.remove");
    assert_eq!(by_cli.interface.as_str(), "desktop");
    assert_eq!(by_cli.requested_by.unwrap().as_str(), "cli");
    let asked = s
        .desktop
        .list_audit_entries(Some(&from_cli), 10, None)
        .unwrap()
        .items;
    let asked: Vec<(&str, &str)> = asked
        .iter()
        .rev()
        .map(|e| (e.action.as_str(), e.interface.as_str()))
        .collect();
    assert_eq!(
        asked,
        [("bulk.request", "cli"), ("bulk.approve", "desktop")]
    );

    // Stale: one item changed after both asked, so neither applies any of it.
    let e = s.create_on_mcp("Synthetic stale e");
    let f = s.create_on_cli("Synthetic stale f");
    let stale_mcp = request_id(&s.mcp.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": [e, f]}),
    ));
    let stale_cli = request_id(&s.w.ok(&[
        "bulk",
        "preview",
        "--action",
        "set_task_status",
        "--status",
        "completed",
        "--id",
        &e,
        "--id",
        &f,
    ]));
    let e_before = s.task(&e);
    s.w.ok(&[
        "task",
        "update",
        &f,
        "--revision",
        "1",
        "--title",
        "Synthetic edited first",
    ]);
    let f_after = s.task(&f);
    assert!(
        s.desktop
            .pending_bulk_requests()
            .unwrap()
            .iter()
            .all(|p| p.stale)
    );
    for request in [&stale_mcp, &stale_cli] {
        assert_eq!(
            s.desktop.approve_bulk_request(request).unwrap_err().code(),
            "stale_preview"
        );
        assert_eq!(s.bulk_status(request), "stale");
    }
    assert_eq!(s.task(&e), e_before, "all or nothing");
    assert_eq!(s.task(&f), f_after, "all or nothing");
    assert_eq!(s.desktop.count_pending_bulk_requests().unwrap(), 0);
}

/// Backups run from the CLI while one MCP server stays up through two
/// restores, one on the desktop and one by the real CLI at a terminal. It is
/// told `database_restored` once each time, then reads what is true now. The
/// exclusion and the decisions made since the Backup all survive.
fn restores_while_an_mcp_server_stays_running(s: &mut Stage, private: &Private) {
    // In the Backup: a Note the AI can read, and two requests still waiting.
    let kept = s.create_on_cli("Synthetic kept errand");
    let later_private = id_of(&s.mcp.ok(
        "note_create",
        json!({"markdown": format!("# Synthetic shared for now\n\n{SECRET} later")}),
    ));
    let later_link = s.desktop.link_note(&kept, &later_private).unwrap().id;
    let approved_task = s.create_on_mcp("Synthetic approved later");
    let declined_task = s.create_on_cli("Synthetic declined later");
    let approved = request_id(&s.mcp.ok(
        "bulk_preview",
        json!({"action": "remove_tasks", "ids": [approved_task]}),
    ));
    let declined = request_id(&s.w.ok(&[
        "bulk",
        "preview",
        "--action",
        "remove_tasks",
        "--id",
        &declined_task,
    ]));
    assert_eq!(
        s.mcp.ok("note_get", json!({"id": later_private}))["item"]["id"],
        later_private.as_str()
    );

    let backup = s.w.backup();
    let backed_up = s.w.content();
    let at_backup = s.task(&kept);

    // Since the Backup: the person's decisions and everyone's edits.
    s.desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: later_private.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    s.desktop.approve_bulk_request(&approved).unwrap();
    s.desktop.decline_bulk_request(&declined).unwrap();
    assert_eq!(s.task(&approved_task)["removed"], true);
    s.mcp.ok(
        "task_update",
        json!({"id": kept, "revision": 1, "title": "Synthetic edit since the backup"}),
    );
    let later = s.create_on_cli("Synthetic made since the backup");
    // Made and made private since the Backup: the restore takes the Note away
    // but keeps the Audit Entries, which must not name it to an AI then.
    let gone_private = s
        .desktop
        .create_note(CreateNote {
            markdown: "# Synthetic private since the backup".into(),
            folder_id: None,
        })
        .unwrap()
        .id;
    let gone_link = s.desktop.link_note(&kept, &gone_private).unwrap().id;
    s.desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: gone_private.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    assert_same_as_unused(s, &later_private, &later_link, &private.task);
    let approved_at = s.desktop.bulk_request(&approved).unwrap().decided_at;
    let declined_at = s.desktop.bulk_request(&declined).unwrap().decided_at;
    assert!(approved_at.is_some() && declined_at.is_some());

    // A second window, open since before the restore.
    let window = s.w.desktop();

    // The desktop restores; the server was never told.
    let outcome = s.desktop.restore_backup(&backup).unwrap();
    assert_eq!(outcome.restored.id, backup);

    // Told once, on the first call, whatever it is: here, a write that would
    // otherwise land, since the restore rewound the revision it names.
    s.mcp.refused(
        "task_update",
        json!({"id": kept, "revision": 1, "title": "Synthetic stale write"}),
        "database_restored",
    );
    // Then fresh reads.
    let read = s.mcp.ok("task_get", json!({"id": kept}));
    assert_eq!(
        read["item"], at_backup,
        "the restored Task, not what it wrote"
    );
    s.mcp.refused("task_get", json!({"id": later}), "not_found");

    // A window from before must reload, however often it tries; a new one and
    // a new CLI process just read.
    for _ in 0..2 {
        assert_eq!(
            window.get_task(&kept).unwrap_err().code(),
            "database_restored"
        );
    }
    assert_eq!(s.task(&kept), at_backup);
    s.w.refused(&["task", "get", &later], 3, "not_found");
    // The data is the Backup's, but for the one flag a restore keeps.
    let mut expected = backed_up.clone();
    let shared_row = expected["notes"]
        .iter()
        .position(|row| row.starts_with(&later_private))
        .unwrap();
    let flag = expected["notes"][shared_row].rfind('|').unwrap();
    let notes = expected.get_mut("notes").unwrap();
    notes[shared_row].truncate(flag);
    notes[shared_row].push_str("|Integer(1)");
    assert_same_content(&s.w.content(), &expected, "after the restore");

    // The Note stays excluded although the Backup had it shared, on every
    // surface. The person still sees it.
    assert_same_as_unused(s, &later_private, &later_link, &private.task);
    assert_desktop_sees(s, &later_private, &kept);
    // And the one excluded before the Backup.
    assert_same_as_unused(s, &private.note, &private.link, &private.task);
    assert_desktop_sees(s, &private.note, &private.task);
    // And the one that is gone, whose history the person still has.
    assert_same_as_unused(s, &gone_private, &gone_link, &private.task);
    assert!(s.desktop.get_note(&gone_private).unwrap().is_none());
    assert!(
        !s.desktop
            .list_audit_entries(Some(&gone_private), 10, None)
            .unwrap()
            .items
            .is_empty()
    );

    // Decisions stand although the Backup had them waiting; a decided request
    // cannot be decided again, and its Task is as the Backup had it.
    assert_eq!(s.bulk_status(&approved), "applied");
    assert_eq!(s.bulk_status(&declined), "declined");
    assert_eq!(
        s.desktop.bulk_request(&approved).unwrap().decided_at,
        approved_at
    );
    assert_eq!(
        s.desktop.bulk_request(&declined).unwrap().decided_at,
        declined_at
    );
    assert_eq!(s.desktop.count_pending_bulk_requests().unwrap(), 0);
    for request in [&approved, &declined] {
        assert_eq!(
            s.desktop.approve_bulk_request(request).unwrap_err().code(),
            "unchanged"
        );
    }
    assert_eq!(s.task(&approved_task)["removed"], false);

    // The Audit log kept everything since the Backup and records the restore.
    let all = s.desktop.list_audit_entries(None, 100, None).unwrap().items;
    let restore = &all[0];
    assert_eq!(restore.action.as_str(), "backup.restore");
    assert_eq!(restore.interface.as_str(), "desktop");
    assert_eq!(restore.entity_id, backup);
    assert!(
        all.iter()
            .any(|e| e.entity_id == later && e.action.as_str() == "task.create"),
        "the Audit Entry of a Task the restore took away is kept"
    );
    assert!(
        all.iter()
            .any(|e| e.entity_id == approved && e.action.as_str() == "bulk.approve")
    );

    // Now the real CLI restores, at a terminal, with the server still up:
    // back to the Pre-restore backup, the data as it was just before.
    let before_second = s.mcp.ok("task_get", json!({"id": kept}));
    assert_eq!(before_second["item"]["revision"], 1);
    let second = s.w.restore(&outcome.pre_restore.id);
    assert_eq!(second["item"]["restored"]["kind"], "pre_restore");

    s.mcp.refused("task_list", json!({}), "database_restored");
    assert_eq!(
        s.mcp.ok("task_get", json!({"id": later}))["item"]["title"],
        "Synthetic made since the backup",
        "the Pre-restore backup held it"
    );
    // The store that did the first restore is now itself from before.
    assert_eq!(
        s.desktop.get_task(&kept).unwrap_err().code(),
        "database_restored"
    );
    s.desktop = s.w.desktop();
    assert_eq!(s.task(&kept)["title"], "Synthetic edit since the backup");
    assert_eq!(s.task(&approved_task)["removed"], true);
    assert_same_as_unused(s, &later_private, &later_link, &private.task);
    assert_eq!(s.bulk_status(&approved), "applied");
    assert_eq!(s.bulk_status(&declined), "declined");
    let newest = s.desktop.list_audit_entries(None, 1, None).unwrap().items;
    assert_eq!(newest[0].action.as_str(), "backup.restore");
    assert_eq!(newest[0].interface.as_str(), "cli");

    // And the client carries on writing.
    let current = s.mcp.ok("task_get", json!({"id": kept}));
    s.mcp.ok(
        "task_update",
        json!({"id": kept, "revision": current["item"]["revision"], "title": "Synthetic fresh write"}),
    );
    assert_eq!(s.task(&kept)["title"], "Synthetic fresh write");
    s.w.assert_sound();
    s.w.assert_only_whole_backups(&mut Default::default());
}

#[test]
fn one_database_through_the_real_cli_the_real_mcp_server_and_the_desktop() {
    let w = World::new();
    let desktop = w.desktop();
    let mcp = w.server();
    let mut s = Stage { w, desktop, mcp };
    let started = s.mcp.pid();

    let three = create_from_each_interface(&mut s);
    stale_revisions_are_refused_across_interfaces(&mut s, &three);
    audit_names_the_right_interface(&mut s, &three);
    remove_and_restore_across_interfaces(&mut s, &three);
    let private = exclude_from_ai_holds_on_every_ai_surface(&mut s);
    bulk_changes_apply_only_on_desktop_approval(&mut s);
    s.assert_server_quiet_and_offline();
    restores_while_an_mcp_server_stays_running(&mut s, &private);

    // One server, the whole way: no socket, nothing on stderr, and every line
    // it wrote was JSON (`Server::read` panics otherwise). A clean exit.
    assert_eq!(s.mcp.pid(), started);
    s.assert_server_quiet_and_offline();
    let Stage { mcp, .. } = s;
    assert_eq!(mcp.close(), Some(0));
}

/// A client that read revision 2, a restore that rewinds the Task to revision
/// 1, and the person editing it to revision 2 again: the number the client
/// holds now names the person's edit. Returns whether `write` landed, what it
/// was told, and the title the person now has.
fn a_stale_write_after_a_restore(
    write: impl FnOnce(&World, &str) -> (bool, String),
) -> (bool, String, String) {
    let w = World::new();
    let id = id_of(&w.ok(&["task", "create", "--title", "Synthetic original"]));
    let backup = w.backup();
    let read = w.ok(&[
        "task",
        "update",
        &id,
        "--revision",
        "1",
        "--title",
        "Synthetic the client read this",
    ]);
    assert_eq!(read["item"]["revision"], 2);

    let desktop = w.desktop();
    desktop.restore_backup(&backup).unwrap();
    desktop
        .patch_task(PatchTask {
            id: id.clone(),
            expected_revision: 1,
            title: Some("Synthetic the persons edit".into()),
            description: None,
        })
        .unwrap();

    let (landed, answer) = write(&w, &id);
    let title = desktop.get_task(&id).unwrap().unwrap().title;
    (landed, answer, title)
}

#[test]
#[ignore = "product defect: a restore rewinds revisions and only a running MCP server or an \
            open store remembers the Restore Epoch, so a CLI client's stale revision can match \
            again and overwrite the person's edit; the fix needs an owner decision"]
fn a_cli_write_from_before_a_restore_does_not_overwrite_the_persons_edit() {
    let (landed, answer, title) = a_stale_write_after_a_restore(|w, id| {
        let run = w.run(&[
            "task",
            "update",
            id,
            "--revision",
            "2",
            "--title",
            "Synthetic stale overwrite",
        ]);
        (run.code == 0, run.stdout)
    });
    assert!(!landed, "the stale write landed: {answer}");
    assert_eq!(title, "Synthetic the persons edit");
}

#[test]
#[ignore = "product defect: the same as the CLI case, for an MCP server started after the \
            restore: it has no earlier Restore Epoch to compare with, so a client that kept its \
            revision across a reconnect is not told; the fix needs an owner decision"]
fn an_mcp_write_from_before_a_restore_does_not_overwrite_the_persons_edit_after_a_reconnect() {
    let (landed, answer, title) = a_stale_write_after_a_restore(|w, id| {
        let mut server = w.server();
        let envelope = server.call(
            "task_update",
            json!({"id": id, "revision": 2, "title": "Synthetic stale overwrite"}),
        );
        (envelope["ok"] == true, envelope.to_string())
    });
    assert!(!landed, "the stale write landed: {answer}");
    assert_eq!(title, "Synthetic the persons edit");
}
