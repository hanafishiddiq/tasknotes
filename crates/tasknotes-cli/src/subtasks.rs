//! `tasknotes subtask …`: a Task's checklist (batch 4), through the core's
//! own rules and writes, as the desktop and MCP use them.

use serde_json::Value;
use tasknotes_core::subtasks::{AddSubtask, MoveSubtask, UpdateSubtask};
use tasknotes_core::tasks::TaskStore;

use crate::args::parse;
use crate::contract::{self, CliError};
use crate::{ONE_ID, USAGE, flags};

pub fn subtask_command(store: &TaskStore, verb: &str, words: &[String]) -> Result<Value, CliError> {
    let item = match verb {
        "list" => {
            let p = parse(words, &ONE_ID)?;
            let items = store.list_subtasks(p.id())?;
            return Ok(contract::success_page(
                items.iter().map(contract::subtask).collect(),
                None,
            ));
        }
        "add" => {
            let p = parse(words, &flags(&["text"], 1))?;
            store.add_subtask(AddSubtask {
                task_id: p.id().to_owned(),
                text: p.required("text")?.to_owned(),
            })?
        }
        "update" => {
            let p = parse(words, &flags(&["revision", "text", "done"], 1))?;
            store.update_subtask(UpdateSubtask {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                text: p.value("text").map(str::to_owned),
                done: match p.value("done") {
                    None => None,
                    Some("true") => Some(true),
                    Some("false") => Some(false),
                    Some(_) => return Err(CliError::invalid("--done must be true or false")),
                },
            })?
        }
        "move" => {
            let p = parse(words, &flags(&["revision", "index"], 1))?;
            store.move_subtask(MoveSubtask {
                id: p.id().to_owned(),
                expected_revision: p.revision()?,
                index: p
                    .number("index")?
                    .ok_or_else(|| CliError::invalid("--index is required"))?,
            })?
        }
        "remove" => {
            let p = parse(words, &flags(&["revision"], 1))?;
            store.remove_subtask(p.id(), p.revision()?)?
        }
        "restore" => {
            let p = parse(words, &flags(&["revision"], 1))?;
            store.restore_subtask(p.id(), p.revision()?)?
        }
        _ => return Err(CliError::invalid(USAGE)),
    };
    Ok(contract::success_item(contract::subtask(&item)))
}
