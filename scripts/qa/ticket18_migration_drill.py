#!/usr/bin/env python3
"""Upgrade databases really made by old Tasknotes code to the current schema.

For each old commit: build its core from git, write synthetic records with it,
then open the file with the current CLI and compare every record before and
after. Synthetic data in temporary directories only.

Usage: ticket18_migration_drill.py CURRENT_TASKNOTES_BIN OUT_JSON
"""
import json
import os
import re
import sqlite3
import subprocess
import sys
import tempfile

CURRENT_SCHEMA = 15

# (commit, schema the commit writes, label)
OLD = [
    ("b799cce", 1, "Ticket 01"),
    ("bc8ac50", 2, "Ticket 02"),
    ("2937c9a", 3, "Ticket 03"),
    ("80de645", 5, "Ticket 05, today's main"),
    ("a98dc16", 6, "Ticket 06"),
]

GENERATOR = """
use tasknotes_core::tasks::*;
fn main() {{
    let store = TaskStore::open(std::path::Path::new(&std::env::args().nth(1).unwrap())).unwrap();
    for i in 0..3 {{
        let task = store.create_task(CreateTask {{
            title: format!("Synthetic drill{{i}} title"),
            description: format!("Synthetic drillbody{{i}} — été 日本語"),
        }}).unwrap();
        {extras}
    }}
    {notes}
}}
"""
CHANGE = """
        let task = store.change_task(ChangeTask {
            id: task.id, expected_revision: task.revision,
            status: Some(WorkStatus::InProgress), priority: Some(Priority::High), progress: Some(40),
        }).unwrap();
"""
PROGRESS = """
        let task = store.record_progress(RecordProgress {
            id: task.id, expected_revision: task.revision, status: None, progress: Some(60),
            next_step: Some("Synthetic drillnext".into()), note: Some(format!("Synthetic drillnote{i}")),
        }).unwrap();
"""
NOTES = """
    store.create_note(tasknotes_core::notes::CreateNote {
        markdown: "# Synthetic drill note\\n\\nSynthetic drillnotebody".into(),
    }).unwrap();
"""


def build_old_database(commit, level, work):
    source = os.path.join(work, "old")
    os.makedirs(source)
    archive = subprocess.run(["git", "archive", commit, "crates/tasknotes-core"], capture_output=True, check=True)
    subprocess.run(["tar", "-x", "-C", source], input=archive.stdout, check=True)
    generator = os.path.join(source, "generator")
    os.makedirs(os.path.join(generator, "src"))
    with open(os.path.join(generator, "Cargo.toml"), "w") as handle:
        handle.write('[package]\nname = "generator"\nversion = "0.0.0"\nedition = "2024"\n\n'
                     '[dependencies]\ntasknotes-core = { path = "../crates/tasknotes-core" }\n\n[workspace]\n')
    extras = ""
    if level >= 2:
        extras += CHANGE
    if level >= 3:
        extras += PROGRESS
    code = GENERATOR.format(extras="let _ = task;" if not extras else extras + "let _ = task;",
                            notes=NOTES if level >= 6 else "")
    with open(os.path.join(generator, "src", "main.rs"), "w") as handle:
        handle.write(code)
    database = os.path.join(work, "data", "tasknotes.db")
    os.makedirs(os.path.dirname(database))
    subprocess.run(["cargo", "run", "--quiet", "--", database], cwd=generator, check=True, capture_output=True)
    return database


def rows(database, sql):
    connection = sqlite3.connect(database)
    try:
        return [list(row) for row in connection.execute(sql)]
    finally:
        connection.close()


def tables(database):
    return {row[0] for row in rows(database, "SELECT name FROM sqlite_master WHERE type='table'")}


def snapshot(database):
    found = {"tasks": rows(database, "SELECT id, title, description, status, priority, progress, revision, "
                                     "created_at, updated_at FROM tasks ORDER BY id")}
    if "progress_entries" in tables(database):
        found["progress"] = rows(database, "SELECT task_id, note, next_step, progress FROM progress_entries "
                                           "ORDER BY sequence")
    if "notes" in tables(database):
        found["notes"] = rows(database, "SELECT id, markdown, title, revision FROM notes ORDER BY id")
    return found


def schema(database):
    clean = lambda sql: re.sub(r"\s+", " ", sql or "").strip()
    return sorted(
        [kind, name, clean(sql)]
        for kind, name, sql in rows(database, "SELECT type, name, sql FROM sqlite_master")
        if not name.startswith("sqlite_")
    )


def cli(binary, data, *words):
    done = subprocess.run([binary, "--data-dir", data, *words], capture_output=True, text=True)
    return json.loads(done.stdout)


def grant(data):
    with open(os.path.join(data, "ai-access.json"), "w") as handle:
        json.dump({"schema_version": 1, "cli": True}, handle)


def drill(binary, commit, level, label, fresh_schema):
    with tempfile.TemporaryDirectory(prefix="tasknotes-ticket18-") as work:
        database = build_old_database(commit, level, work)
        data = os.path.dirname(database)
        version_before = rows(database, "PRAGMA user_version")[0][0]
        before = snapshot(database)
        grant(data)
        listed = cli(binary, data, "task", "list")
        version_after = rows(database, "PRAGMA user_version")[0][0]
        after = snapshot(database)
        words = ["drill0", "drillbody1"]
        if level >= 3:
            words += ["drillnote2", "drillnext"]
        found = {word: len(cli(binary, data, "task", "list", "--text", word).get("items", [])) for word in words}
        notes = cli(binary, data, "note", "list", "--text", "drillnotebody").get("items", []) if level >= 6 else None
        first = listed["items"][0]
        write = cli(binary, data, "task", "update", first["id"], "--revision", str(first["revision"]),
                    "--title", "Synthetic after the upgrade")
        backup = cli(binary, data, "backup", "create")
        comparable = {key: value for key, value in before.items()}
        result = {
            "commit": commit,
            "label": label,
            "version_before": version_before,
            "version_after": version_after,
            "tasks": len(before["tasks"]),
            "records_unchanged": all(after[key] == comparable[key] for key in comparable),
            "search_hits": found,
            "note_search_hits": None if notes is None else len(notes),
            "integrity_check": rows(database, "PRAGMA integrity_check")[0][0],
            "foreign_key_problems": len(rows(database, "PRAGMA foreign_key_check")),
            "write_after_upgrade_ok": write.get("ok"),
            "backup_after_upgrade_ok": backup.get("ok"),
            "schema_differences_from_a_new_database": [
                entry for entry in schema(database) if entry not in fresh_schema
            ] + [entry for entry in fresh_schema if entry not in schema(database)],
        }
        return result


def main():
    binary, out = sys.argv[1:3]
    with tempfile.TemporaryDirectory(prefix="tasknotes-ticket18-") as fresh:
        grant(fresh)
        cli(binary, fresh, "task", "list")
        fresh_schema = schema(os.path.join(fresh, "tasknotes.db"))
    results = [drill(binary, commit, level, label, fresh_schema) for commit, level, label in OLD]
    with open(out, "w") as handle:
        json.dump(results, handle, indent=2, ensure_ascii=False)
    print(json.dumps(results, indent=2, ensure_ascii=False))
    ok = all(
        r["records_unchanged"] and r["integrity_check"] == "ok" and not r["foreign_key_problems"]
        and r["version_after"] == CURRENT_SCHEMA and r["note_search_hits"] in (None, 1)
        and r["write_after_upgrade_ok"] and r["backup_after_upgrade_ok"]
        and not r["schema_differences_from_a_new_database"] and all(r["search_hits"].values())
        for r in results
    )
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
