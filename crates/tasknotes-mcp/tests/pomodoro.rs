//! Ticket 14: the `pomodoro_history` tool reads the core's daily history in
//! the CLI's envelope, each session on the clock it started on (the server
//! runs in Jakarta here, `TZ`). Only the desktop records sessions.
//! Synthetic data only.

mod common;

use common::{Data, Server};
use serde_json::{Value, json};
use tasknotes_core::notes::CreateNote;
use tasknotes_core::pomodoro::{EndedSession, SessionOutcome};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{CreateTask, Task};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
/// 2026-11-10 00:00 in Jakarta (UTC+7), 2026-11-09 17:00 UTC.
const NOV_10_JAKARTA: i64 = 1_794_243_600_000;
/// UTC+7 without daylight saving, in POSIX form, so no time-zone files are needed.
const JAKARTA: [(&str, &str); 1] = [("TZ", "WIB-7")];

fn session(task: Option<&Task>, started_at: i64, outcome: SessionOutcome) -> EndedSession {
    let finished = outcome == SessionOutcome::Finished;
    EndedSession {
        task_id: task.map(|task| task.id.clone()),
        started_at,
        utc_offset_seconds: 7 * 3600,
        planned_seconds: 25 * 60,
        overtime_seconds: if finished { 3 * 60 } else { 0 },
        outcome,
        stopped_after_seconds: (!finished).then_some(10 * 60),
        ended_at: started_at + if finished { 28 } else { 10 } * MINUTE,
    }
}

/// Sessions on 11-10 (one for a Task, one Stopped) and on 11-12 (for a
/// Task that is now in Removed).
fn world() -> (Data, Task, Server) {
    let data = Data::trusted();
    let desktop = data.desktop();
    let create = |title: &str| {
        desktop
            .create_task(CreateTask {
                title: title.into(),
                description: String::new(),
            })
            .unwrap()
    };
    let task = create("Synthetic outline");
    let old = create("Synthetic dropped idea");
    for ended in [
        session(
            Some(&task),
            NOV_10_JAKARTA + 9 * HOUR,
            SessionOutcome::Finished,
        ),
        session(None, NOV_10_JAKARTA + 10 * HOUR, SessionOutcome::Stopped),
        session(
            Some(&old),
            NOV_10_JAKARTA + 57 * HOUR,
            SessionOutcome::Finished,
        ),
    ] {
        desktop.record_pomodoro_session(ended).unwrap();
    }
    desktop
        .set_task_removed(SetRemoved {
            id: old.id.clone(),
            expected_revision: desktop.get_task(&old.id).unwrap().unwrap().revision,
            removed: true,
        })
        .unwrap();
    let server = data.server_with(&JAKARTA);
    (data, task, server)
}

#[test]
fn pomodoro_history_is_a_read_only_tool() {
    let (_data, _, mut server) = world();
    let tools = server.tools();
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "pomodoro_history")
        .expect("listed");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    let description = tool["description"].as_str().unwrap();
    assert!(description.contains("Stopped"), "{description}");
    assert!(description.contains("local"), "{description}");
}

