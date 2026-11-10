//! Ticket 14: the Pomodoro daily history, read the same way by the window,
//! the CLI and MCP. Days are local calendar days; only Finished sessions
//! count. Synthetic data only.

use std::path::Path;

use tasknotes_core::audit::Interface;
use tasknotes_core::calendar::TimeZone;
use tasknotes_core::pomodoro::{EndedSession, HistoryRange, SessionOutcome};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    CreateList, CreateTask, PatchTask, SetListRemoved, Task, TaskError, TaskStore,
};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
/// 2026-11-10 00:00 in Jakarta (UTC+7), 2026-11-09 17:00 UTC.
const NOV_10_JAKARTA: i64 = 1_794_243_600_000;
const JAKARTA: TimeZone = TimeZone::Fixed { minutes: 7 * 60 };

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

/// A focus planned for `planned` minutes that started at `started_at`.
fn finished(started_at: i64, planned: i64, overtime_seconds: i64) -> EndedSession {
    EndedSession {
        task_id: None,
        started_at,
        planned_seconds: planned * 60,
        overtime_seconds,
        outcome: SessionOutcome::Finished,
        ended_at: started_at + planned * MINUTE + overtime_seconds * 1000,
    }
}

fn stopped(started_at: i64, planned: i64, after_minutes: i64) -> EndedSession {
    EndedSession {
        task_id: None,
        started_at,
        planned_seconds: planned * 60,
        overtime_seconds: 0,
        outcome: SessionOutcome::Stopped,
        ended_at: started_at + after_minutes * MINUTE,
    }
}

fn everything(zone: TimeZone) -> HistoryRange {
    HistoryRange {
        from: None,
        to: None,
        zone,
    }
}

#[test]
fn a_day_counts_its_finished_sessions_and_their_planned_plus_counted_overtime() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // 09:00 25 min with 7:12 counted, 10:00 25 min, 11:00 stopped after 10 min.
    store
        .record_pomodoro_session(finished(NOV_10_JAKARTA + 9 * HOUR, 25, 7 * 60 + 12))
        .unwrap();
    store
        .record_pomodoro_session(finished(NOV_10_JAKARTA + 10 * HOUR, 25, 0))
        .unwrap();
    store
        .record_pomodoro_session(stopped(NOV_10_JAKARTA + 11 * HOUR, 25, 10))
        .unwrap();

    let page = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap();
    assert_eq!(page.next_cursor, None);
    assert_eq!(page.items.len(), 1);
    let day = &page.items[0];
    assert_eq!(day.date, "2026-11-10");
    assert_eq!(day.finished_sessions, 2, "the Stopped one is not counted");
    assert_eq!(day.stopped_sessions, 1);
    assert_eq!(day.focus_minutes, 57, "25 + 7, then 25");

    let listed: Vec<_> = day
        .sessions
        .iter()
        .map(|s| {
            (
                s.started_at,
                s.outcome,
                s.planned_minutes,
                s.overtime_minutes,
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            (NOV_10_JAKARTA + 11 * HOUR, SessionOutcome::Stopped, 25, 0),
            (NOV_10_JAKARTA + 10 * HOUR, SessionOutcome::Finished, 25, 0),
            (NOV_10_JAKARTA + 9 * HOUR, SessionOutcome::Finished, 25, 7),
        ],
        "every session, newest first"
    );
}

#[test]
fn days_follow_the_wall_clock_of_the_owners_time_zone() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // 06:30 in Jakarta on 11-10 is still 23:30 on 11-09 in UTC.
    store
        .record_pomodoro_session(finished(NOV_10_JAKARTA + 6 * HOUR + 30 * MINUTE, 25, 0))
        .unwrap();
    let date_in = |zone| {
        store
            .pomodoro_history(&everything(zone), 10, None)
            .unwrap()
            .items[0]
            .date
            .clone()
    };
    assert_eq!(date_in(JAKARTA), "2026-11-10");
    assert_eq!(date_in(TimeZone::UTC), "2026-11-09");
}

