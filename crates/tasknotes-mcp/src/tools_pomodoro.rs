//! The Pomodoro daily history over MCP (Ticket 14): read-only, the CLI's
//! `pomodoro history` shapes over the core's one rule for what a day counts.
//! Only the desktop records sessions.

use schemars::JsonSchema;
use serde::Deserialize;
use tasknotes_cli::contract;
use tasknotes_core::calendar::TimeZone;
use tasknotes_core::pomodoro::HistoryRange;

use crate::tools::{Kind, ToolDef, limit, tool};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HistoryPage {
    /// The first local date to include, YYYY-MM-DD; the oldest when left out.
    #[serde(default)]
    from: Option<String>,
    /// The last local date to include, YYYY-MM-DD; the newest when left out.
    #[serde(default)]
    to: Option<String>,
    /// Days per page, 50 when left out.
    #[serde(default)]
    limit: Option<u32>,
    /// The next_cursor of the previous page.
    #[serde(default)]
    cursor: Option<String>,
}

pub fn tools() -> Vec<ToolDef> {
    vec![tool(
        "pomodoro_history",
        Kind::Read,
        "The person's Pomodoro history by local calendar day, newest day first, in pages of days. Each session's day and start are on the clock it started on: started_at (Unix ms) plus utc_offset_seconds, so travelling never moves it. Each day gives finished_sessions and focus_minutes (planned length plus the Overtime the person counted, of Finished sessions only) and lists every session that started that day, oldest first, including Stopped ones, which never count (stopped_after_minutes: the focus before Stop). A session that runs past midnight belongs to the day it started. task_title is null without a Task or while its Task is in Removed (task_removed).",
        |c, a: HistoryPage| {
            let range = HistoryRange {
                from: a.from,
                to: a.to,
                zone: TimeZone::Local,
            };
            let page = c
                .store
                .pomodoro_history(&range, limit(a.limit), a.cursor.as_deref())?;
            Ok(contract::success_page(
                page.items.iter().map(contract::pomodoro_day).collect(),
                page.next_cursor,
            ))
        },
    )]
}
