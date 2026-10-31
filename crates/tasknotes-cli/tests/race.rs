//! Story 53 / Q48: the desktop (the core as the Tauri commands call it) and
//! a separate CLI process write the same Task from the same revision. One
//! wins; the other gets a stable conflict instead of overwriting it.

mod common;

use std::sync::{Arc, Barrier};

use common::{Cli, error_code, finish};
use tasknotes_core::audit::{AuditAction, AuditOutcome, Interface};
use tasknotes_core::tasks::{CreateTask, TaskError, TaskStore, UpdateTask};

fn desktop(cli: &Cli) -> TaskStore {
    TaskStore::open(&cli.db()).unwrap()
}

fn new_task(store: &TaskStore, title: &str) -> String {
    store
        .create_task(CreateTask {
            title: title.into(),
            description: "Synthetic".into(),
        })
        .unwrap()
        .id
}

fn desktop_edit(store: &TaskStore, id: &str, title: &str, revision: i64) -> Result<(), TaskError> {
    store
        .update_task(UpdateTask {
            id: id.into(),
            title: title.into(),
            description: "Synthetic".into(),
            expected_revision: revision,
        })
        .map(|_| ())
}

#[test]
fn a_cli_write_from_a_stale_revision_does_not_overwrite_a_desktop_edit() {
    let cli = Cli::trusted();
    let ui = desktop(&cli);
    let id = new_task(&ui, "Prepare the slides");
    desktop_edit(&ui, &id, "Prepare the final slides", 1).unwrap();

    let run = cli.run(&[
        "task",
        "update",
        &id,
        "--revision",
        "1",
        "--title",
        "AI title",
    ]);
    assert_eq!(error_code(&run, 4), "revision_conflict");
    assert_eq!(
        ui.get_task(&id).unwrap().unwrap().title,
        "Prepare the final slides"
    );
}

#[test]
fn a_desktop_write_from_a_stale_revision_does_not_overwrite_a_cli_edit() {
    let cli = Cli::trusted();
    let ui = desktop(&cli);
    let id = new_task(&ui, "Order printer ink");
    cli.ok(&[
        "task",
        "update",
        &id,
        "--revision",
        "1",
        "--title",
        "Order black ink",
    ]);

    assert!(matches!(
        desktop_edit(&ui, &id, "Order colour ink", 1),
        Err(TaskError::RevisionConflict)
    ));
    assert_eq!(ui.get_task(&id).unwrap().unwrap().title, "Order black ink");
}

#[test]
fn simultaneous_desktop_and_cli_writes_never_both_succeed() {
    let cli = Cli::trusted();
    let ui = desktop(&cli);
    let mut wins = (0, 0);
    for round in 0..30 {
        let id = new_task(&ui, &format!("Race round {round}"));
        let start = Arc::new(Barrier::new(2));
        let ui_thread = {
            let (ui, id, start) = (ui.clone(), id.clone(), start.clone());
            std::thread::spawn(move || {
                start.wait();
                // Process start-up takes milliseconds; stagger the desktop
                // write across it so both sides get to win some rounds.
                std::thread::sleep(std::time::Duration::from_millis(round));
                desktop_edit(&ui, &id, "Desktop wins", 1)
            })
        };
        let mut command = cli.command(&[
            "task",
            "update",
            &id,
            "--revision",
            "1",
            "--title",
            "CLI wins",
        ]);
        start.wait();
        let run = finish(command.output().unwrap());
        let ui_result = ui_thread.join().unwrap();

        let task = ui.get_task(&id).unwrap().unwrap();
        assert_eq!(task.revision, 2, "exactly one write landed");
        match (&ui_result, run.code) {
            (Ok(()), 4) => {
                assert_eq!(run.json["error"]["code"], "revision_conflict");
                assert_eq!(task.title, "Desktop wins");
                wins.0 += 1;
            }
            (Err(TaskError::RevisionConflict), 0) => {
                assert_eq!(task.title, "CLI wins");
                wins.1 += 1;
            }
            other => panic!("round {round}: both or neither won: {other:?} {}", run.json),
        }

        let entries = ui.list_audit_entries(Some(&id), 10, None).unwrap().items;
        let updates: Vec<_> = entries
            .iter()
            .filter(|e| e.action == AuditAction::TaskUpdate)
            .map(|e| (e.interface, e.outcome))
            .collect();
        assert_eq!(
            updates.len(),
            2,
            "the win and the refusal are both recorded"
        );
        assert!(
            updates.contains(&(Interface::Desktop, AuditOutcome::Applied))
                != updates.contains(&(Interface::Cli, AuditOutcome::Applied))
        );
    }
    eprintln!("desktop won {} rounds, CLI won {}", wins.0, wins.1);
    assert_eq!(wins.0 + wins.1, 30);
}