#[test]
fn a_session_that_runs_past_midnight_belongs_to_the_day_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // 23:50 on 11-10 with 9 min counted: it ends at 00:24 on 11-11.
    let late = NOV_10_JAKARTA + 23 * HOUR + 50 * MINUTE;
    store
        .record_pomodoro_session(finished(late, 25, 9 * 60))
        .unwrap();
    store
        .record_pomodoro_session(finished(late + HOUR, 25, 0))
        .unwrap();

    let page = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap();
    let days: Vec<_> = page
        .items
        .iter()
        .map(|day| (day.date.as_str(), day.finished_sessions, day.focus_minutes))
        .collect();
    assert_eq!(days, vec![("2026-11-11", 1, 25), ("2026-11-10", 1, 34)]);
}

/// One Finished 25-minute session at 09:00 on each of `days` after 11-10.
fn one_a_day(store: &TaskStore, days: impl IntoIterator<Item = i64>) {
    for day in days {
        store
            .record_pomodoro_session(finished(NOV_10_JAKARTA + day * 24 * HOUR + 9 * HOUR, 25, 0))
            .unwrap();
    }
}

fn dates(page: &tasknotes_core::pomodoro::PomodoroHistoryPage) -> Vec<&str> {
    page.items.iter().map(|day| day.date.as_str()).collect()
}

#[test]
fn days_come_newest_first_in_pages_that_continue_from_a_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // 11-10, 11-11, 11-13 and 11-20: days without sessions are not listed.
    one_a_day(&store, [0, 1, 3, 10]);
    let range = everything(JAKARTA);

    let first = store.pomodoro_history(&range, 3, None).unwrap();
    assert_eq!(dates(&first), ["2026-11-20", "2026-11-13", "2026-11-11"]);
    let cursor = first.next_cursor.expect("one more day");
    let second = store.pomodoro_history(&range, 3, Some(&cursor)).unwrap();
    assert_eq!(dates(&second), ["2026-11-10"]);
    assert_eq!(second.next_cursor, None);

    // Exactly a page's worth: no cursor to an empty page.
    let all = store.pomodoro_history(&range, 4, None).unwrap();
    assert_eq!(all.items.len(), 4);
    assert_eq!(all.next_cursor, None);
}

#[test]
fn a_range_keeps_the_days_from_its_first_to_its_last_date_both_included() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    one_a_day(&store, [0, 1, 2, 3, 4]);
    let range = |from: Option<&str>, to: Option<&str>| HistoryRange {
        from: from.map(str::to_owned),
        to: to.map(str::to_owned),
        zone: JAKARTA,
    };

    let page = store
        .pomodoro_history(&range(Some("2026-11-11"), Some("2026-11-13")), 10, None)
        .unwrap();
    assert_eq!(dates(&page), ["2026-11-13", "2026-11-12", "2026-11-11"]);
    let since = store
        .pomodoro_history(&range(Some("2026-11-13"), None), 10, None)
        .unwrap();
    assert_eq!(dates(&since), ["2026-11-14", "2026-11-13"]);
    let until = store
        .pomodoro_history(&range(None, Some("2026-11-10")), 10, None)
        .unwrap();
    assert_eq!(dates(&until), ["2026-11-10"]);
    let one_day = store
        .pomodoro_history(&range(Some("2026-11-12"), Some("2026-11-12")), 10, None)
        .unwrap();
    assert_eq!(dates(&one_day), ["2026-11-12"]);

    // Paging stays inside the range.
    let first = store
        .pomodoro_history(&range(Some("2026-11-11"), Some("2026-11-13")), 2, None)
        .unwrap();
    assert_eq!(dates(&first), ["2026-11-13", "2026-11-12"]);
    let rest = store
        .pomodoro_history(
            &range(Some("2026-11-11"), Some("2026-11-13")),
            2,
            first.next_cursor.as_deref(),
        )
        .unwrap();
    assert_eq!(dates(&rest), ["2026-11-11"]);
    assert_eq!(rest.next_cursor, None);
}

