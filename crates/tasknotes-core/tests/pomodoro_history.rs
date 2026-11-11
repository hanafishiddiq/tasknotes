//! Ticket 14: the Pomodoro daily history, read the same way by the window,
//! the CLI and MCP. Days are local calendar days on the clock each session
//! started on; only Finished sessions count. Synthetic data only.

use std::path::Path;

use rusqlite::Connection;
use tasknotes_core::audit::Interface;
use tasknotes_core::calendar::TimeZone;
use tasknotes_core::pomodoro::{EndedSession, HistoryRange, PomodoroTotals, SessionOutcome};
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    CreateList, CreateTask, PatchTask, SetListRemoved, Task, TaskError, TaskStore,
};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
/// 2026-11-10 00:00 in Jakarta (UTC+7), 2026-11-09 17:00 UTC.
const NOV_10_JAKARTA: i64 = 1_794_243_600_000;
const JAKARTA: TimeZone = TimeZone::Fixed { minutes: 7 * 60 };
/// Seconds east of UTC of the Mac's clock in Jakarta.
const JAKARTA_OFFSET: i32 = 7 * 3600;

fn store_at(path: &Path) -> TaskStore {
    TaskStore::open(path).expect("open isolated synthetic Tasknotes DB")
}

/// A focus planned for `planned` minutes that started at `started_at` in
/// Jakarta.
fn finished(started_at: i64, planned: i64, overtime_seconds: i64) -> EndedSession {
    EndedSession {
        task_id: None,
        started_at,
        utc_offset_seconds: JAKARTA_OFFSET,
        planned_seconds: planned * 60,
        overtime_seconds,
        outcome: SessionOutcome::Finished,
        stopped_after_seconds: None,
        ended_at: started_at + planned * MINUTE + overtime_seconds * 1000,
    }
}

fn stopped(started_at: i64, planned: i64, after_minutes: i64) -> EndedSession {
    EndedSession {
        task_id: None,
        started_at,
        utc_offset_seconds: JAKARTA_OFFSET,
        planned_seconds: planned * 60,
        overtime_seconds: 0,
        outcome: SessionOutcome::Stopped,
        stopped_after_seconds: Some(after_minutes * 60),
        ended_at: started_at + after_minutes * MINUTE,
    }
}

/// A Finished 25-minute session on a clock `hours` east of UTC.
fn at_offset(started_at: i64, hours: i32) -> EndedSession {
    EndedSession {
        utc_offset_seconds: hours * 3600,
        ..finished(started_at, 25, 0)
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
                s.stopped_after_minutes,
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            (
                NOV_10_JAKARTA + 9 * HOUR,
                SessionOutcome::Finished,
                25,
                7,
                None
            ),
            (
                NOV_10_JAKARTA + 10 * HOUR,
                SessionOutcome::Finished,
                25,
                0,
                None
            ),
            (
                NOV_10_JAKARTA + 11 * HOUR,
                SessionOutcome::Stopped,
                25,
                0,
                Some(10)
            ),
        ],
        "every session, oldest first: the day reads as a log"
    );
}

#[test]
fn a_session_stays_on_the_day_of_the_clock_it_started_on_whatever_zone_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // 06:30 in Jakarta on 11-10 is still 23:30 on 11-09 in UTC and 18:30 on
    // 11-09 in New York: the person flew there afterwards.
    let early = NOV_10_JAKARTA + 6 * HOUR + 30 * MINUTE;
    store
        .record_pomodoro_session(finished(early, 25, 0))
        .unwrap();
    for zone in [
        JAKARTA,
        TimeZone::UTC,
        TimeZone::Fixed { minutes: -5 * 60 },
        TimeZone::Fixed { minutes: 14 * 60 },
        TimeZone::Local,
    ] {
        let page = store.pomodoro_history(&everything(zone), 10, None).unwrap();
        assert_eq!(dates(&page), ["2026-11-10"], "{zone:?}");
        let session = &page.items[0].sessions[0];
        assert_eq!(session.utc_offset_seconds, JAKARTA_OFFSET, "{zone:?}");
        let one_day = HistoryRange {
            from: Some("2026-11-10".into()),
            to: Some("2026-11-10".into()),
            zone,
        };
        let only = store.pomodoro_history(&one_day, 10, None).unwrap();
        assert_eq!(dates(&only), ["2026-11-10"], "{zone:?}");
    }
}

