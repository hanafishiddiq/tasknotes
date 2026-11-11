import type { HistorySession, PomodoroDay, PomodoroView } from "../lib/pomodoro-api";
import { focusTime, sessionOutcome, sessionTimes } from "./PomodoroRecord";

const MINUTES_PER_DAY = 24 * 60;
/** The rail always shows at least this much of the working day, in steps of two hours. */
const EARLIEST_START = 8 * 60;
const LATEST_END = 18 * 60;
const STEP = 120;

/** Minutes since midnight on the clock the session started on. */
function minuteOfDay(session: HistorySession, time: number): number {
  const local = time + session.utcOffsetSeconds * 1000;
  return Math.floor((((local % 86_400_000) + 86_400_000) % 86_400_000) / 60_000);
}

/** Minutes since this computer's midnight, now. */
function minuteNow(now: Date): number {
  return now.getHours() * 60 + now.getMinutes() + now.getSeconds() / 60;
}

/** Seconds the running focus has been focused, as the shell counts them; null while none runs. */
function focusedSoFar(view: PomodoroView | null): number | null {
  if (!view || view.plannedSeconds === null) return null;
  if (view.phase === "focus") return view.plannedSeconds - view.remainingSeconds;
  if (view.phase === "focus_done") return view.plannedSeconds + view.overtimeSeconds;
  return null;
}

interface PomodoroDayRailProps {
  /** Today's sessions, as the core sent them; undefined before the first one. */
  day: PomodoroDay | undefined;
  view: PomodoroView | null;
  now: Date;
}

/**
 * Today as one line under the timer (batch 3, layout A): a mark for each
 * session at the time it ran (Finished and Ended Early filled, Stopped
 * hollow), the focus running now as a dim mark up to a thin line for now,
 * and today's counted focus at its end. The record below lists the same
 * sessions in words.
 */
export default function PomodoroDayRail({ day, view, now }: PomodoroDayRailProps) {
  const sessions = day?.sessions ?? [];
  const nowMinute = minuteNow(now);
  const first = Math.min(EARLIEST_START, ...sessions.map((session) => minuteOfDay(session, session.startedAt)));
  const from = Math.floor(first / STEP) * STEP;
  const to = Math.min(MINUTES_PER_DAY, Math.max(LATEST_END, Math.ceil((nowMinute + 60) / STEP) * STEP));
  const at = (minute: number) => `${((Math.min(Math.max(minute, from), to) - from) / (to - from)) * 100}%`;
  const span = (start: number, end: number) =>
    `${((Math.min(Math.max(end, from), to) - Math.min(Math.max(start, from), to)) / (to - from)) * 100}%`;

  const focused = focusedSoFar(view);
  const liveStart = focused === null ? null : nowMinute - focused / 60;
  const hours: number[] = [];
  for (let minute = from; minute <= to; minute += STEP) hours.push(minute / 60);

  return (
    <figure className="pomo-rail" aria-label="Today">
      <span className="pomo-rail-label" aria-hidden="true">Today</span>
      <div className="pomo-rail-line">
        {sessions.map((session) => {
          const start = minuteOfDay(session, session.startedAt);
          const end = Math.max(start + 1, start + (session.endedAt - session.startedAt) / 60_000);
          const words = `${sessionTimes(session)}, ${session.taskTitle ?? (session.taskRemoved ? "Task removed" : "No Task")}, ${sessionOutcome(session)}`;
          return (
            <span key={session.id} className="pomo-mark" data-outcome={session.outcome} role="img"
              aria-label={words} title={words} style={{ left: at(start), width: span(start, end) }} />
          );
        })}
        {liveStart !== null && (
          <span className="pomo-mark pomo-mark--now" role="img" aria-label="The focus running now"
            style={{ left: at(liveStart), width: span(liveStart, nowMinute) }} />
        )}
        <span className="pomo-rail-now" aria-hidden="true" style={{ left: at(nowMinute) }} />
      </div>
      <span className="pomo-rail-total">
        <span aria-hidden="true">{focusTime(day?.focusMinutes ?? 0)}</span>
        <span className="sr-only">{day?.focusMinutes ?? 0} minutes of focus today</span>
      </span>
      <span aria-hidden="true" />
      <span className="pomo-rail-hours" aria-hidden="true">
        {hours.map((hour) => <span key={hour}>{String(hour).padStart(2, "0")}</span>)}
      </span>
    </figure>
  );
}