fn task(store: &TaskStore, title: &str, list_id: Option<&str>) -> Task {
    store
        .create_task_in_list(
            CreateTask {
                title: title.into(),
                description: String::new(),
            },
            list_id,
        )
        .unwrap()
}

fn for_task(task: &Task, session: EndedSession) -> EndedSession {
    EndedSession {
        task_id: Some(task.id.clone()),
        ..session
    }
}

#[test]
fn each_session_names_its_task_no_task_or_a_task_in_removed() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let shown = task(&store, "Synthetic report", None);
    let set_aside = task(&store, "Synthetic old idea", None);
    let list = store
        .create_list(CreateList {
            name: "Synthetic shelf".into(),
        })
        .unwrap();
    let shelved = task(&store, "Synthetic shelved", Some(&list.id));
    let at = |hour| NOV_10_JAKARTA + hour * HOUR;
    store
        .record_pomodoro_session(for_task(&shown, finished(at(9), 25, 0)))
        .unwrap();
    store
        .record_pomodoro_session(for_task(&set_aside, finished(at(10), 25, 0)))
        .unwrap();
    store
        .record_pomodoro_session(for_task(&shelved, stopped(at(11), 25, 5)))
        .unwrap();
    store
        .record_pomodoro_session(finished(at(12), 25, 0))
        .unwrap();
    // Afterwards: one Task renamed, one put in Removed, one List put in Removed.
    store
        .patch_task(PatchTask {
            id: shown.id.clone(),
            expected_revision: store.get_task(&shown.id).unwrap().unwrap().revision,
            title: Some("Synthetic report, renamed".into()),
            description: None,
        })
        .unwrap();
    store
        .set_task_removed(SetRemoved {
            id: set_aside.id.clone(),
            expected_revision: store.get_task(&set_aside.id).unwrap().unwrap().revision,
            removed: true,
        })
        .unwrap();
    store
        .set_list_removed(SetListRemoved {
            id: list.id.clone(),
            expected_revision: list.revision,
            removed: true,
        })
        .unwrap();

    let page = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap();
    let tasks: Vec<_> = page.items[0]
        .sessions
        .iter()
        .map(|s| (s.task_id.clone(), s.task_title.as_deref(), s.task_removed))
        .collect();
    assert_eq!(
        tasks,
        vec![
            (None, None, false),
            (Some(shelved.id.clone()), None, true),
            (Some(set_aside.id.clone()), None, true),
            (
                Some(shown.id.clone()),
                Some("Synthetic report, renamed"),
                false
            ),
        ]
    );
    // A Task in Removed does not change what the day counts.
    assert_eq!(page.items[0].finished_sessions, 3);
    assert_eq!(page.items[0].focus_minutes, 75);
}

#[test]
fn the_day_total_is_the_sum_of_the_minutes_its_sessions_show() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // Each counts 0:40 of Overtime, shown as "25 + 1 min" like its Progress
    // Entry; 0:20 rounds to nothing ("25 min").
    for (hour, overtime) in [(9, 40), (10, 40), (11, 40), (12, 20)] {
        store
            .record_pomodoro_session(finished(NOV_10_JAKARTA + hour * HOUR, 25, overtime))
            .unwrap();
    }
    let day = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap()
        .items
        .remove(0);
    let shown: Vec<_> = day
        .sessions
        .iter()
        .map(|s| (s.planned_minutes, s.overtime_minutes))
        .collect();
    assert_eq!(shown, vec![(25, 0), (25, 1), (25, 1), (25, 1)]);
    assert_eq!(day.focus_minutes, 103);
}

