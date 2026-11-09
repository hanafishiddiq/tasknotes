//! Ticket 01: an optional icon and colour for each List and Note Folder.
//! Synthetic data only.

use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::Value;
use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::export::{ExportFormat, ExportRequest};
use tasknotes_core::note_folders::{CreateNoteFolder, NoteFolder, RenameNoteFolder};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::style::{self, SetStyle};
use tasknotes_core::tasks::{
    CreateList, List, RenameList, SCHEMA_VERSION, SetListRemoved, TaskError, TaskStore,
};

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

fn list(store: &TaskStore, name: &str) -> List {
    store.create_list(CreateList { name: name.into() }).unwrap()
}

fn folder(store: &TaskStore, name: &str) -> NoteFolder {
    store
        .create_note_folder(CreateNoteFolder {
            name: name.into(),
            parent_id: None,
        })
        .unwrap()
}

fn current_list(store: &TaskStore, id: &str) -> List {
    store
        .list_lists(100, None, true)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .unwrap()
}

/// A change naming both fields: `Some(name)` sets, `None` clears.
fn both(id: &str, revision: i64, icon: Option<&str>, color: Option<&str>) -> SetStyle {
    SetStyle {
        id: id.into(),
        expected_revision: revision,
        icon: Some(icon.map(str::to_owned)),
        color: Some(color.map(str::to_owned)),
    }
}

fn last_entry(store: &TaskStore, id: &str) -> (AuditAction, AuditOutcome, Option<String>) {
    let entry = store
        .list_audit_entries(Some(id), 1, None)
        .unwrap()
        .items
        .remove(0);
    (entry.action, entry.outcome, entry.error_code)
}

#[test]
fn the_allow_lists_hold_twelve_colours_and_a_curated_set_of_lucide_names() {
    let options = style::options();
    assert_eq!(
        options.colors,
        [
            "red", "orange", "yellow", "green", "mint", "teal", "cyan", "blue", "indigo", "purple",
            "pink", "brown"
        ]
    );
    assert!(
        (150..=400).contains(&options.icons.len()),
        "{}",
        options.icons.len()
    );
    let mut seen = std::collections::HashSet::new();
    for icon in &options.icons {
        assert!(seen.insert(icon), "{icon} is listed twice");
        assert!(
            !icon.is_empty()
                && icon
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !icon.starts_with('-')
                && !icon.ends_with('-'),
            "{icon} is not a Lucide name"
        );
    }
    // The default looks stay choosable on purpose.
    assert!(options.icons.iter().any(|icon| icon == "folder"));
    assert!(style::is_icon("folder") && !style::is_icon("Folder") && !style::is_icon("nope"));
    assert!(style::is_color("mint") && !style::is_color("magenta"));
}

#[test]
fn a_new_list_and_folder_keep_the_default_look() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let made = list(&store, "Synthetic list");
    assert_eq!((made.icon.as_deref(), made.color.as_deref()), (None, None));
    let made = folder(&store, "Synthetic folder");
    assert_eq!((made.icon.as_deref(), made.color.as_deref()), (None, None));
    let json = serde_json::to_value(&made).unwrap();
    assert_eq!(json["icon"], Value::Null);
    assert_eq!(json["color"], Value::Null);
}

#[test]
fn a_list_takes_an_icon_and_a_colour_in_one_revisioned_audited_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let made = list(&store, "Synthetic work");

    let styled = store
        .set_list_style(both(
            &made.id,
            made.revision,
            Some("briefcase"),
            Some("blue"),
        ))
        .unwrap();

    assert_eq!(styled.icon.as_deref(), Some("briefcase"));
    assert_eq!(styled.color.as_deref(), Some("blue"));
    assert_eq!(styled.revision, made.revision + 1);
    assert_eq!(styled.name, made.name);
    assert_eq!(current_list(&store, &made.id), styled);
    assert_eq!(
        last_entry(&store, &made.id),
        (AuditAction::ListStyle, AuditOutcome::Applied, None)
    );
    assert_eq!(AuditAction::ListStyle.as_str(), "list.style");
}

