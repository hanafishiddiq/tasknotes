import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "./invoke";

/** Sent by the desktop shell whenever what the Pomodoro timer shows changes, every second while it counts. */
export const POMODORO_CHANGED = "tasknotes://pomodoro";

export interface PomodoroDurations {
  focusMinutes: number;
  shortBreakMinutes: number;
  longBreakMinutes: number;
  sessionsBeforeLongBreak: number;
}

export interface Limit { min: number; max: number }

/** What each length may be, as the core decides (`tasknotes_core::pomodoro::DurationLimits`). */
export type DurationLimits = Record<keyof PomodoroDurations, Limit>;

/**
 * Where the timer is, with the whole seconds to show, worked out by the
 * shell so the window only formats them.
 */
export type PomodoroPhase =
  | { phase: "idle" }
  /** `paused`: held by Pause until Resume; paused time never counts. */
  | { phase: "focus"; taskId: string | null; remainingSeconds: number; paused: boolean }
  /** The planned end passed: Overtime runs until Start break, Count overtime or Stop. */
  | { phase: "focus_done"; taskId: string | null; overtimeSeconds: number }
  | { phase: "break"; long: boolean; remainingSeconds: number; paused: boolean }
  | { phase: "break_done"; long: boolean };

export type PomodoroView = PomodoroPhase & {
  /** The menu bar text, e.g. "🍅 12:34". */
  title: string | null;
  /** The length of the focus or break under way as it was planned when it started; null otherwise. */
  plannedSeconds: number | null;
  /** The beads: Finished sessions in the cycle toward the long break, or the whole cycle a long break closed while it lasts. */
  finishedInCycle: number;
  /** When the focus under way started, in wall-clock milliseconds; null without one. */
  focusStartedAt: number | null;
  durations: PomodoroDurations;
  limits: DurationLimits;
  /** The Task the next focus after a break is for. */
  lastTaskId: string | null;
  /** Pomodoro notifications make no sound. */
  silent: boolean;
  /** Why an ended session could not be stored; the shell keeps it to try again. */
  saveError: string | null;
};

export interface PomodoroChanged {
  view: PomodoroView;
  /** A Task that gained a Pomodoro Progress Entry. */
  taskChanged: string | null;
  /** A session was stored just now: the history has a new entry. */
  sessionStored: boolean;
}

/** A stored session as the history shows it (`tasknotes_core::pomodoro::HistorySession`). */
export interface HistorySession {
  id: string;
  startedAt: number;
  endedAt: number;
  /**
   * Seconds east of UTC of the clock the session started on: its day and start
   * time stay there wherever the Mac is now.
   */
  utcOffsetSeconds: number;
  /**
   * Finished counts as a session; Ended Early (a focus ended with Start break
   * before its end) counts only its minutes; Stopped counts nothing.
   */
  outcome: "finished" | "ended_early" | "stopped";
  /** Whole minutes, rounded as the session's Progress Entry says them. */
  plannedMinutes: number;
  /** The Overtime the person counted, in whole minutes; 0 for a Stopped session. */
  overtimeMinutes: number;
  /**
   * A Stopped session's focus before Stop, or an Ended Early one's before Start
   * break, in whole minutes, sleep and pauses never included; null for a Finished
   * one and for a Stopped one stored before this was kept.
   */
  stoppedAfterMinutes: number | null;
  taskId: string | null;
  /** Null without a Task, or while its Task is in Removed (`taskRemoved`). */
  taskTitle: string | null;
  taskRemoved: boolean;
}

/**
 * One local day with sessions, as the core counts it: only Finished sessions
 * count as sessions; focus is their planned length plus counted Overtime, plus
 * the minutes of the Ended Early ones.
 */
export interface PomodoroDay {
  /** YYYY-MM-DD on the clock its sessions started on; a session belongs to the day it started. */
  date: string;
  finishedSessions: number;
  focusMinutes: number;
  stoppedSessions: number;
  /** In the order they started: the day reads as a log. */
  sessions: HistorySession[];
}

export interface PomodoroHistoryPage {
  /** Newest day first. */
  items: PomodoroDay[];
  nextCursor: string | null;
}

/** Every session ever stored, counted as the days are (batch 3, Q42). */
export interface PomodoroTotals {
  finishedSessions: number;
  focusMinutes: number;
}

/** Days the history page reads at a time. */
export const HISTORY_PAGE_DAYS = 14;

export type PomodoroCommand =
  | { action: "start_focus"; taskId: string | null }
  /** After a break, for the shell's last Task. */
  | { action: "start_next_focus" }
  | { action: "start_break" }
  | { action: "count_overtime" }
  /** Holds a running focus or break; Resume lets it run again. */
  | { action: "pause" }
  | { action: "resume" }
  /** Before the planned end: the focus ends early (its minutes count) and the break starts. */
  | { action: "break_early" }
  | { action: "stop" }
  | { action: "retry_save" }
  | { action: "discard_unsaved" };

export const pomodoroApi = {
  state: () => invoke<PomodoroView>("pomodoro_state"),
  /** The daily history, newest day first; `cursor` continues after an earlier page. */
  history: (cursor: string | null = null) => invoke<PomodoroHistoryPage>(
    "pomodoro_history", { limit: HISTORY_PAGE_DAYS, cursor },
  ),
  /** Every session ever stored, counted as the days are. */
  totals: () => invoke<PomodoroTotals>("pomodoro_totals"),
  command: (command: PomodoroCommand) => invoke<PomodoroView>("pomodoro_command", { command }),
  setDurations: (durations: PomodoroDurations) =>
    invoke<PomodoroView>("set_pomodoro_durations", { durations }),
  /** Silent: Pomodoro notifications make no sound. Saved by the shell. */
  setSilent: (silent: boolean) => invoke<PomodoroView>("set_pomodoro_silent", { silent }),
  /** Whether the one-time notification-style tip is due now; given out once. */
  takeTip: () => invoke<boolean>("take_pomodoro_tip"),
  openNotificationSettings: () => invoke<void>("open_notification_settings"),
};

export function listenForPomodoro(onChange: (changed: PomodoroChanged) => void): Promise<UnlistenFn> {
  return listen<PomodoroChanged>(POMODORO_CHANGED, (event) => onChange(event.payload));
}

/** Formats whole seconds as mm:ss, minutes past 59 kept as minutes ("90:00"), as the menu bar shows them. */
export function clock(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}