#[test]
fn a_session_stored_before_offsets_were_kept_goes_on_the_reading_zone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store = store_at(&path);
    // As schema v22 stored it: no offset and no time before Stop.
    let early = NOV_10_JAKARTA + 6 * HOUR + 30 * MINUTE;
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO pomodoro_sessions
               (id, started_at, planned_seconds, outcome, ended_at, created_at)
             VALUES ('old', ?1, 1500, 'stopped', ?1 + 600000, ?1)",
            [early],
        )
        .unwrap();
    let read_in = |zone| {
        let page = store.pomodoro_history(&everything(zone), 10, None).unwrap();
        let day = &page.items[0];
        let session = &day.sessions[0];
        (
            day.date.clone(),
            session.utc_offset_seconds,
            session.stopped_after_minutes,
        )
    };
    assert_eq!(
        read_in(JAKARTA),
        ("2026-11-10".into(), JAKARTA_OFFSET, None)
    );
    assert_eq!(read_in(TimeZone::UTC), ("2026-11-09".into(), 0, None));
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
            (
                Some(shown.id.clone()),
                Some("Synthetic report, renamed"),
                false
            ),
            (Some(set_aside.id.clone()), None, true),
            (Some(shelved.id.clone()), None, true),
            (None, None, false),
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
    assert_eq!(shown, vec![(25, 1), (25, 1), (25, 1), (25, 0)]);
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
    assert_eq!(day.sessions[0].stopped_after_minutes, Some(12));
}

/// A focus of `planned` minutes the person ended with Start break after
/// `after_seconds` (batch 3, Q43).
fn ended_early(started_at: i64, planned: i64, after_seconds: i64) -> EndedSession {
    EndedSession {
        outcome: SessionOutcome::EndedEarly,
        stopped_after_seconds: Some(after_seconds),
        ended_at: started_at + after_seconds * 1000,
        ..stopped(started_at, planned, 0)
    }
}

#[test]
fn a_focus_ended_early_adds_its_minutes_to_the_day_but_is_not_a_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    store
        .record_pomodoro_session(finished(NOV_10_JAKARTA + 9 * HOUR, 25, 0))
        .unwrap();
    // 10 min 40 s shows as 11 min, as its Progress Entry says it.
    store
        .record_pomodoro_session(ended_early(NOV_10_JAKARTA + 10 * HOUR, 25, 10 * 60 + 40))
        .unwrap();
    store
        .record_pomodoro_session(stopped(NOV_10_JAKARTA + 11 * HOUR, 25, 12))
        .unwrap();
    let day = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(
        (
            day.finished_sessions,
            day.focus_minutes,
            day.stopped_sessions
        ),
        (1, 36, 1)
    );
    let early = &day.sessions[1];
    assert_eq!(early.outcome, SessionOutcome::EndedEarly);
    assert_eq!(early.stopped_after_minutes, Some(11));
}

#[test]
fn the_all_time_totals_count_as_the_days_do() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    assert_eq!(
        store.pomodoro_totals().unwrap(),
        PomodoroTotals {
            finished_sessions: 0,
            focus_minutes: 0
        }
    );
    // Two days: 25 + 1 min and 25 min Finished, 11 min ended early, one Stopped.
    store
        .record_pomodoro_session(finished(NOV_10_JAKARTA + 9 * HOUR, 25, 40))
        .unwrap();
    store
        .record_pomodoro_session(ended_early(NOV_10_JAKARTA + 10 * HOUR, 25, 10 * 60 + 40))
        .unwrap();
    store
        .record_pomodoro_session(finished(NOV_10_JAKARTA + 33 * HOUR, 25, 20))
        .unwrap();
    store
        .record_pomodoro_session(stopped(NOV_10_JAKARTA + 34 * HOUR, 25, 12))
        .unwrap();
    let totals = store.pomodoro_totals().unwrap();
    assert_eq!(
        totals,
        PomodoroTotals {
            finished_sessions: 2,
            focus_minutes: 62
        }
    );
    let days = store
        .pomodoro_history(&everything(JAKARTA), 10, None)
        .unwrap()
        .items;
    assert_eq!(
        days.iter().map(|day| day.focus_minutes).sum::<i64>(),
        totals.focus_minutes
    );
    // Every interface may read them: sessions hold no user text.
    for interface in [Interface::Cli, Interface::Mcp] {
        let other = TaskStore::open_as(&dir.path().join("t.db"), interface).unwrap();
        assert_eq!(other.pomodoro_totals().unwrap(), totals);
    }
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
            matches!(error, TaskError::InvalidPomodoroRange),
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
    // 06:30 on 11-13 in Jakarta is 23:30 on 11-12 UTC.
    let early = NOV_10_JAKARTA + 3 * 24 * HOUR + 6 * HOUR + 30 * MINUTE;
    store
        .record_pomodoro_session(finished(early, 25, 0))
        .unwrap();
    // 22:00 on 11-12 in New York (UTC-4) is 02:00 on 11-13 UTC.
    store
        .record_pomodoro_session(at_offset(early + 2 * HOUR + 30 * MINUTE, -4))
        .unwrap();
    let one_day = |date: &str| HistoryRange {
        from: Some(date.into()),
        to: Some(date.into()),
        zone: TimeZone::UTC,
    };
    let starts = |date| -> Vec<i64> {
        let page = store.pomodoro_history(&one_day(date), 10, None).unwrap();
        page.items
            .iter()
            .flat_map(|day| day.sessions.iter().map(|s| s.started_at))
            .collect()
    };
    assert_eq!(starts("2026-11-13"), [early]);
    assert_eq!(starts("2026-11-12"), [early + 2 * HOUR + 30 * MINUTE]);
}

