//! `tasknotes`: a local, structured JSON command line over the shared core
//! (ADR-0001). It holds no business rules: validation, revisions, Removed and
//! Audit Entries all live in `tasknotes-core`. It never listens on a port.

pub mod access;
mod args;
pub mod bulk;
pub mod contract;
mod data;
mod notes;

use std::ffi::OsString;
use std::path::Path;

use serde_json::{Value, json};
use tasknotes_core::ai_access;
use tasknotes_core::audit::Interface;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::style::{self, SetStyle};
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateSubList, CreateTask, Difficulty, DifficultyFilter, List,
    MoveList, MoveTask, PatchTask, PatchTaskDetails, Priority, RecordProgress, RenameList,
    ReorderList, ReorderTask, SetListRemoved, SortMode, Task, TaskError, TaskQuery, TaskScope,
    TaskStore, TaskView, WorkStatus,
};

use access::Environment;
use args::{Parsed, Spec, parse};
use contract::CliError;

/// What the process prints on stdout and exits with.
#[derive(Debug)]
pub struct Outcome {
    pub exit_code: i32,
    pub body: Value,
}

/// Shown at the terminal by `access grant`; the person types `allow`.
fn grant_prompt(interface: Interface) -> String {
    let through = match interface {
        Interface::Mcp => "MCP clients you set up to start tasknotes-mcp",
        _ => "the tasknotes command",
    };
    format!(
        "Allow local AI clients to read and change your Tasks and Notes \
         (except Notes marked Exclude from AI) through {through}? \
         Type allow to confirm: "
    )
}

