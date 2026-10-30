//! The recovery drill a person would run, through the real CLI at a real
//! terminal (a restore refuses anything else): damaged backups are refused
//! with the live data untouched, a good one comes back exactly, and its
//! Pre-restore backup brings back exactly what it replaced. Synthetic data.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use common::{World, assert_refused, assert_same_content, id_of, sha256_hex, text};
use rusqlite::Connection;
use serde_json::{Value, json};
use tasknotes_core::notes::CreateNote;
use tasknotes_core::privacy::SetNoteExcludeFromAi;

/// Every Task and Note as the real CLI shows them, in pages of 100: the view
/// that must also survive, along with `World::content`'s raw rows.
fn cli_view(w: &World) -> Value {
    let mut notes = Vec::new();
    for summary in w.ok(&["note", "list", "--limit", "100"])["items"]
        .as_array()
        .unwrap()
    {
        notes.push(w.ok(&["note", "get", text(&summary["id"])])["item"].clone());
    }
    let tasks = w.ok(&["task", "list", "--limit", "100"])["items"].clone();
    let mut history = Vec::new();
    for task in tasks.as_array().unwrap() {
        history.push(w.ok(&["task", "history", text(&task["id"])])["items"].clone());
    }
    json!({
        "tasks": tasks,
        "removed": w.ok(&["task", "list", "--removed", "--limit", "100"])["items"],
        "history": history,
        "notes": notes,
        "removed_view": w.ok(&["removed", "list", "--limit", "100"])["items"],
        "lists": w.ok(&["list", "list", "--include-removed"])["items"],
        "folders": w.ok(&["folder", "list"])["items"],
    })
}

fn snapshot_of(w: &World, id: &str) -> PathBuf {
    w.backups().join(format!("{id}.sqlite3"))
}

fn manifest_of(w: &World, id: &str) -> PathBuf {
    w.backups().join(format!("{id}.json"))
}

/// A careless copy tool or a hand edit: the manifest made to agree with
/// whatever the snapshot now holds, so only opening it can tell.
fn reseal(w: &World, id: &str) {
    let bytes = std::fs::read(snapshot_of(w, id)).unwrap();
    let path = manifest_of(w, id);
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    manifest["sha256"] = sha256_hex(&bytes).into();
    manifest["sizeBytes"] = bytes.len().into();
    std::fs::write(path, manifest.to_string()).unwrap();
}

/// Scribble over the end of one index page: table reads still work there, and
/// only an integrity check notices.
fn damage_an_index_page(path: &PathBuf) {
    let root: usize = Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = 'tasks_created_order'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut bytes = std::fs::read(path).unwrap();
    let page_end = root * 4096;
    bytes[page_end - 64..page_end].fill(0x5A);
    std::fs::write(path, bytes).unwrap();
}

/// A restore of `id` is refused with this exit and code, and leaves everything
/// as it was: the content, the Backups, the Audit log, a window opened before.
fn assert_refused_and_untouched(w: &World, id: &str, exit: i32, code: &str, damage: &str) {
    let window = w.desktop();
    let content = w.content();
    let view = cli_view(w);
    let backups: BTreeSet<String> = common::file_names(&w.backups()).into_iter().collect();
    let audit = window.list_audit_entries(None, 100, None).unwrap().items;

    let args = ["backup", "restore", id];
    let run = w.at_terminal(&args, "restore");
    assert_refused(&run, &args, exit, code);

    assert_same_content(&w.content(), &content, damage);
    assert_eq!(cli_view(w), view, "{damage}: the CLI's view");
    let after: BTreeSet<String> = common::file_names(&w.backups()).into_iter().collect();
    assert_eq!(
        after, backups,
        "{damage}: no Pre-restore backup, no staging copy"
    );
    assert_eq!(
        window.list_audit_entries(None, 100, None).unwrap().items,
        audit,
        "{damage}: nothing recorded; a window open before carries on"
    );
    w.assert_sound();
}

