import type { HistorySession, PomodoroDay, PomodoroView } from "../lib/pomodoro-api";
import { focusTime, onItsClock, plural, sessionOutcome, sessionTask, sessionTimes } from "../lib/pomodoro-format";
import { useWallMinute } from "../lib/use-wall-minute";

const MINUTES_PER_DAY = 24 * 60;
/** The rail always shows at least this much of the working day, in steps of two hours. */
const EARLIEST_START = 8 * 60;
const LATEST_END = 18 * 60;
const STEP = 120;
const DAY_MS = 86_400_000;

/** Minutes since midnight on the clock the session started on. */
function minuteOfDay(session: HistorySession, time: number): number {
  return Math.floor(((onItsClock(session, time) % DAY_MS) + DAY_MS) % DAY_MS / 60_000);
}

/** Minutes since this computer's midnight. */
function minuteHere(time: Date): number {
  return time.getHours() * 60 + time.getMinutes() + time.getSeconds() / 60;
}

interface PomodoroDayRailProps {
  /** Today's sessions, as the core sent them; undefined before the first one. */
  day: PomodoroDay | undefined;
  view: PomodoroView | null;
}

/**
 * Today as one line under the timer (batch 3, layout A): a mark for each
 * session at the time it ran (Finished and Ended Early filled, Stopped
 * hollow), the focus under way as a dim mark from its start to a thin line
 * for now, and today's counted focus at its end. The record below lists the
 * same sessions in words.
 */
export default function PomodoroDayRail({ day, view }: PomodoroDayRailProps) {
  const now = useWallMinute();
  const sessions = day?.sessions ?? [];
  const nowMinute = minuteHere(now);
  const first = Math.min(EARLIEST_START, ...sessions.map((session) => minuteOfDay(session, session.startedAt)));
  const from = Math.floor(first / STEP) * STEP;
  const to = Math.min(MINUTES_PER_DAY, Math.max(LATEST_END, Math.ceil((nowMinute + 60) / STEP) * STEP));
  const clamp = (minute: number) => Math.min(Math.max(minute, from), to);
  const at = (minute: number) => `${((clamp(minute) - from) / (to - from)) * 100}%`;
  const span = (start: number, end: number) => `${((clamp(end) - clamp(start)) / (to - from)) * 100}%`;

  // The shell's start of the focus under way, so a pause or sleep never moves the mark.
  const liveStart = view?.focusStartedAt == null ? null : minuteHere(new Date(view.focusStartedAt));
  const hours: number[] = [];
  for (let minute = from; minute <= to; minute += STEP) hours.push(minute / 60);

  return (
    <figure className="pomo-rail" aria-label="Today">
      <span className="pomo-rail-label" aria-hidden="true">Today</span>
      <div className="pomo-rail-line">
        {sessions.map((session) => {
          const start = minuteOfDay(session, session.startedAt);
          const end = Math.max(start + 1, start + (session.endedAt - session.startedAt) / 60_000);
          const words = `${sessionTimes(session)}, ${sessionTask(session)}, ${sessionOutcome(session)}`;
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
        <span className="sr-only">{plural(day?.focusMinutes ?? 0, "minute")} of focus today</span>
      </span>
      <span aria-hidden="true" />
      <span className="pomo-rail-hours" aria-hidden="true">
        {hours.map((hour) => <span key={hour}>{String(hour).padStart(2, "0")}</span>)}
      </span>
    </figure>
  );
}