/// Run one command. `confirm` shows the person at the terminal a prompt and
/// returns what they typed; it is only called on an interactive terminal,
/// by `access grant` and `backup restore`.
pub fn run(
    args: &[OsString],
    env: &Environment,
    confirm: impl FnOnce(&str) -> std::io::Result<String>,
) -> Outcome {
    let mut warnings = Vec::new();
    let result = args
        .iter()
        .map(|arg| {
            arg.to_str()
                .map(str::to_owned)
                .ok_or_else(|| CliError::invalid("Every argument must be valid UTF-8 text"))
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|args| dispatch(&args, env, confirm, &mut warnings));
    let mut outcome = match result {
        Ok(body) => Outcome { exit_code: 0, body },
        Err(error) => Outcome {
            exit_code: error.exit_code(),
            body: contract::failure(&error),
        },
    };
    contract::add_warnings(&mut outcome.body, warnings);
    outcome
}

fn dispatch(
    args: &[String],
    env: &Environment,
    confirm: impl FnOnce(&str) -> std::io::Result<String>,
    warnings: &mut Vec<Value>,
) -> Result<Value, CliError> {
    let (data_flag, rest) = match args {
        [flag, dir, rest @ ..] if flag == "--data-dir" => (Some(dir.as_str()), rest),
        _ => (None, args),
    };
    let (noun, verb, words) = match rest {
        [noun, verb, words @ ..] => (noun.as_str(), verb.as_str(), words),
        _ => return Err(CliError::invalid(USAGE)),
    };
    let data_dir = access::data_dir(data_flag, env)?;

    if noun == "access" {
        // `--mcp` grants or revokes local MCP clients instead of the CLI.
        let p = parse(
            words,
            &Spec {
                values: &[],
                repeated: &[],
                switches: &["mcp"],
                positionals: 0,
            },
        )?;
        let interface = if p.has("mcp") {
            Interface::Mcp
        } else {
            Interface::Cli
        };
        return match verb {
            "status" if !p.has("mcp") => Ok(access_status(&data_dir)),
            "grant" => access::grant(&data_dir, interface, env, |asked| {
                confirm(&grant_prompt(asked))
            })
            .map(|()| access_status(&data_dir)),
            "revoke" => access::revoke(&data_dir, interface).map(|()| access_status(&data_dir)),
            _ => Err(CliError::invalid(USAGE)),
        };
    }
    if !matches!(
        noun,
        "task"
            | "list"
            | "audit"
            | "note"
            | "folder"
            | "link"
            | "removed"
            | "bulk"
            | "style"
            | "backup"
            | "export"
    ) {
        return Err(CliError::invalid(USAGE));
    }
    // The core refuses before the database is opened, created or migrated.
    let store = ai_access::open_authorized(&data_dir, Interface::Cli)?;
    // Backups run while any Tasknotes process is active (spec): the first
    // command of the day takes one. A failed backup must not block the
    // command (the next process tries again); it is reported as a warning
    // in the envelope, since the contract keeps stderr empty.
    data::daily_backup(&store, warnings);
    match noun {
        "backup" => data::backup_command(&store, verb, words, env, confirm, warnings),
        "export" => data::export_command(&store, verb, words),
        "task" => task_command(&store, verb, words),
        "list" => list_command(&store, verb, words),
        "note" => notes::note_command(&store, verb, words),
        "folder" => notes::folder_command(&store, verb, words),
        "link" => notes::link_command(&store, verb, words),
        "removed" => notes::removed_command(&store, verb, words),
        "bulk" => bulk::bulk_command(&store, verb, words),
        "style" => style_command(verb, words),
        _ => audit_command(&store, verb, words),
    }
}

pub(crate) const USAGE: &str = "Usage: tasknotes [--data-dir DIR] \
    <task|list|note|folder|link|removed|audit|bulk|style|backup|export|access> <command> [id] [--options]; \
    task change ID --revision N [--status S] [--priority P] [--difficulty D|none] [--progress N] [--wont-do true|false] (a Completed Task reopens to its completed_from with --status); \
    task list [--status S] [--priority P] [--difficulty D|none] [--wont-do true|false] [--tag T] [--sort priority|newest|difficulty|custom (with --list ID or --inbox)]; \
    task reorder ID --revision N (--after ID | --first) (within its work-status group of its List's Custom Order; switches its List to Custom in the desktop); \
    list create --name N [--parent ID]; list move ID --revision N (--parent ID | --top); \
    list reorder ID --revision N (--after ID | --first) [--parent ID | --top]; \
    list|folder style ID --revision N [--icon NAME|none] [--color NAME|none]; style options; \
    folder reorder ID --revision N (--after ID | --first) [--parent ID | --top]; \
    bulk preview --action A --id ID... [--status S [--wont-do]] | bulk status REQUEST_ID (approved only in the app); \
    access grant|revoke [--mcp]";

pub(crate) const NO_FLAGS: Spec = flags(&[], 0);

fn access_status(data_dir: &Path) -> Value {
    contract::success_item(json!({
        "cli": ai_access::is_authorized(data_dir, Interface::Cli),
        "mcp": ai_access::is_authorized(data_dir, Interface::Mcp),
        "data_dir": data_dir.display().to_string(),
    }))
}

fn task_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let task = match verb {
        "list" => return task_page(store, words),
        "history" => return task_history(store, words),
        "notes" => return notes::task_notes(store, words),
        "get" => {
            let p = parse(words, &ONE_ID)?;
            store.get_task(p.id())?.ok_or(TaskError::NotFound)?
        }
        "create" => task_create(store, words)?,
        "update" => task_update(store, words)?,
        "change" => task_change(store, words)?,
        "progress" => task_progress(store, words)?,
        "details" => task_details(store, words)?,
        "move" => task_move(store, words)?,
        "reorder" => task_reorder(store, words)?,
        "remove" | "restore" => {
            let p = parse(words, &flags(&["revision"], 1))?;
            store.set_task_removed(SetRemoved {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                removed: verb == "remove",
            })?
        }
        _ => return Err(CliError::invalid(USAGE)),
    };
    Ok(contract::success_item(contract::task(&task)))
}

fn task_create(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(words, &flags(&["title", "description", "list"], 0))?;
    Ok(store.create_task_in_list(
        CreateTask {
            title: p.required("title")?.to_owned(),
            description: p.value("description").unwrap_or_default().to_owned(),
        },
        p.value("list"),
    )?)
}

/// A flag that is not given keeps its current value (the core's patch rule).
fn task_update(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(words, &flags(&["revision", "title", "description"], 1))?;
    Ok(store.patch_task(PatchTask {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        title: p.value("title").map(str::to_owned),
        description: p.value("description").map(str::to_owned),
    })?)
}

fn task_change(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(
        words,
        &flags(
            &[
                "revision",
                "status",
                "priority",
                "difficulty",
                "progress",
                "wont-do",
            ],
            1,
        ),
    )?;
    Ok(store.change_task(ChangeTask {
        wont_do: wont_do(&p)?,
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        status: p.choice("status", &WorkStatus::ALL)?,
        priority: p.choice("priority", &Priority::ALL)?,
        progress: p.number("progress")?,
        difficulty: difficulty_change(&p)?,
    })?)
}

/// `--wont-do true` marks a Task Won't Do and `--wont-do false` clears it
/// (reopening it); as a `task list` filter it keeps only those Tasks, or only
/// the others.
fn wont_do(p: &Parsed) -> Result<Option<bool>, CliError> {
    match p.value("wont-do") {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(_) => Err(CliError::invalid("--wont-do must be true or false")),
    }
}