/// Read every page of `limit` days and the whole history at once: the pages
/// together are the whole, each day once and complete.
fn every_page(store: &TaskStore, limit: u32) -> Vec<Vec<(String, Vec<i64>)>> {
    let shape = |page: &tasknotes_core::pomodoro::PomodoroHistoryPage| -> Vec<(String, Vec<i64>)> {
        page.items
            .iter()
            .map(|day| {
                let starts = day.sessions.iter().map(|s| s.started_at).collect();
                (day.date.clone(), starts)
            })
            .collect()
    };
    let range = everything(TimeZone::UTC);
    let whole = shape(&store.pomodoro_history(&range, 100, None).unwrap());
    let mut pages = Vec::new();
    let mut cursor = None;
    loop {
        let page = store
            .pomodoro_history(&range, limit, cursor.as_deref())
            .unwrap();
        pages.push(shape(&page));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(pages.concat(), whole, "the pages are the whole history");
    pages
}

#[test]
fn paging_never_splits_or_skips_a_day_when_the_clock_goes_back_past_midnight() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // UTC-5 with daylight saving (UTC-4) that ends on 2026-11-01 at 00:30,
    // when the clock goes back to 23:30 of 10-31: for that hour the local
    // date is a day earlier than a moment before it.
    let (daylight, standard) = (-4, -5);
    for (started_at, hours) in [
        (1_793_458_800_000, daylight), // 10-31 11:00
        (1_793_506_500_000, daylight), // 11-01 00:15
        (1_793_508_300_000, standard), // 10-31 23:45, after the change
        (1_793_541_600_000, standard), // 11-01 09:00
    ] {
        store
            .record_pomodoro_session(at_offset(started_at, hours))
            .unwrap();
    }
    let pages = every_page(&store, 1);
    assert_eq!(
        pages,
        vec![
            vec![(
                "2026-11-01".into(),
                vec![1_793_506_500_000, 1_793_541_600_000]
            )],
            vec![(
                "2026-10-31".into(),
                vec![1_793_458_800_000, 1_793_508_300_000]
            )],
        ]
    );
}

#[test]
fn paging_never_misses_a_session_whose_clock_was_two_days_ahead() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // Kiritimati (UTC+14) at 00:10 on 11-13; ten minutes later the Mac is
    // set to Pago Pago (UTC-11), where it is 23:20 on 11-11; then back to
    // Kiritimati, 02:00 on 11-13. Going back in time, the date jumps two
    // days ahead between the second session and the first.
    let (kiritimati, pago_pago) = (14, -11);
    store
        .record_pomodoro_session(at_offset(1_794_478_200_000, kiritimati))
        .unwrap();
    store
        .record_pomodoro_session(at_offset(1_794_478_800_000, pago_pago))
        .unwrap();
    store
        .record_pomodoro_session(at_offset(1_794_484_800_000, kiritimati))
        .unwrap();
    for limit in [1, 2] {
        let days: Vec<_> = every_page(&store, limit).concat();
        assert_eq!(
            days,
            vec![
                (
                    "2026-11-13".into(),
                    vec![1_794_478_200_000, 1_794_484_800_000]
                ),
                ("2026-11-11".into(), vec![1_794_478_800_000]),
            ],
            "{limit} a page"
        );
    }
}

#[test]
fn a_capped_page_keeps_whole_days_and_continues_after_the_last_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_at(&dir.path().join("t.db"));
    // Three sessions on 11-13, two on 11-12, one on 11-11 and on 11-10.
    for (day, count) in [(3, 3), (2, 2), (1, 1), (0, 1)] {
        for n in 0..count {
            store
                .record_pomodoro_session(finished(
                    NOV_10_JAKARTA + day * 24 * HOUR + (9 + n) * HOUR,
                    25,
                    0,
                ))
                .unwrap();
        }
    }
    let range = everything(JAKARTA);
    let page = store.pomodoro_history(&range, 10, None).unwrap();
    assert_eq!(page.clone().capped(100), page, "nothing to cut");

    let first = page.clone().capped(4);
    assert_eq!(dates(&first), ["2026-11-13"], "3 + 2 would be 5");
    let rest = store
        .pomodoro_history(&range, 10, first.next_cursor.as_deref())
        .unwrap()
        .capped(4);
    assert_eq!(dates(&rest), ["2026-11-12", "2026-11-11", "2026-11-10"]);
    assert_eq!(rest.next_cursor, None);

    // The first day stays whole, however many sessions it holds.
    let one = page.capped(2);
    assert_eq!(dates(&one), ["2026-11-13"]);
    assert_eq!(one.items[0].sessions.len(), 3);
    assert_eq!(one.next_cursor.as_deref(), Some("p1|2026-11-13"));
}