#[test]
fn a_field_left_out_keeps_its_value_and_null_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let made = list(&store, "Synthetic home");
    let styled = store
        .set_list_style(both(&made.id, made.revision, Some("house"), Some("green")))
        .unwrap();

    let recoloured = store
        .set_list_style(SetStyle {
            id: made.id.clone(),
            expected_revision: styled.revision,
            icon: None,
            color: Some(Some("orange".into())),
        })
        .unwrap();
    assert_eq!(recoloured.icon.as_deref(), Some("house"));
    assert_eq!(recoloured.color.as_deref(), Some("orange"));

    let plain_icon = store
        .set_list_style(SetStyle {
            id: made.id.clone(),
            expected_revision: recoloured.revision,
            icon: Some(None),
            color: None,
        })
        .unwrap();
    assert_eq!(plain_icon.icon, None);
    assert_eq!(plain_icon.color.as_deref(), Some("orange"));

    let default_look = store
        .set_list_style(both(&made.id, plain_icon.revision, None, None))
        .unwrap();
    assert_eq!((default_look.icon, default_look.color), (None, None));
}

#[test]
fn unknown_names_stale_revisions_and_no_op_writes_are_refused_and_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let desktop = store_at(&db);
    let ai = TaskStore::open_as(&db, Interface::Cli).unwrap();
    let made = list(&desktop, "Synthetic list");
    let refused = |result: Result<List, TaskError>, code: &str| {
        let error = result.unwrap_err();
        assert_eq!(error.code(), code, "{error}");
    };

    refused(
        ai.set_list_style(both(&made.id, made.revision, Some("not-an-icon"), None)),
        "invalid_input",
    );
    refused(
        ai.set_list_style(both(&made.id, made.revision, None, Some("Blue"))),
        "invalid_input",
    );
    refused(
        ai.set_list_style(both(&made.id, made.revision, Some("FOLDER"), None)),
        "invalid_input",
    );
    refused(
        ai.set_list_style(SetStyle {
            id: made.id.clone(),
            expected_revision: made.revision,
            icon: None,
            color: None,
        }),
        "invalid_input",
    );
    refused(
        ai.set_list_style(both(&made.id, made.revision, None, None)),
        "unchanged",
    );
    refused(
        ai.set_list_style(both(&made.id, made.revision + 1, Some("star"), None)),
        "revision_conflict",
    );
    // A stale write is recorded as refused, as every revisioned write is.
    assert_eq!(
        last_entry(&ai, &made.id),
        (
            AuditAction::ListStyle,
            AuditOutcome::Rejected,
            Some("revision_conflict".into())
        )
    );
    refused(
        ai.set_list_style(both(
            "6f1c5a3e-0000-4000-8000-000000000000",
            1,
            Some("star"),
            None,
        )),
        "not_found",
    );
    assert_eq!(current_list(&desktop, &made.id), made);

    let removed = desktop
        .set_list_removed(SetListRemoved {
            id: made.id.clone(),
            expected_revision: made.revision,
            removed: true,
        })
        .unwrap();
    refused(
        ai.set_list_style(both(&made.id, removed.revision, Some("star"), None)),
        "removed",
    );
}

#[test]
fn the_same_look_again_is_unchanged_and_the_cli_can_style() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let ai = TaskStore::open_as(&db, Interface::Mcp).unwrap();
    let made = list(&ai, "Synthetic list");
    let styled = ai
        .set_list_style(both(&made.id, made.revision, Some("star"), Some("yellow")))
        .unwrap();
    let again = ai.set_list_style(both(
        &made.id,
        styled.revision,
        Some("star"),
        Some("yellow"),
    ));
    assert_eq!(again.unwrap_err().code(), "unchanged");
    let entry = ai
        .list_audit_entries(Some(&made.id), 10, None)
        .unwrap()
        .items
        .into_iter()
        .find(|entry| {
            entry.outcome == AuditOutcome::Applied && entry.action == AuditAction::ListStyle
        })
        .unwrap();
    assert_eq!(entry.interface, Interface::Mcp);
}

