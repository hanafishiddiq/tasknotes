//! `tasknotes`: a local, structured JSON command line over the shared core
//! (ADR-0001). It holds no business rules: validation, revisions, Removed and
//! Audit Entries all live in `tasknotes-core`. It never listens on a port.

pub mod access;
mod args;
pub mod contract;

use std::ffi::OsString;
use std::path::Path;

use serde_json::{Value, json};
use tasknotes_core::audit::Interface;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, List, MoveTask, PatchTask, PatchTaskDetails,
    RecordProgress, RenameList, SetListRemoved, SortMode, Task, TaskError, TaskQuery, TaskScope,
    TaskStore, TaskView,
};

use access::Environment;
use args::{Spec, parse};
use contract::CliError;

/// What the process prints on stdout and exits with.
#[derive(Debug)]
pub struct Outcome {
    pub exit_code: i32,
    pub body: Value,
}

/// Run one command. `confirm` asks the person at the terminal to type
/// `allow`; it is only called by `access grant` on an interactive terminal.
pub fn run(
    args: &[OsString],
    env: &Environment,
    confirm: impl FnOnce() -> std::io::Result<String>,
) -> Outcome {
    let result = args
        .iter()
        .map(|arg| {
            arg.to_str()
                .map(str::to_owned)
                .ok_or_else(|| CliError::invalid("Every argument must be valid UTF-8 text"))
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|args| dispatch(&args, env, confirm));
    match result {
        Ok(body) => Outcome { exit_code: 0, body },
        Err(error) => Outcome {
            exit_code: error.exit_code(),
            body: contract::failure(&error),
        },
    }
}

fn dispatch(
    args: &[String],
    env: &Environment,
    confirm: impl FnOnce() -> std::io::Result<String>,
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
        parse(words, &NO_FLAGS)?;
        return match verb {
            "status" => Ok(access_status(&data_dir)),
            "grant" => access::grant(&data_dir, env, confirm).map(|()| access_status(&data_dir)),
            "revoke" => access::revoke(&data_dir).map(|()| access_status(&data_dir)),
            _ => Err(CliError::invalid(USAGE)),
        };
    }
    if !matches!(noun, "task" | "list" | "audit") {
        return Err(CliError::invalid(USAGE));
    }
    // Refuse before the database is opened, created or migrated.
    access::require_trust(&data_dir)?;
    let store = TaskStore::open_as(&data_dir.join(access::DATABASE_FILE), Interface::Cli)?;
    match noun {
        "task" => task_command(&store, verb, words),
        "list" => list_command(&store, verb, words),
        _ => audit_command(&store, verb, words),
    }
}

const USAGE: &str =
    "Usage: tasknotes [--data-dir DIR] <task|list|audit|access> <command> [id] [--options]";

const NO_FLAGS: Spec = flags(&[], 0);

fn access_status(data_dir: &Path) -> Value {
    contract::success_item(json!({
        "cli": access::cli_trusted(data_dir),
        "data_dir": data_dir.display().to_string(),
    }))
}

fn task_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let task = match verb {
        "list" => return task_page(store, words),
        "history" => return task_history(store, words),
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
        &flags(&["revision", "status", "priority", "progress"], 1),
    )?;
    Ok(store.change_task(ChangeTask {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        status: p.choice("status", STATUSES)?,
        priority: p.choice("priority", PRIORITIES)?,
        progress: p.number("progress")?,
    })?)
}

fn task_progress(store: &TaskStore, words: &[String]) -> Result<Task, CliError> {
    let p = parse(
        words,
        &flags(&["revision", "status", "progress", "next-step", "note"], 1),
    )?;
    Ok(store.record_progress(RecordProgress {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        status: p.choice("status", STATUSES)?,
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
    if p.has("list") == p.has("inbox") {
        return Err(CliError::invalid(
            "Give exactly one of --list ID or --inbox",
        ));
    }
    Ok(store.move_task(MoveTask {
        id: p.id().to_owned(),
        expected_revision: p.revision()?,
        list_id: p.value("list").map(str::to_owned),
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
            let p = parse(words, &flags(&["name"], 0))?;
            item(store.create_list(CreateList {
                name: p.required("name")?.to_owned(),
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

const STATUSES: &str = "ready, in_progress, completed";
const PRIORITIES: &str = "very_low, low, medium, high, very_high";

const ONE_ID: Spec = flags(&[], 1);

const fn flags<'a>(values: &'a [&'a str], positionals: usize) -> Spec<'a> {
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
                "limit", "cursor", "text", "status", "priority", "tag", "list", "sort",
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
        status: p.choice("status", STATUSES)?,
        priority: p.choice("priority", PRIORITIES)?,
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
        sort: p
            .choice::<SortMode>("sort", "priority, newest")?
            .unwrap_or_default(),
        task_id: None,
    };
    let page = store.query_tasks(&query, p.limit()?, p.value("cursor"))?;
    Ok(contract::success_page(
        page.items.iter().map(contract::task).collect(),
        page.next_cursor,
    ))
}
