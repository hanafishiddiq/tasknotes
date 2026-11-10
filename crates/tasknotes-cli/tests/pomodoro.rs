//! Ticket 14: `tasknotes pomodoro history` reads the core's daily history
//! through the real process, on the Mac's own time zone (set here with `TZ`).
//! The desktop side is the core store opened directly, as the window opens
//! it: only the desktop records sessions. Synthetic data only.

mod common;

use common::{Cli, Run, error_code, finish};
use serde_json::{Value, json};
use tasknotes_core::notes::CreateNote;
use tasknotes_core::pomodoro::{EndedSession, SessionOutcome};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::tasks::{CreateTask, Task, TaskStore};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
/// 2026-11-10 00:00 in Jakarta (UTC+7), 2026-11-09 17:00 UTC.
const NOV_10_JAKARTA: i64 = 1_794_243_600_000;
/// UTC+7 without daylight saving, in POSIX form, so no time-zone files are needed.
const JAKARTA_TZ: &str = "WIB-7";

fn run_in(cli: &Cli, tz: &str, args: &[&str]) -> Run {
    finish(cli.command(args).env("TZ", tz).output().unwrap())
}

fn ok_in(cli: &Cli, tz: &str, args: &[&str]) -> Value {
    let run = run_in(cli, tz, args);
    assert_eq!(run.code, 0, "{args:?} failed: {}", run.json);
    assert!(run.stderr.is_empty(), "stderr: {}", run.stderr);
    run.json
}

fn session(task: Option<&Task>, started_at: i64, overtime_seconds: i64) -> EndedSession {
    EndedSession {
        task_id: task.map(|task| task.id.clone()),
        started_at,
        planned_seconds: 25 * 60,
        overtime_seconds,
        outcome: SessionOutcome::Finished,
        ended_at: started_at + 25 * MINUTE + overtime_seconds * 1000,
    }
}

fn stopped(started_at: i64) -> EndedSession {
    EndedSession {
        overtime_seconds: 0,
        outcome: SessionOutcome::Stopped,
        ended_at: started_at + 10 * MINUTE,
        ..session(None, started_at, 0)
    }
}

/// A Task with sessions on 11-10 (two, one Stopped) and 11-11 (one with
/// 7:12 of Overtime counted).
fn world() -> (Cli, Task) {
    let cli = Cli::trusted();
    let desktop = TaskStore::open(&cli.db()).unwrap();
    let task = desktop
        .create_task(CreateTask {
            title: "Synthetic chapter draft".into(),
            description: String::new(),
        })
        .unwrap();
    for ended in [
        session(Some(&task), NOV_10_JAKARTA + 9 * HOUR, 0),
        stopped(NOV_10_JAKARTA + 10 * HOUR),
        session(None, NOV_10_JAKARTA + 33 * HOUR, 7 * 60 + 12),
    ] {
        desktop.record_pomodoro_session(ended).unwrap();
    }
    (cli, task)
}