#[test]
fn a_note_folder_takes_an_icon_and_a_colour_with_the_same_rules() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let made = folder(&store, "Synthetic recipes");

    let styled = store
        .set_note_folder_style(both(
            &made.id,
            made.revision,
            Some("chef-hat"),
            Some("pink"),
        ))
        .unwrap();
    assert_eq!(styled.icon.as_deref(), Some("chef-hat"));
    assert_eq!(styled.color.as_deref(), Some("pink"));
    assert_eq!(styled.revision, made.revision + 1);
    assert_eq!(store.get_note_folder(&made.id).unwrap().unwrap(), styled);
    let listed = store.list_note_folders(None, 10, None).unwrap().items;
    assert_eq!(listed, [styled.clone()]);
    assert_eq!(
        last_entry(&store, &made.id),
        (AuditAction::FolderStyle, AuditOutcome::Applied, None)
    );
    assert_eq!(AuditAction::FolderStyle.as_str(), "folder.style");

    let wrong = store.set_note_folder_style(both(&made.id, styled.revision, Some("x"), None));
    assert_eq!(wrong.unwrap_err().code(), "invalid_input");
    let stale = store.set_note_folder_style(both(&made.id, made.revision, None, None));
    assert_eq!(stale.unwrap_err().code(), "revision_conflict");

    let removed = store
        .set_note_folder_removed(SetRemoved {
            id: made.id.clone(),
            expected_revision: styled.revision,
            removed: true,
        })
        .unwrap();
    let hidden = store.set_note_folder_style(both(&made.id, removed.revision, None, None));
    assert_eq!(hidden.unwrap_err().code(), "removed");
}

#[test]
fn restoring_a_removed_list_or_note_folder_keeps_its_style() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let made = list(&store, "Synthetic list");
    let styled = store
        .set_list_style(both(&made.id, made.revision, Some("star"), Some("teal")))
        .unwrap();
    let removed = store
        .set_list_removed(SetListRemoved {
            id: made.id.clone(),
            expected_revision: styled.revision,
            removed: true,
        })
        .unwrap();
    assert_eq!(
        (removed.icon.as_deref(), removed.color.as_deref()),
        (Some("star"), Some("teal"))
    );
    let restored = store
        .set_list_removed(SetListRemoved {
            id: made.id.clone(),
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();
    assert!(!restored.removed);
    assert_eq!(
        (restored.icon.as_deref(), restored.color.as_deref()),
        (Some("star"), Some("teal"))
    );
    assert_eq!(current_list(&store, &made.id), restored);

    let made = folder(&store, "Synthetic folder");
    let styled = store
        .set_note_folder_style(both(
            &made.id,
            made.revision,
            Some("chef-hat"),
            Some("pink"),
        ))
        .unwrap();
    let removed = store
        .set_note_folder_removed(SetRemoved {
            id: made.id.clone(),
            expected_revision: styled.revision,
            removed: true,
        })
        .unwrap();
    let restored = store
        .set_note_folder_removed(SetRemoved {
            id: made.id.clone(),
            expected_revision: removed.revision,
            removed: false,
        })
        .unwrap();
    assert!(!restored.removed);
    assert_eq!(
        (restored.icon.as_deref(), restored.color.as_deref()),
        (Some("chef-hat"), Some("pink"))
    );
    assert_eq!(store.get_note_folder(&made.id).unwrap().unwrap(), restored);
}

#[test]
fn emoji_names_stay_as_typed_and_a_rename_keeps_the_look() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let made = list(&store, "🏠 Synthetic home ✨");
    let styled = store
        .set_list_style(both(&made.id, made.revision, Some("house"), Some("teal")))
        .unwrap();
    assert_eq!(styled.name, "🏠 Synthetic home ✨");
    let renamed = store
        .rename_list(RenameList {
            id: made.id.clone(),
            expected_revision: styled.revision,
            name: "🏡 Synthetic house".into(),
        })
        .unwrap();
    assert_eq!(renamed.name, "🏡 Synthetic house");
    assert_eq!(renamed.icon.as_deref(), Some("house"));
    assert_eq!(renamed.color.as_deref(), Some("teal"));

    let made = folder(&store, "📚 Synthetic reading");
    let styled = store
        .set_note_folder_style(both(&made.id, made.revision, Some("book-open"), None))
        .unwrap();
    let renamed = store
        .rename_note_folder(RenameNoteFolder {
            id: made.id.clone(),
            expected_revision: styled.revision,
            name: "📖 Synthetic books".into(),
        })
        .unwrap();
    assert_eq!(renamed.name, "📖 Synthetic books");
    assert_eq!(renamed.icon.as_deref(), Some("book-open"));
}

