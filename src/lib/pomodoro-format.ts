import { clock, type HistorySession, type PomodoroView } from "./pomodoro-api";

/**
 * How the Pomodoro surfaces (the sidebar block, the page, its rail and
 * record) say the shell's and the core's numbers. Formatting only: what
 * counts and when the timer moves are decided in Rust.
 */

// Read on UTC after the session's own offset is added: the clock it started on.
const CLOCK = new Intl.DateTimeFormat("en", { timeStyle: "short", timeZone: "UTC" });

/** Milliseconds on the clock the session started on, read as UTC. */
export const onItsClock = (session: HistorySession, time: number) => time + session.utcOffsetSeconds * 1000;

/** "9:00 AM – 9:25 AM" on the clock the session started on, wherever this Mac is now. */
export function sessionTimes(session: HistorySession): string {
  const start = onItsClock(session, session.startedAt);
  const end = Math.max(start, onItsClock(session, session.endedAt));
  return `${CLOCK.format(start)} – ${CLOCK.format(end)}`;
}

/** "1 minute", "57 minutes": a count in words, for screen readers. */
export const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;

/** "57m", "1h 07m". */
export function focusTime(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
}

/** How a session ended, in words. */
export function sessionOutcome(session: HistorySession): string {
  if (session.outcome === "finished") return "Finished";
  const after = session.stoppedAfterMinutes;
  if (session.outcome === "ended_early") return `Break started early, ${after ?? 0} min counted`;
  if (after === null) return "Stopped";
  return after === 0 ? "Stopped after <1 min" : `Stopped after ${after} min`;
}

/** The Task a session was for, by its title; or why there is none to show. */
export function sessionTask(session: HistorySession): string {
  return session.taskTitle ?? (session.taskRemoved ? "Task removed" : "No Task");
}

/**
 * The time a running focus or break shows ("12:34" left, "+07:12" of
 * Overtime) and what screen readers hear for it; null in the other phases,
 * which each surface says in its own way.
 */
export function runningTime(view: PomodoroView): { text: string; label: string } | null {
  switch (view.phase) {
    case "focus":
    case "break":
      return { text: clock(view.remainingSeconds), label: `${clock(view.remainingSeconds)} left` };
    case "focus_done":
      return { text: `+${clock(view.overtimeSeconds)}`, label: `Overtime ${clock(view.overtimeSeconds)}` };
    default:
      return null;
  }
}