#[test]
fn a_day_of_only_stopped_sessions_is_listed_with_nothing_counted() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    store
        .record_pomodoro_session(stopped(NOV_10_JAKARTA + 9 * HOUR, 50, 12))
        .unwrap();
    let day = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(day.date, "2026-11-10");
    assert_eq!(
        (
            day.finished_sessions,
            day.focus_minutes,
            day.stopped_sessions
        ),
        (0, 0, 1)
    );
    assert_eq!(day.sessions[0].outcome, SessionOutcome::Stopped);
    assert_eq!(day.sessions[0].planned_minutes, 50);
}

#[test]
fn an_empty_history_has_no_days() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let page = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.next_cursor, None);
}

#[test]
fn impossible_dates_sizes_and_cursors_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let range = |from: &str, to: &str| HistoryRange {
        from: Some(from.into()),
        to: Some(to.into()),
        zone: JAKARTA,
    };
    for (from, to) in [
        ("2026-02-30", "2026-03-01"),
        ("2026-11-10", "11/12/2026"),
        ("2026-11-12", "2026-11-10"),
    ] {
        let error = store
            .pomodoro_history(&range(from, to), 10, None)
            .unwrap_err();
        assert!(
            matches!(error, TaskError::InvalidHistoryRange),
            "{from}..{to}"
        );
        assert_eq!(error.code(), "invalid_input");
    }
    for limit in [0, 101] {
        let error = store
            .pomodoro_history(&everything(JAKARTA), limit, None)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_input", "limit {limit}");
    }
    for cursor in ["", "2026-11-10", "p1|2026-11-31", "h1|5"] {
        let error = store
            .pomodoro_history(&everything(JAKARTA), 10, Some(cursor))
            .unwrap_err();
        assert_eq!(error.code(), "invalid_cursor", "{cursor:?}");
    }
}

#[test]
fn every_interface_reads_the_same_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let desktop = store_at(&path);
    let shown = task(&desktop, "Synthetic shared", None);
    desktop
        .record_pomodoro_session(for_task(&shown, finished(NOV_10_JAKARTA + 9 * HOUR, 25, 0)))
        .unwrap();
    let seen = desktop
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap();
    for interface in [Interface::Cli, Interface::Mcp] {
        let store = TaskStore::open_as(&path, interface).unwrap();
        assert_eq!(
            store
                .pomodoro_history(&everything(JAKARTA), 10, None)
                .unwrap(),
            seen,
            "{interface:?}"
        );
    }
}

#[test]
fn the_ends_of_a_range_are_local_midnights_not_utc_ones() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    let new_york = TimeZone::Fixed { minutes: -4 * 60 };
    // 06:30 on 11-13 in Jakarta is 23:30 on 11-12 UTC.
    let early = NOV_10_JAKARTA + 3 * 24 * HOUR + 6 * HOUR + 30 * MINUTE;
    store
        .record_pomodoro_session(finished(early, 25, 0))
        .unwrap();
    let one_day = |date: &str, zone| HistoryRange {
        from: Some(date.into()),
        to: Some(date.into()),
        zone,
    };
    let page = store
        .pomodoro_history(&one_day("2026-11-13", JAKARTA), 10, None)
        .unwrap();
    assert_eq!(dates(&page), ["2026-11-13"]);
    // The same instant is 19:30 on 11-12 in New York (UTC-4).
    let page = store
        .pomodoro_history(&one_day("2026-11-12", new_york), 10, None)
        .unwrap();
    assert_eq!(dates(&page), ["2026-11-12"]);
    // 22:00 on 11-12 in New York is 02:00 on 11-13 UTC.
    store
        .record_pomodoro_session(finished(early + 2 * HOUR + 30 * MINUTE, 25, 0))
        .unwrap();
    let page = store
        .pomodoro_history(&one_day("2026-11-12", new_york), 10, None)
        .unwrap();
    assert_eq!(page.items[0].finished_sessions, 2);
}