#[test]
fn a_recovery_drill_through_the_real_cli_is_a_round_trip_and_destroys_nothing() {
    let w = World::new();

    // A life's worth of synthetic data, in every shape a restore must keep.
    let ready = id_of(&w.ok(&["task", "create", "--title", "Synthetic ready errand"]));
    let started = id_of(&w.ok(&[
        "task",
        "create",
        "--title",
        "Synthetic started errand",
        "--description",
        "Synthetic long description",
    ]));
    w.ok(&[
        "task",
        "progress",
        &started,
        "--revision",
        "1",
        "--progress",
        "35",
        "--next-step",
        "Synthetic next step",
        "--note",
        "Synthetic progress note",
    ]);
    w.ok(&[
        "task",
        "details",
        &started,
        "--revision",
        "2",
        "--tag",
        "synthetic-tag",
    ]);
    let finished = id_of(&w.ok(&["task", "create", "--title", "Synthetic finished errand"]));
    w.ok(&[
        "task",
        "change",
        &finished,
        "--revision",
        "1",
        "--status",
        "completed",
    ]);
    let thrown = id_of(&w.ok(&["task", "create", "--title", "Synthetic removed errand"]));
    w.ok(&["task", "remove", &thrown, "--revision", "1"]);
    let list = id_of(&w.ok(&["list", "create", "--name", "Synthetic list"]));
    let listed = id_of(&w.ok(&[
        "task",
        "create",
        "--title",
        "Synthetic listed errand",
        "--list",
        &list,
    ]));
    let folder = id_of(&w.ok(&["folder", "create", "--name", "Synthetic folder"]));
    let shared = id_of(&w.ok(&[
        "note",
        "create",
        "--markdown",
        "# Synthetic shared note\n\nzzsharedword and **bold** text\n\n- [ ] a checklist",
        "--folder",
        &folder,
    ]));
    w.ok(&["link", "create", "--task", &started, "--note", &shared]);
    let desktop = w.desktop();
    let private = desktop
        .create_note(CreateNote {
            markdown: "# Synthetic private note\n\nzzprivateword".into(),
            folder_id: None,
        })
        .unwrap();
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    drop(desktop);

    let good = w.backup();
    let at_backup = w.content();
    let view_at_backup = cli_view(&w);
    let audit_at_backup = w
        .desktop()
        .list_audit_entries(None, 100, None)
        .unwrap()
        .items;
    assert!(
        audit_at_backup
            .iter()
            .all(|e| e.action.as_str() != "backup.restore")
    );

    // Life goes on after the Backup, and this is what a restore must not lose.
    w.ok(&[
        "task",
        "update",
        &ready,
        "--revision",
        "1",
        "--title",
        "Synthetic renamed zzlaterword",
    ]);
    w.ok(&["task", "restore", &thrown, "--revision", "2"]);
    w.ok(&["task", "remove", &listed, "--revision", "1"]);
    let later = id_of(&w.ok(&["task", "create", "--title", "Synthetic made later"]));
    w.ok(&[
        "task",
        "progress",
        &started,
        "--revision",
        "3",
        "--progress",
        "70",
        "--note",
        "Synthetic later progress",
    ]);
    w.ok(&[
        "note",
        "update",
        &shared,
        "--revision",
        "1",
        "--markdown",
        "# Synthetic shared note\n\nchanged after the backup",
    ]);
    let later_note = id_of(&w.ok(&["note", "create", "--markdown", "# Synthetic later note"]));
    let window = w.desktop();
    window
        .create_note(CreateNote {
            markdown: "# Synthetic later private".into(),
            folder_id: None,
        })
        .map(|note| {
            window
                .set_note_exclude_from_ai(SetNoteExcludeFromAi {
                    id: note.id,
                    exclude_from_ai: true,
                })
                .unwrap()
        })
        .unwrap();
    let before_drill = w.content();
    let view_before_drill = cli_view(&w);
    assert_ne!(
        view_before_drill, view_at_backup,
        "the drill has work to do"
    );

    // Without the person at a terminal, or without their word: refused.
    let args = ["backup", "restore", good.as_str()];
    assert_refused(&w.run(&args), &args, 6, "not_trusted");
    assert_refused(&w.at_terminal(&args, "no"), &args, 6, "not_trusted");
    assert_same_content(&w.content(), &before_drill, "an unconfirmed restore");

    // Damaged backups, each on its own copy of the good one's bytes.
    let original = std::fs::read(snapshot_of(&w, &good)).unwrap();
    let manifest = std::fs::read(manifest_of(&w, &good)).unwrap();
    let put_back = || {
        std::fs::write(snapshot_of(&w, &good), &original).unwrap();
        std::fs::write(manifest_of(&w, &good), &manifest).unwrap();
    };

    let mut flipped = original.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 0xFF;
    std::fs::write(snapshot_of(&w, &good), &flipped).unwrap();
    assert_refused_and_untouched(&w, &good, 5, "invalid_backup", "one flipped byte");
    put_back();

    std::fs::write(snapshot_of(&w, &good), &original[..original.len() / 3]).unwrap();
    assert_refused_and_untouched(&w, &good, 5, "invalid_backup", "a truncated snapshot");
    put_back();

    damage_an_index_page(&snapshot_of(&w, &good));
    reseal(&w, &good);
    assert_refused_and_untouched(&w, &good, 5, "invalid_backup", "a damaged page, resealed");
    put_back();

    std::fs::write(snapshot_of(&w, &good), b"synthetic bytes, not a database").unwrap();
    reseal(&w, &good);
    assert_refused_and_untouched(&w, &good, 5, "invalid_backup", "not a database, resealed");
    put_back();

    std::fs::write(manifest_of(&w, &good), b"{ not json").unwrap();
    assert_refused_and_untouched(&w, &good, 3, "not_found", "an unreadable manifest");
    put_back();

    std::fs::remove_file(snapshot_of(&w, &good)).unwrap();
    assert_refused_and_untouched(&w, &good, 3, "not_found", "a missing snapshot");
    put_back();

    assert_refused_and_untouched(&w, "../../tasknotes", 3, "not_found", "a path as an id");

    // The real thing: the good Backup comes back, exactly.
    let first = w.restore(&good);
    assert_eq!(first["item"]["restored"]["id"], good.as_str());
    assert_eq!(first["item"]["restored"]["kind"], "manual");
    assert_eq!(first["item"]["pre_restore"]["kind"], "pre_restore");
    assert_same_content(&w.content(), &at_backup, "after restoring the good Backup");
    assert_eq!(
        cli_view(&w),
        view_at_backup,
        "the CLI reads the Backup's data"
    );
    w.assert_sound();

    // A store from before must reload; the search index came back with it.
    assert_eq!(
        window.get_task(&ready).unwrap_err().code(),
        "database_restored"
    );
    drop(window);
    let found = |word: &str| -> usize {
        let tasks = w.ok(&["task", "list", "--text", word])["items"]
            .as_array()
            .unwrap()
            .len();
        let notes = w.ok(&["note", "list", "--text", word])["items"]
            .as_array()
            .unwrap()
            .len();
        tasks + notes
    };
    assert_eq!(found("zzlaterword"), 0);
    assert_eq!(found("zzsharedword"), 1);
    assert_eq!(found("zzprivateword"), 0, "private through the restore");
    w.refused(&["task", "get", &later], 3, "not_found");
    w.refused(&["note", "get", &later_note], 3, "not_found");
    w.refused(&["note", "get", &private.id], 3, "not_found");

    // Second thoughts: the Pre-restore backup brings back what it replaced.
    let pre_restore = text(&first["item"]["pre_restore"]["id"]).to_owned();
    let second = w.restore(&pre_restore);
    assert_eq!(second["item"]["restored"]["id"], pre_restore.as_str());
    assert_same_content(&w.content(), &before_drill, "the round trip, row for row");
    assert_eq!(
        cli_view(&w),
        view_before_drill,
        "the round trip, through the CLI"
    );
    w.assert_sound();
    assert_eq!(found("zzlaterword"), 1);
    assert_eq!(found("zzsharedword"), 0);
    assert_eq!(found("zzprivateword"), 0, "private through both restores");
    let desktop = w.desktop();
    assert!(
        desktop
            .get_note(&private.id)
            .unwrap()
            .unwrap()
            .exclude_from_ai
    );

    // Nothing recorded was lost, and both restores are on record as the CLI's.
    let audit = desktop.list_audit_entries(None, 100, None).unwrap().items;
    for entry in &audit_at_backup {
        assert!(audit.contains(entry), "an Audit Entry was lost: {entry:?}");
    }
    let restores: Vec<(&str, &str)> = audit
        .iter()
        .filter(|e| e.action.as_str() == "backup.restore")
        .map(|e| (e.interface.as_str(), e.entity_id.as_str()))
        .collect();
    assert_eq!(
        restores,
        [("cli", pre_restore.as_str()), ("cli", good.as_str())]
    );
    assert!(
        audit.iter().any(|e| e.entity_id == later),
        "the later Task's history survived both"
    );

    // Each Pre-restore backup is still there, and the drill left no debris.
    let kinds: Vec<String> = w
        .assert_only_whole_backups(&mut BTreeSet::new())
        .iter()
        .map(|b| format!("{:?}", b.kind))
        .collect();
    let pre_restores = kinds.iter().filter(|k| *k == "PreRestore").count();
    assert_eq!(pre_restores, 2, "{kinds:?}");
    assert_eq!(w.leftovers(), Vec::<String>::new());

    // And the data is alive: a new write, a new Backup.
    let revision = w.ok(&["task", "get", &later])["item"]["revision"]
        .as_i64()
        .unwrap()
        .to_string();
    w.ok(&[
        "task",
        "update",
        &later,
        "--revision",
        &revision,
        "--title",
        "Synthetic still writing",
    ]);
    w.backup();
}
