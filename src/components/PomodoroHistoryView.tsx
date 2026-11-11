import { useEffect, useId, useRef, useState } from "react";
import { LoaderCircle, Timer } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { localCalendarDay } from "../lib/deadlines";
import { pomodoroApi, type HistorySession, type PomodoroDay } from "../lib/pomodoro-api";
import { useLocalCalendarDay } from "../lib/use-local-calendar-day";

const DAY = new Intl.DateTimeFormat("en", { weekday: "long", month: "short", day: "numeric" });
const DAY_WITH_YEAR = new Intl.DateTimeFormat("en", {
  weekday: "long", month: "short", day: "numeric", year: "numeric",
});
// Read on UTC after the session's own offset is added: the clock it started on.
const START = new Intl.DateTimeFormat("en", { timeStyle: "short", timeZone: "UTC" });

/** "9:00 AM" on the clock the session started on, wherever this Mac is now. */
function startTime(session: HistorySession): string {
  return START.format(session.startedAt + session.utcOffsetSeconds * 1000);
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

/** What the day counts, as the core counted it. */
function dayTotal(day: PomodoroDay): string {
  const sessions = `${day.finishedSessions} session${day.finishedSessions === 1 ? "" : "s"}`;
  const stopped = day.stoppedSessions > 0 ? ` · ${day.stoppedSessions} stopped` : "";
  return `${sessions} · ${day.focusMinutes} min focus${stopped}`;
}

/** "25 + 7 min" with counted Overtime, "25 min" without; a Stopped one counts nothing. */
function sessionLength(session: HistorySession): string {
  if (session.outcome === "stopped") return `${session.plannedMinutes} min planned`;
  return session.overtimeMinutes > 0
    ? `${session.plannedMinutes} + ${session.overtimeMinutes} min`
    : `${session.plannedMinutes} min`;
}

/** "Finished"; "Stopped after 12 min", or "Stopped" for one stored before that time was kept. */
function sessionOutcome(session: HistorySession): string {
  if (session.outcome === "finished") return "Finished";
  const after = session.stoppedAfterMinutes;
  if (after === null) return "Stopped";
  return after === 0 ? "Stopped after <1 min" : `Stopped after ${after} min`;
}

interface PomodoroHistoryViewProps {
  hidden: boolean;
  /** Read again whenever this changes: a session was stored, or something changed outside. */
  refreshKey: number;
  onOpenTask: (taskId: string) => void;
}

/**
 * The Pomodoro page (Ticket 14): one section per local day, newest first,
 * with what the core counts for it and every session of it, oldest first. It
 * only shows the core's numbers and keeps no rule of its own. Older days load
 * on request.
 */
export default function PomodoroHistoryView({ hidden, refreshKey, onOpenTask }: PomodoroHistoryViewProps) {
  const today = useLocalCalendarDay();
  const [days, setDays] = useState<PomodoroDay[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Days on screen, so reading again keeps the older ones the person loaded.
  const shown = useRef(0);
  shown.current = days.length;
  // Bumped by every read from the top. An older page asked for before it
  // continues a list that is gone, so its answer is dropped: appending it
  // could skip the day that the new first page no longer reaches.
  const generation = useRef(0);

  useEffect(() => {
    if (hidden) return;
    const read = ++generation.current;
    const current = () => read === generation.current;
    const keep = shown.current;
    (async () => {
      let page = await pomodoroApi.history();
      let items = page.items;
      while (items.length < keep && page.nextCursor !== null) {
        page = await pomodoroApi.history(page.nextCursor);
        items = [...items, ...page.items];
      }
      if (!current()) return;
      setDays(items);
      setCursor(page.nextCursor);
      setError(null);
    })()
      .catch((reason) => { if (current()) setError("Couldn't load the Pomodoro history: " + errorMessage(reason)); })
      .finally(() => { if (current()) setLoading(false); });
    return () => { generation.current++; };
  }, [hidden, refreshKey]);

  async function loadMore() {
    if (!cursor || loadingMore) return;
    const read = generation.current;
    setLoadingMore(true);
    try {
      const page = await pomodoroApi.history(cursor);
      if (read !== generation.current) return;
      setDays((current) => {
        const seen = new Set(current.map((day) => day.date));
        return [...current, ...page.items.filter((day) => !seen.has(day.date))];
      });
      setCursor(page.nextCursor);
    } catch (reason) {
      if (read === generation.current) setError("Couldn't load more days: " + errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  return (
    <main className="main-view" hidden={hidden}>
      <div className="main-scroll">
        <section className="content-wrap pomodoro-history" aria-label="Pomodoro history">
          <div className="page-title"><h1>Pomodoro</h1></div>
          <p className="pomodoro-history-hint">
            Each day counts its finished sessions: the planned focus plus any Overtime you counted.
            Stopped sessions are listed but not counted. A session keeps the day and time of the
            clock it started on, even after you travel, and a session that runs past midnight
            stays on the day it started.
          </p>
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
                <p>Each focus you finish or stop with the Pomodoro timer is listed here by day.</p>
              </div>
            )
          ) : (
            days.map((day) => (
              <HistoryDay key={day.date} day={day} label={dayLabel(day.date, today)} onOpenTask={onOpenTask} />
            ))
          )}
          {cursor && (
            <button className="button button-small pomodoro-more" type="button" disabled={loadingMore}
              onClick={() => void loadMore()}>{loadingMore ? "Loading..." : "Load more days"}</button>
          )}
        </section>
      </div>
    </main>
  );
}

function HistoryDay({ day, label, onOpenTask }: {
  day: PomodoroDay; label: string; onOpenTask: (taskId: string) => void;
}) {
  const heading = useId();
  return (
    <section className="pomodoro-day" aria-labelledby={heading}>
      <div className="pomodoro-day-head">
        <h2 id={heading}>{label}</h2>
        <span className="pomodoro-day-total">{dayTotal(day)}</span>
      </div>
      <ul className="pomodoro-sessions">
        {day.sessions.map((session) => {
          const { taskId, taskTitle } = session;
          return (
            <li key={session.id} className="pomodoro-session" data-outcome={session.outcome}>
              <span className="pomodoro-session-time">{startTime(session)}</span>
              <span className="pomodoro-session-task">
                {taskId !== null && taskTitle !== null ? (
                  <button type="button" className="pomodoro-task-link"
                    onClick={() => onOpenTask(taskId)}>{taskTitle}</button>
                ) : (
                  <span className="pomodoro-no-task">{session.taskRemoved ? "Task removed" : "No Task"}</span>
                )}
              </span>
              <span className="pomodoro-session-length">{sessionLength(session)}</span>
              <span className="pomodoro-session-outcome">{sessionOutcome(session)}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