/// Puts a database back exactly as schema v17 left it.
fn back_to_v17(db: &Path) {
    let conn = Connection::open(db).unwrap();
    conn.execute_batch(
        "ALTER TABLE lists DROP COLUMN icon;
         ALTER TABLE lists DROP COLUMN color;
         ALTER TABLE note_folders DROP COLUMN icon;
         ALTER TABLE note_folders DROP COLUMN color;
         PRAGMA user_version = 17;",
    )
    .unwrap();
}

fn stamps(db: &Path, table: &str) -> Vec<(String, i64, i64)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare(&format!(
            "SELECT id, revision, updated_at FROM {table} ORDER BY id"
        ))
        .unwrap();
    statement
        .query_map(params![], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn upgrading_from_v17_gives_everything_the_default_look_and_changes_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("upgrade.db");
    let store = store_at(&db);
    let a = list(&store, "Synthetic list");
    let f = folder(&store, "Synthetic folder");
    // Styled, then rolled back to v17, which has nowhere to keep a Style.
    store
        .set_list_style(both(&a.id, a.revision, Some("star"), Some("red")))
        .unwrap();
    store
        .set_note_folder_style(both(&f.id, f.revision, Some("book"), Some("blue")))
        .unwrap();
    list(&store, "Synthetic plain list");
    drop(store);
    back_to_v17(&db);
    let (lists_before, folders_before) = (stamps(&db, "lists"), stamps(&db, "note_folders"));

    let store = store_at(&db);

    assert_eq!(SCHEMA_VERSION, 19);
    let read = current_list(&store, &a.id);
    assert_eq!((read.icon, read.color), (None, None));
    let read = store.get_note_folder(&f.id).unwrap().unwrap();
    assert_eq!((read.icon, read.color), (None, None));
    let version: i64 = Connection::open(&db)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    // Every older row, styled before the rollback or never, is NULL in both columns.
    let conn = Connection::open(&db).unwrap();
    for table in ["lists", "note_folders"] {
        let (rows, styled): (i64, i64) = conn
            .query_row(
                &format!("SELECT count(*), count(icon) + count(color) FROM {table}"),
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(rows > 0, "{table}");
        assert_eq!(styled, 0, "{table}");
    }
    assert_eq!(stamps(&db, "lists"), lists_before);
    assert_eq!(stamps(&db, "note_folders"), folders_before);
}

#[test]
fn json_exports_include_each_icon_and_colour() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let plain = list(&store, "Synthetic plain");
    let styled = list(&store, "Synthetic styled");
    store
        .set_list_style(both(
            &styled.id,
            styled.revision,
            Some("rocket"),
            Some("purple"),
        ))
        .unwrap();
    let f = folder(&store, "Synthetic folder");
    store
        .set_note_folder_style(both(&f.id, f.revision, None, Some("brown")))
        .unwrap();

    let json = dir.path().join("export.json");
    store
        .export_data(ExportRequest {
            format: ExportFormat::Json,
            destination: json.clone(),
            include_private_notes: false,
        })
        .unwrap();
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    let find = |key: &str, id: &str| {
        doc[key]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == id)
            .unwrap()
            .clone()
    };
    let record = find("lists", &plain.id);
    assert_eq!(
        (&record["icon"], &record["color"]),
        (&Value::Null, &Value::Null)
    );
    let record = find("lists", &styled.id);
    assert_eq!(record["icon"], "rocket");
    assert_eq!(record["color"], "purple");
    let record = find("note_folders", &f.id);
    assert_eq!(
        (&record["icon"], &record["color"]),
        (&Value::Null, &Value::from("brown"))
    );
}

#[test]
fn the_desktop_sends_a_style_change_as_camel_case_with_null_to_clear() {
    let change: SetStyle = serde_json::from_value(serde_json::json!({
        "id": "x", "expectedRevision": 3, "icon": null, "color": "red"
    }))
    .unwrap();
    assert_eq!(change.icon, Some(None));
    assert_eq!(change.color, Some(Some("red".into())));
    let change: SetStyle =
        serde_json::from_value(serde_json::json!({"id": "x", "expectedRevision": 3})).unwrap();
    assert_eq!((change.icon, change.color), (None, None));
}
