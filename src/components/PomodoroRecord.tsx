import { useId } from "react";
import { LoaderCircle, Timer } from "lucide-react";
import { localCalendarDay } from "../lib/deadlines";
import type { HistorySession, PomodoroDay } from "../lib/pomodoro-api";
import type { PomodoroHistory } from "../lib/use-pomodoro-history";

const DAY = new Intl.DateTimeFormat("en", { weekday: "long", month: "short", day: "numeric" });
const DAY_WITH_YEAR = new Intl.DateTimeFormat("en", {
  weekday: "long", month: "short", day: "numeric", year: "numeric",
});
// Read on UTC after the session's own offset is added: the clock it started on.
const CLOCK = new Intl.DateTimeFormat("en", { timeStyle: "short", timeZone: "UTC" });

/** Milliseconds on the clock the session started on, read as UTC. */
const onItsClock = (session: HistorySession, time: number) => time + session.utcOffsetSeconds * 1000;

/** "9:00 AM – 9:25 AM" on the clock the session started on, wherever this Mac is now. */
export function sessionTimes(session: HistorySession): string {
  const start = onItsClock(session, session.startedAt);
  const end = Math.max(start, onItsClock(session, session.endedAt));
  return `${CLOCK.format(start)} – ${CLOCK.format(end)}`;
}

/** Noon on a YYYY-MM-DD date on this computer's clock, never near a midnight. */
function noonOf(date: string, shiftDays = 0): Date {
  const [year, month, day] = date.split("-").map(Number);
  return new Date(year, month - 1, day + shiftDays, 12);
}

/** "Today", "Yesterday", "Tuesday, Nov 10", or with the year when it is not this year's. */
function dayLabel(date: string, today: string): string {
  if (date === today) return "Today";
  if (date === localCalendarDay(noonOf(today, -1))) return "Yesterday";
  return (date.slice(0, 4) === today.slice(0, 4) ? DAY : DAY_WITH_YEAR).format(noonOf(date));
}

/** "57m", "1h 07m". */
export function focusTime(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
}

const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;

/** What the day counts, as the core counted it: short on screen, in words for screen readers. */
function dayTotal(day: PomodoroDay): { shown: string; said: string } {
  const stopped = day.stoppedSessions > 0 ? ` · ${day.stoppedSessions} stopped` : "";
  return {
    shown: `${day.finishedSessions} · ${focusTime(day.focusMinutes)}${stopped}`,
    said: `${plural(day.finishedSessions, "session")}, ${plural(day.focusMinutes, "minute")} of focus`
      + (day.stoppedSessions > 0 ? `, ${day.stoppedSessions} stopped` : ""),
  };
}

/** The minutes a session shows: "25 + 7m", "25m"; an Ended Early or Stopped one its focus before it ended. */
function sessionLength(session: HistorySession): string {
  if (session.outcome === "finished") {
    return session.overtimeMinutes > 0
      ? `${session.plannedMinutes} + ${session.overtimeMinutes}m`
      : `${session.plannedMinutes}m`;
  }
  const after = session.stoppedAfterMinutes;
  if (after === null) return "";
  return after === 0 ? "<1m" : `${after}m`;
}

/** How it ended, in words: the dot's name and the row's tooltip. */
export function sessionOutcome(session: HistorySession): string {
  if (session.outcome === "finished") return "Finished";
  const after = session.stoppedAfterMinutes;
  if (session.outcome === "ended_early") return `Break started early, ${after ?? 0} min counted`;
  if (after === null) return "Stopped";
  return after === 0 ? "Stopped after <1 min" : `Stopped after ${after} min`;
}

interface PomodoroRecordProps {
  history: PomodoroHistory;
  today: string;
  onOpenTask: (taskId: string) => void;
}

/**
 * The focus record under the timer (batch 3, Ticket 04, layout A): the
 * all-time totals, then one section per local day, newest first, with what
 * the core counts for it and every session of it, oldest first. It only
 * shows the core's numbers and keeps no rule of its own.
 */
export default function PomodoroRecord({ history, today, onOpenTask }: PomodoroRecordProps) {
  const { days, totals, loading, loadingMore, hasMore, error, loadMore } = history;
  const heading = useId();
  return (
    <section className="pomo-record" aria-labelledby={heading}>
      <div className="pomo-record-head">
        <h2 id={heading}>Focus record</h2>
        {totals && (totals.finishedSessions > 0 || totals.focusMinutes > 0) && (
          <span className="pomo-record-all">
            <span aria-hidden="true">
              {plural(totals.finishedSessions, "session")} · {focusTime(totals.focusMinutes)} all time
            </span>
            <span className="sr-only">
              All time: {plural(totals.finishedSessions, "session")}, {plural(totals.focusMinutes, "minute")} of focus
            </span>
          </span>
        )}
      </div>
      {error && <div role="alert" className="error-banner"><span>{error}</span></div>}
      {loading ? (
        <div className="loading-state" role="status">
          <LoaderCircle className="spin" size={16} /> Loading the Pomodoro history...
        </div>
      ) : days.length === 0 ? (
        !error && (
          <div className="empty-state">
            <Timer size={26} strokeWidth={1.5} aria-hidden="true" />
            <h2>No Pomodoro sessions yet</h2>
            <p>Each focus you finish, end early or stop is listed here by day.</p>
          </div>
        )
      ) : (
        days.map((day) => (
          <RecordDay key={day.date} day={day} label={dayLabel(day.date, today)} onOpenTask={onOpenTask} />
        ))
      )}
      {hasMore && (
        <button className="button button-small pomo-more" type="button" disabled={loadingMore}
          onClick={() => void loadMore()}>{loadingMore ? "Loading..." : "Load more days"}</button>
      )}
    </section>
  );
}

function RecordDay({ day, label, onOpenTask }: {
  day: PomodoroDay; label: string; onOpenTask: (taskId: string) => void;
}) {
  const heading = useId();
  const total = dayTotal(day);
  return (
    <section className="pomo-day" aria-labelledby={heading}>
      <div className="pomo-day-head">
        <h3 id={heading}>{label}</h3>
        <span className="pomo-day-total">
          <span aria-hidden="true">{total.shown}</span>
          <span className="sr-only">{total.said}</span>
        </span>
      </div>
      <ul className="pomo-sessions">
        {day.sessions.map((session) => {
          const { taskId, taskTitle } = session;
          const outcome = sessionOutcome(session);
          return (
            <li key={session.id} className="pomo-session" data-outcome={session.outcome} title={outcome}>
              <span className="pomo-dot" role="img" aria-label={outcome} />
              <span className="pomo-session-time">{sessionTimes(session)}</span>
              <span className="pomo-session-task">
                {taskId !== null && taskTitle !== null ? (
                  <button type="button" className="pomo-task-link"
                    onClick={() => onOpenTask(taskId)}>{taskTitle}</button>
                ) : (
                  <span className="pomo-no-task">{session.taskRemoved ? "Task removed" : "No Task"}</span>
                )}
              </span>
              <span className="pomo-session-length">{sessionLength(session)}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