/// `--difficulty NAME` sets the Difficulty and `--difficulty none` clears it
/// to No Difficulty; left out, it is kept.
fn difficulty_change(p: &Parsed) -> Result<Option<Option<Difficulty>>, CliError> {
    if p.value("difficulty") == Some("none") {
        return Ok(Some(None));
    }
    Ok(p.choice("difficulty", &Difficulty::ALL)?.map(Some))
}

fn task_progress(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(
        words,
        &flags(&["revision", "status", "progress", "next-step", "note"], 1),
    )?;
    Ok(store.record_progress(RecordProgress {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        status: p.choice("status", &WorkStatus::ALL)?,
        progress: p.number("progress")?,
        next_step: p.value("next-step").map(str::to_owned),
        note: p.value("note").map(str::to_owned),
    })?)
}

/// `--tag` (repeated) sets the Tags, `--no-tags` clears them; `--deadline`
/// sets the Deadline, `--no-deadline` clears it; absent flags keep the value.
fn task_details(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(
        words,
        &Spec {
            values: &["revision", "deadline"],
            repeated: &["tag"],
            switches: &["no-tags", "no-deadline"],
            positionals: 1,
        },
    )?;
    if p.has("tag") && p.has("no-tags") || p.has("deadline") && p.has("no-deadline") {
        return Err(CliError::invalid(
            "Give either --tag or --no-tags, and either --deadline or --no-deadline",
        ));
    }
    let tags = if p.has("no-tags") {
        Some(Vec::new())
    } else {
        p.has("tag").then(|| p.all("tag"))
    };
    let due_date = if p.has("no-deadline") {
        Some(None)
    } else {
        p.value("deadline").map(|d| Some(d.to_owned()))
    };
    Ok(store.patch_task_details(PatchTaskDetails {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        tags,
        due_date,
    })?)
}

fn task_move(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(
        words,
        &Spec {
            values: &["revision", "list"],
            repeated: &[],
            switches: &["inbox"],
            positionals: 1,
        },
    )?;
    let list_id = contract::destination(
        p.value("list").map(str::to_owned),
        p.has("inbox"),
        ("--list ID", "--inbox"),
    )?;
    Ok(store.move_task(MoveTask {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        list_id,
    })?)
}

/// Puts a Task directly after another of its work-status group in its
/// List's Custom Order, or first (ADR 0010). The List then shows Custom in
/// the desktop, as after a drag, so the person sees this order.
fn task_reorder(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(
        words,
        &Spec {
            values: &["revision", "after"],
            repeated: &[],
            switches: &["first"],
            positionals: 1,
        },
    )?;
    Ok(store.reorder_task(ReorderTask {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        after_id: contract::destination(
            p.value("after").map(str::to_owned),
            p.has("first"),
            ("--after ID", "--first"),
        )?,
        before_id: None,
        shown_sort: None,
    })?)
}

fn task_history(store: &TaskStore, words: &[String]) -> Result<Value, CliError> {
    let p = parse(words, &flags(&["limit", "cursor"], 1))?;
    let page = store.list_progress_entries(p.id(), p.limit()?, p.value("cursor"))?;
    Ok(contract::success_page(
        page.items.iter().map(contract::progress_entry).collect(),
        page.next_cursor,
    ))
}

