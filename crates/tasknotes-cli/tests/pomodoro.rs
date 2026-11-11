//! Ticket 14: `tasknotes pomodoro history` reads the core's daily history
//! through the real process: each session on the clock it started on, and a
//! session stored before schema v23 on the Mac's own time zone (set here
//! with `TZ`). The desktop side is the core store opened directly, as the
//! window opens it: only the desktop records sessions. Synthetic data only.

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

/// A Finished 25-minute session started in Jakarta.
fn session(task: Option<&Task>, started_at: i64, overtime_seconds: i64) -> EndedSession {
    EndedSession {
        task_id: task.map(|task| task.id.clone()),
        started_at,
        utc_offset_seconds: 7 * 3600,
        planned_seconds: 25 * 60,
        overtime_seconds,
        outcome: SessionOutcome::Finished,
        stopped_after_seconds: None,
        ended_at: started_at + 25 * MINUTE + overtime_seconds * 1000,
    }
}

fn stopped(started_at: i64) -> EndedSession {
    EndedSession {
        overtime_seconds: 0,
        outcome: SessionOutcome::Stopped,
        stopped_after_seconds: Some(10 * 60),
        ended_at: started_at + 10 * MINUTE,
        ..session(None, started_at, 0)
    }
}

/// A Finished 25-minute session as schema v22 stored it: no offset.
fn stored_before_offsets(cli: &Cli, started_at: i64) {
    rusqlite::Connection::open(cli.db())
        .unwrap()
        .execute(
            "INSERT INTO pomodoro_sessions
               (id, started_at, planned_seconds, outcome, ended_at, created_at)
             VALUES (lower(hex(randomblob(8))), ?1, 1500, 'finished', ?1 + 1500000, ?1)",
            [started_at],
        )
        .unwrap();
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
    assert_eq!(counted["utc_offset_seconds"], 7 * 3600);
    // Minutes only: the seconds would say the same thing twice.
    assert!(counted.get("overtime_seconds").is_none());
    assert!(counted.get("planned_seconds").is_none());
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
        .map(|s| {
            (
                s["outcome"].clone(),
                s["task_title"].clone(),
                s["stopped_after_minutes"].clone(),
            )
        })
        .collect();
    assert_eq!(
        outcomes,
        vec![
            (
                json!("finished"),
                json!("Synthetic chapter draft"),
                Value::Null
            ),
            (json!("stopped"), Value::Null, json!(10)),
        ],
        "oldest first within a day"
    );
    assert_eq!(older["sessions"][0]["task_id"], json!(task.id));
}

fn dates(page: &Value) -> Vec<Value> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["date"].clone())
        .collect()
}

#[test]
fn sessions_keep_the_days_of_the_clock_they_started_on_wherever_the_mac_is_now() {
    let (cli, _) = world();
    // Started in Jakarta at 09:00 and 10:00 on 11-10 and 09:00 on 11-11
    // (02:00 and 03:00 UTC); read after flying to UTC-12, where those
    // instants are still the day before.
    for tz in [JAKARTA_TZ, "XST+12", "UTC0"] {
        let page = ok_in(&cli, tz, &["pomodoro", "history"]);
        assert_eq!(
            dates(&page),
            [json!("2026-11-11"), json!("2026-11-10")],
            "{tz}"
        );
        let only = ok_in(
            &cli,
            tz,
            &[
                "pomodoro",
                "history",
                "--from",
                "2026-11-10",
                "--to",
                "2026-11-10",
            ],
        );
        assert_eq!(dates(&only), [json!("2026-11-10")], "{tz}");
    }
}

#[test]
fn sessions_stored_before_offsets_were_kept_follow_the_macs_time_zone() {
    let cli = Cli::trusted();
    drop(TaskStore::open(&cli.db()).unwrap());
    // 09:00 in Jakarta on 11-10: 02:00 UTC, the day before in UTC-12.
    stored_before_offsets(&cli, NOV_10_JAKARTA + 9 * HOUR);
    let jakarta = ok_in(&cli, JAKARTA_TZ, &["pomodoro", "history"]);
    assert_eq!(dates(&jakarta), [json!("2026-11-10")]);
    assert_eq!(
        jakarta["items"][0]["sessions"][0]["utc_offset_seconds"],
        7 * 3600
    );
    let far_west = ok_in(&cli, "XST+12", &["pomodoro", "history"]);
    assert_eq!(dates(&far_west), [json!("2026-11-09")]);
    assert_eq!(
        far_west["items"][0]["sessions"][0]["utc_offset_seconds"],
        -12 * 3600
    );
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
    assert_eq!(dates(&only), [json!("2026-11-10")]);

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

/// Sessions stored before schema v23 (no offset of their own) on a Mac
/// whose clock goes back past midnight: the core's fallback to the Mac's
/// zone follows its daylight-saving rules, as `localtime_r` gives them.
#[test]
fn days_follow_daylight_saving_even_when_the_clock_goes_back_past_midnight() {
    let cli = Cli::trusted();
    drop(TaskStore::open(&cli.db()).unwrap());
    for started_at in [
        1_793_458_800_000, // 10-31 11:00 daylight time
        1_793_506_500_000, // 11-01 00:15 daylight time
        1_793_508_300_000, // 10-31 23:45 standard time, after the change
        1_793_541_600_000, // 11-01 09:00 standard time
    ] {
        stored_before_offsets(&cli, started_at);
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
        vec![json!(1_793_506_500_000_i64), json!(1_793_541_600_000_i64)],
    );
    let oct_31 = (
        json!("2026-10-31"),
        vec![json!(1_793_458_800_000_i64), json!(1_793_508_300_000_i64)],
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