#[test]
fn history_lists_days_newest_first_with_what_each_counts() {
    let (cli, task) = world();
    let page = ok_in(&cli, JAKARTA_TZ, &["pomodoro", "history"]);
    assert_eq!(page["next_cursor"], Value::Null);
    let days = page["items"].as_array().unwrap();
    assert_eq!(days.len(), 2);

    let newest = &days[0];
    assert_eq!(newest["date"], "2026-11-11");
    assert_eq!(newest["finished_sessions"], 1);
    assert_eq!(newest["focus_minutes"], 32, "25 + 7");
    assert_eq!(newest["stopped_sessions"], 0);
    let counted = &newest["sessions"][0];
    assert_eq!(counted["started_at"], NOV_10_JAKARTA + 33 * HOUR);
    assert_eq!(counted["outcome"], "finished");
    assert_eq!(counted["planned_minutes"], 25);
    assert_eq!(counted["overtime_minutes"], 7);
    assert_eq!(counted["overtime_seconds"], 432);
    assert_eq!(counted["task_id"], Value::Null);
    assert_eq!(counted["task_title"], Value::Null);
    assert_eq!(counted["task_removed"], false);

    let older = &days[1];
    assert_eq!(older["date"], "2026-11-10");
    assert_eq!(
        older["finished_sessions"], 1,
        "the Stopped one is not counted"
    );
    assert_eq!(older["focus_minutes"], 25);
    assert_eq!(older["stopped_sessions"], 1);
    let outcomes: Vec<_> = older["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["outcome"].clone(), s["task_title"].clone()))
        .collect();
    assert_eq!(
        outcomes,
        vec![
            (json!("stopped"), Value::Null),
            (json!("finished"), json!("Synthetic chapter draft")),
        ]
    );
    assert_eq!(older["sessions"][1]["task_id"], json!(task.id));
}

#[test]
fn days_are_the_macs_local_days() {
    let (cli, _) = world();
    // 09:00 and 10:00 in Jakarta on 11-10 are 02:00 and 03:00 UTC that day;
    // 09:00 on 11-11 is 02:00 UTC. In UTC-12 all three fall a day earlier.
    let dates = |tz| -> Vec<Value> {
        ok_in(&cli, tz, &["pomodoro", "history"])["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|day| day["date"].clone())
            .collect()
    };
    assert_eq!(
        dates(JAKARTA_TZ),
        [json!("2026-11-11"), json!("2026-11-10")]
    );
    assert_eq!(dates("XST+12"), [json!("2026-11-10"), json!("2026-11-09")]);
}

#[test]
fn from_and_to_choose_the_days_and_limit_pages_them() {
    let (cli, _) = world();
    let only = ok_in(
        &cli,
        JAKARTA_TZ,
        &[
            "pomodoro",
            "history",
            "--from",
            "2026-11-10",
            "--to",
            "2026-11-10",
        ],
    );
    let dates: Vec<_> = only["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["date"].clone())
        .collect();
    assert_eq!(dates, [json!("2026-11-10")]);

    let first = ok_in(&cli, JAKARTA_TZ, &["pomodoro", "history", "--limit", "1"]);
    assert_eq!(first["items"][0]["date"], "2026-11-11");
    let cursor = first["next_cursor"]
        .as_str()
        .expect("a second page")
        .to_owned();
    let second = ok_in(
        &cli,
        JAKARTA_TZ,
        &["pomodoro", "history", "--limit", "1", "--cursor", &cursor],
    );
    assert_eq!(second["items"][0]["date"], "2026-11-10");
    assert_eq!(second["next_cursor"], Value::Null);
}

#[test]
fn impossible_dates_and_stray_words_are_invalid_input() {
    let (cli, _) = world();
    for args in [
        &["pomodoro", "history", "--from", "2026-02-30"][..],
        &[
            "pomodoro",
            "history",
            "--from",
            "2026-11-12",
            "--to",
            "2026-11-10",
        ],
        &["pomodoro", "history", "--since", "2026-11-10"],
        &["pomodoro", "history", "2026-11-10"],
        &["pomodoro", "start"],
    ] {
        let run = run_in(&cli, JAKARTA_TZ, args);
        assert_eq!(error_code(&run, 2), "invalid_input", "{args:?}");
    }
    let run = run_in(
        &cli,
        JAKARTA_TZ,
        &["pomodoro", "history", "--cursor", "nope"],
    );
    assert_eq!(error_code(&run, 2), "invalid_cursor");
}

#[test]
fn history_needs_the_persons_grant_like_every_command() {
    let cli = Cli::untrusted();
    let run = run_in(&cli, JAKARTA_TZ, &["pomodoro", "history"]);
    assert_eq!(error_code(&run, 6), "not_trusted");
}

#[test]
fn history_shows_task_titles_but_never_note_text() {
    let (cli, task) = world();
    let desktop = TaskStore::open(&cli.db()).unwrap();
    let note = desktop
        .create_note(CreateNote {
            markdown: "# Private synthetic plan\n\nsecret-marker-4417".into(),
            folder_id: None,
        })
        .unwrap();
    desktop.link_note(&task.id, &note.id).unwrap();
    desktop
        .set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: note.id.clone(),
            exclude_from_ai: true,
        })
        .unwrap();
    let out = ok_in(&cli, JAKARTA_TZ, &["pomodoro", "history"]).to_string();
    assert!(out.contains("Synthetic chapter draft"));
    for leak in [
        "secret-marker-4417",
        "Private synthetic plan",
        note.id.as_str(),
    ] {
        assert!(!out.contains(leak), "leaked {leak:?}: {out}");
    }
}

/// UTC-5, with daylight saving (UTC-4) that ends on 2026-11-01 at 00:30,
/// when the clock goes back to 23:30 of 10-31: for that hour the local date
/// is a day earlier than a moment before it.
const FALL_BACK_TZ: &str = "XST5XDT,M3.2.0,M11.1.0/0:30";

#[test]
fn days_follow_daylight_saving_even_when_the_clock_goes_back_past_midnight() {
    let cli = Cli::trusted();
    let desktop = TaskStore::open(&cli.db()).unwrap();
    for started_at in [
        1_793_458_800_000, // 10-31 11:00 daylight time
        1_793_506_500_000, // 11-01 00:15 daylight time
        1_793_508_300_000, // 10-31 23:45 standard time, after the change
        1_793_541_600_000, // 11-01 09:00 standard time
    ] {
        desktop
            .record_pomodoro_session(session(None, started_at, 0))
            .unwrap();
    }
    let days = |page: &Value| -> Vec<(Value, Vec<Value>)> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|day| {
                let starts = day["sessions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["started_at"].clone())
                    .collect();
                (day["date"].clone(), starts)
            })
            .collect()
    };
    let nov_1 = (
        json!("2026-11-01"),
        vec![json!(1_793_541_600_000_i64), json!(1_793_506_500_000_i64)],
    );
    let oct_31 = (
        json!("2026-10-31"),
        vec![json!(1_793_508_300_000_i64), json!(1_793_458_800_000_i64)],
    );
    let all = ok_in(&cli, FALL_BACK_TZ, &["pomodoro", "history"]);
    assert_eq!(days(&all), vec![nov_1.clone(), oct_31.clone()]);

    // One day a page: the 00:15 session is found past the 23:45 one.
    let first = ok_in(&cli, FALL_BACK_TZ, &["pomodoro", "history", "--limit", "1"]);
    assert_eq!(days(&first), vec![nov_1]);
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    let second = ok_in(
        &cli,
        FALL_BACK_TZ,
        &["pomodoro", "history", "--limit", "1", "--cursor", &cursor],
    );
    assert_eq!(days(&second), vec![oct_31]);
    assert_eq!(second["next_cursor"], Value::Null);
}