fn list_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let item = |list: List| Ok(contract::success_item(contract::list(&list)));
    match verb {
        "list" => {
            let p = parse(
                words,
                &Spec {
                    values: &["limit", "cursor"],
                    repeated: &[],
                    switches: &["include-removed"],
                    positionals: 0,
                },
            )?;
            let page = store.list_lists(p.limit()?, p.value("cursor"), p.has("include-removed"))?;
            Ok(contract::success_page(
                page.items.iter().map(contract::list).collect(),
                page.next_cursor,
            ))
        }
        "create" => {
            let p = parse(words, &flags(&["name", "parent"], 0))?;
            let name = p.required("name")?.to_owned();
            item(match p.value("parent") {
                None => store.create_list(CreateList { name })?,
                Some(parent) => store.create_sub_list(CreateSubList {
                    name,
                    parent_id: parent.to_owned(),
                })?,
            })
        }
        "move" => {
            let p = parse(
                words,
                &Spec {
                    values: &["revision", "parent"],
                    repeated: &[],
                    switches: &["top"],
                    positionals: 1,
                },
            )?;
            item(store.move_list(MoveList {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                parent_id: contract::destination(
                    p.value("parent").map(str::to_owned),
                    p.has("top"),
                    ("--parent ID", "--top"),
                )?,
            })?)
        }
        "rename" => {
            let p = parse(words, &flags(&["revision", "name"], 1))?;
            item(store.rename_list(RenameList {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                name: p.required("name")?.to_owned(),
            })?)
        }
        "reorder" => {
            let p = parse(
                words,
                &Spec {
                    values: &["revision", "after", "parent"],
                    repeated: &[],
                    switches: &["first", "top"],
                    positionals: 1,
                },
            )?;
            item(store.reorder_list(ReorderList {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                after_id: contract::destination(
                    p.value("after").map(str::to_owned),
                    p.has("first"),
                    ("--after ID", "--first"),
                )?,
                parent: contract::list_parent(
                    p.value("parent").map(str::to_owned),
                    p.has("top"),
                    ("--parent ID", "--top"),
                )?,
            })?)
        }
        "style" => item(store.set_list_style(style_change(words)?)?),
        "remove" | "restore" => {
            let p = parse(words, &flags(&["revision"], 1))?;
            item(store.set_list_removed(SetListRemoved {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                removed: verb == "remove",
            })?)
        }
        _ => Err(CliError::invalid(USAGE)),
    }
}

/// `ID --revision N [--icon NAME|none] [--color NAME|none]` for a List or
/// Note Folder: a flag left out keeps its value and `none` clears it. The
/// names are checked by the core.
pub(crate) fn style_change(words: &[String]) -> Result<SetStyle, CliError> {
    let p = parse(words, &flags(&["revision", "icon", "color"], 1))?;
    let named = |flag: &str| {
        p.value(flag)
            .map(|name| (name != "none").then(|| name.to_owned()))
    };
    Ok(SetStyle {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        icon: named("icon"),
        color: named("color"),
    })
}

/// `style options`: every icon name and colour a Style may hold.
fn style_command(verb: &str, words: &[String]) -> Result<Value, CliError> {
    if verb != "options" {
        return Err(CliError::invalid(USAGE));
    }
    parse(words, &NO_FLAGS)?;
    Ok(contract::success_item(contract::to_value(style::options())))
}

fn audit_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    if verb != "list" {
        return Err(CliError::invalid(USAGE));
    }
    let p = parse(words, &flags(&["limit", "cursor", "entity"], 0))?;
    let page = store.list_audit_entries(p.value("entity"), p.limit()?, p.value("cursor"))?;
    Ok(contract::success_page(
        page.items.iter().map(contract::to_value).collect(),
        page.next_cursor,
    ))
}

pub(crate) const ONE_ID: Spec = flags(&[], 1);

pub(crate) const fn flags<'a>(values: &'a [&'a str], positionals: usize) -> Spec<'a> {
    Spec {
        values,
        repeated: &[],
        switches: &[],
        positionals,
    }
}

fn task_page(store: &TaskStore, words: &[String]) -> Result<Value, CliError> {
    let p = parse(
        words,
        &Spec {
            values: &[
                "limit",
                "cursor",
                "text",
                "status",
                "priority",
                "difficulty",
                "wont-do",
                "tag",
                "list",
                "sort",
            ],
            repeated: &[],
            switches: &["inbox", "removed"],
            positionals: 0,
        },
    )?;
    if p.has("list") && p.has("inbox") {
        return Err(CliError::invalid(
            "Give at most one of --list ID or --inbox",
        ));
    }
    let query = TaskQuery {
        text: p.value("text").unwrap_or_default().to_owned(),
        status: p.choice("status", &WorkStatus::ALL)?,
        priority: p.choice("priority", &Priority::ALL)?,
        difficulty: p.choice("difficulty", &DifficultyFilter::ALL)?,
        wont_do: wont_do(&p)?,
        tag: p.value("tag").map(str::to_owned),
        view: match (p.value("list"), p.has("inbox")) {
            (Some(id), _) => TaskView::List(id.to_owned()),
            (None, true) => TaskView::Inbox,
            (None, false) => TaskView::All,
        },
        scope: if p.has("removed") {
            TaskScope::Removed
        } else {
            TaskScope::Active
        },
        sort: p.choice("sort", &SortMode::ALL)?.unwrap_or_default(),
        task_id: None,
    };
    let page = store.query_tasks(&query, p.limit()?, p.value("cursor"))?;
    Ok(contract::success_page(
        page.items.iter().map(contract::task).collect(),
        page.next_cursor,
    ))
}