#[test]
fn history_counts_finished_sessions_per_local_day_newest_first() {
    let (_data, task, mut server) = world();
    let page = server.ok("pomodoro_history", json!({}));
    assert_eq!(page["next_cursor"], Value::Null);
    let days = page["items"].as_array().unwrap();
    let summary: Vec<_> = days
        .iter()
        .map(|day| {
            (
                day["date"].clone(),
                day["finished_sessions"].clone(),
                day["focus_minutes"].clone(),
                day["stopped_sessions"].clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (json!("2026-11-12"), json!(1), json!(28), json!(0)),
            (json!("2026-11-10"), json!(1), json!(28), json!(1)),
        ]
    );
    let removed = &days[0]["sessions"][0];
    assert_eq!(removed["task_title"], Value::Null);
    assert_eq!(removed["task_removed"], true);
    // Oldest first within a day.
    let shown = &days[1]["sessions"][0];
    assert_eq!(shown["task_id"], json!(task.id));
    assert_eq!(shown["task_title"], "Synthetic outline");
    assert_eq!(shown["planned_minutes"], 25);
    assert_eq!(shown["overtime_minutes"], 3);
    assert_eq!(shown["stopped_after_minutes"], Value::Null);
    assert_eq!(shown["utc_offset_seconds"], 7 * 3600);
    let stopped = &days[1]["sessions"][1];
    assert_eq!(stopped["outcome"], "stopped");
    assert_eq!(stopped["stopped_after_minutes"], 10);
}

#[test]
fn a_session_says_each_thing_once_in_the_minutes_its_day_adds_up() {
    let (_data, _, mut server) = world();
    let page = server.ok("pomodoro_history", json!({}));
    let mut fields: Vec<_> = page["items"][0]["sessions"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    fields.sort();
    assert_eq!(
        fields,
        [
            "ended_at",
            "id",
            "outcome",
            "overtime_minutes",
            "planned_minutes",
            "started_at",
            "stopped_after_minutes",
            "task_id",
            "task_removed",
            "task_title",
            "utc_offset_seconds",
        ]
    );
}

#[test]
fn a_page_is_a_week_of_days_unless_the_client_asks_for_more() {
    let data = Data::trusted();
    let desktop = data.desktop();
    for day in 0..9 {
        desktop
            .record_pomodoro_session(session(
                None,
                NOV_10_JAKARTA + day * 24 * HOUR + 9 * HOUR,
                SessionOutcome::Finished,
            ))
            .unwrap();
    }
    let mut server = data.server_with(&JAKARTA);
    let week = server.ok("pomodoro_history", json!({}));
    let dates: Vec<_> = week["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["date"].clone())
        .collect();
    assert_eq!(dates.len(), 7);
    assert_eq!(dates[0], "2026-11-18");
    assert_eq!(dates[6], "2026-11-12");
    let rest = server.ok(
        "pomodoro_history",
        json!({"cursor": week["next_cursor"].as_str().expect("two more days")}),
    );
    assert_eq!(rest["items"].as_array().unwrap().len(), 2);
    assert_eq!(rest["next_cursor"], Value::Null);
    let all = server.ok("pomodoro_history", json!({"limit": 9}));
    assert_eq!(all["items"].as_array().unwrap().len(), 9);
}

#[test]
fn one_answer_holds_at_most_a_hundred_sessions_in_whole_days() {
    let data = Data::trusted();
    let desktop = data.desktop();
    // 60 one-minute sessions on 11-11 and 50 on 11-10.
    for (day, count) in [(1, 60), (0, 50)] {
        for n in 0..count {
            let started_at = NOV_10_JAKARTA + day * 24 * HOUR + 8 * HOUR + n * 2 * MINUTE;
            desktop
                .record_pomodoro_session(EndedSession {
                    planned_seconds: 60,
                    overtime_seconds: 0,
                    ended_at: started_at + MINUTE,
                    ..session(None, started_at, SessionOutcome::Finished)
                })
                .unwrap();
        }
    }
    let mut server = data.server_with(&JAKARTA);
    let first = server.ok("pomodoro_history", json!({"limit": 10}));
    let days = first["items"].as_array().unwrap();
    assert_eq!(days.len(), 1, "60 + 50 would pass 100");
    assert_eq!(days[0]["date"], "2026-11-11");
    assert_eq!(days[0]["sessions"].as_array().unwrap().len(), 60);
    let cursor = first["next_cursor"]
        .as_str()
        .expect("the next day")
        .to_owned();
    let second = server.ok("pomodoro_history", json!({"limit": 10, "cursor": cursor}));
    assert_eq!(second["items"][0]["date"], "2026-11-10");
    assert_eq!(second["items"][0]["sessions"].as_array().unwrap().len(), 50);
    assert_eq!(second["next_cursor"], Value::Null);
}

#[test]
fn from_to_limit_and_cursor_read_a_range_of_days() {
    let (_data, _, mut server) = world();
    let only = server.ok(
        "pomodoro_history",
        json!({"from": "2026-11-11", "to": "2026-11-12"}),
    );
    assert_eq!(only["items"].as_array().unwrap().len(), 1);
    assert_eq!(only["items"][0]["date"], "2026-11-12");

    let first = server.ok("pomodoro_history", json!({"limit": 1}));
    let cursor = first["next_cursor"]
        .as_str()
        .expect("a second page")
        .to_owned();
    let second = server.ok("pomodoro_history", json!({"limit": 1, "cursor": cursor}));
    assert_eq!(second["items"][0]["date"], "2026-11-10");
    assert_eq!(second["next_cursor"], Value::Null);
}

#[test]
fn impossible_ranges_and_unknown_arguments_are_refused() {
    let (_data, _, mut server) = world();
    for arguments in [
        json!({"from": "2026-11-31"}),
        json!({"from": "2026-11-12", "to": "2026-11-10"}),
        json!({"since": "2026-11-10"}),
        json!({"limit": 0}),
    ] {
        let envelope = server.call("pomodoro_history", arguments.clone());
        assert_eq!(envelope["error"]["code"], "invalid_input", "{arguments}");
    }
    let envelope = server.call("pomodoro_history", json!({"cursor": "nope"}));
    assert_eq!(envelope["error"]["code"], "invalid_cursor");
}

#[test]
fn history_never_carries_note_text() {
    let (data, task, mut server) = world();
    let desktop = data.desktop();
    let note = desktop
        .create_note(CreateNote {
            markdown: "# Private synthetic log\n\nsecret-marker-9921".into(),
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
    let out = server.ok("pomodoro_history", json!({})).to_string();
    for leak in [
        "secret-marker-9921",
        "Private synthetic log",
        note.id.as_str(),
    ] {
        assert!(!out.contains(leak), "leaked {leak:?}: {out}");
    }
}
