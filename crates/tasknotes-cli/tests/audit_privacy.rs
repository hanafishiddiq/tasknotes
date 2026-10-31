//! Ticket 11 review fix: AI audit history through the real `tasknotes`
//! process must not reveal that an excluded Note exists. Every field and
//! every cursor of `audit list`, page by page, must look exactly as if the
//! private Note had never existed; only the ids and times of the visible
//! entries may differ. The desktop side is the core store opened directly,
//! as the Tauri window opens it. Synthetic data only.

mod common;

use std::collections::HashMap;

use common::Cli;
use serde_json::Value;
use tasknotes_core::notes::UpdateNote;
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::TaskStore;

fn created_id(envelope: &Value) -> String {
    envelope["item"]["id"].as_str().unwrap().to_owned()
}

/// Every page of `audit list` at page size 1, following `next_cursor`.
fn all_pages(cli: &Cli) -> Vec<Value> {
    let mut pages = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["audit", "list", "--limit", "1"];
        if let Some(c) = cursor.as_deref() {
            args.extend(["--cursor", c]);
        }
        let page = cli.ok(&args);
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        pages.push(page);
        if cursor.is_none() {
            return pages;
        }
        assert!(pages.len() < 50, "audit pages never end");
    }
}

/// Replace what legitimately differs between two runs: each UUID becomes
/// its order of first appearance (in any string, cursors included) and each
/// time becomes 0. Everything else stays byte for byte.
fn normalize(value: &Value, ids: &mut HashMap<String, usize>) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, v)| {
                    let v = if key == "created_at" && v.is_number() {
                        Value::from(0)
                    } else {
                        normalize(v, ids)
                    };
                    (key.clone(), v)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|v| normalize(v, ids)).collect()),
        Value::String(text) => Value::String(replace_uuids(text, ids)),
        other => other.clone(),
    }
}

fn replace_uuids(text: &str, ids: &mut HashMap<String, usize>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        let candidate = rest.get(..36).filter(|c| is_uuid(c));
        if let Some(uuid) = candidate {
            let next = ids.len();
            let n = *ids.entry(uuid.to_owned()).or_insert(next);
            out.push_str(&format!("<id{n}>"));
            rest = &rest[36..];
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn normalized_history(cli: &Cli) -> Vec<Value> {
    let mut ids = HashMap::new();
    all_pages(cli)
        .iter()
        .map(|page| normalize(page, &mut ids))
        .collect()
}

#[test]
fn ai_audit_history_reads_as_if_an_excluded_note_never_existed() {
    // With a private Note: created through the CLI (so its creation was
    // once AI-visible), then excluded and edited on the desktop.
    let with_private = Cli::trusted();
    with_private.ok(&["note", "create", "--markdown", "# Shared one"]);
    let private = created_id(&with_private.ok(&["note", "create", "--markdown", "# Diary"]));
    let desktop = TaskStore::open(&with_private.db()).unwrap();
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: private.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    for edit in 1..=4 {
        let current = desktop.get_note(&private).unwrap().unwrap();
        desktop
            .update_note(UpdateNote {
                id: private.clone(),
                markdown: format!("# Diary\n\nentry {edit}"),
                expected_revision: current.revision,
            })
            .unwrap();
    }
    with_private.ok(&["note", "create", "--markdown", "# Shared two"]);

    // Without: the same visible history, the private Note never made.
    let without = Cli::trusted();
    without.ok(&["note", "create", "--markdown", "# Shared one"]);
    without.ok(&["note", "create", "--markdown", "# Shared two"]);

    let seen = normalized_history(&with_private);
    assert_eq!(seen, normalized_history(&without));
    assert_eq!(seen.len(), 2, "one entry per page: {seen:?}");
    // The person still has the whole history on the desktop.
    assert_eq!(
        desktop
            .list_audit_entries(None, 100, None)
            .unwrap()
            .items
            .len(),
        8
    );
}
