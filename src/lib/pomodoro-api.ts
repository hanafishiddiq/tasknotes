import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "./invoke";

/** Sent by the desktop shell whenever the Pomodoro timer changes phase. */
export const POMODORO_CHANGED = "tasknotes://pomodoro";

export interface PomodoroDurations {
  focusMinutes: number;
  shortBreakMinutes: number;
  longBreakMinutes: number;
  sessionsBeforeLongBreak: number;
}

/** The limits the core accepts (`tasknotes_core::pomodoro`). */
export const DURATION_LIMITS: Record<keyof PomodoroDurations, { min: number; max: number }> = {
  focusMinutes: { min: 1, max: 180 },
  shortBreakMinutes: { min: 1, max: 60 },
  longBreakMinutes: { min: 1, max: 60 },
  sessionsBeforeLongBreak: { min: 1, max: 12 },
};

/** Where the timer is; times are Unix milliseconds on the shell's clock. */
export type PomodoroPhase =
  | { phase: "idle" }
  | { phase: "focus"; taskId: string | null; startedAt: number; endsAt: number }
  /** The planned end passed; waiting for Start break or Keep focusing. */
  | { phase: "focus_done"; taskId: string | null; startedAt: number; endsAt: number }
  /** Keep focusing: Overtime counts from `endsAt`, the planned end. */
  | { phase: "overtime"; taskId: string | null; startedAt: number; endsAt: number }
  | { phase: "break"; long: boolean; startedAt: number; endsAt: number }
  | { phase: "break_done"; long: boolean };

export type PomodoroView = PomodoroPhase & {
  /** The menu bar text, e.g. "🍅 12:34". */
  title: string | null;
  durations: PomodoroDurations;
  /** The Task the menu bar's Start focus uses next. */
  lastTaskId: string | null;
  /** The shell's clock when this was read. */
  now: number;
};

export interface PomodoroChanged {
  view: PomodoroView;
  /** A Task that gained a Pomodoro Progress Entry. */
  taskChanged: string | null;
}

export type PomodoroCommand =
  | { action: "start_focus"; taskId: string | null }
  | { action: "keep_focusing" }
  | { action: "start_break" }
  | { action: "stop" };

export const pomodoroApi = {
  state: () => invoke<PomodoroView>("pomodoro_state"),
  command: (command: PomodoroCommand) => invoke<PomodoroView>("pomodoro_command", { command }),
  setDurations: (durations: PomodoroDurations) =>
    invoke<PomodoroView>("set_pomodoro_durations", { durations }),
  /** Silent mode: the focus-end notification makes no sound. */
  setSilent: (silent: boolean) => invoke<void>("set_pomodoro_silent", { silent }),
  /** Whether the one-time notification-style tip is due now; given out once. */
  takeTip: () => invoke<boolean>("take_pomodoro_tip"),
  openNotificationSettings: () => invoke<void>("open_notification_settings"),
};

export function listenForPomodoro(onChange: (changed: PomodoroChanged) => void): Promise<UnlistenFn> {
  return listen<PomodoroChanged>(POMODORO_CHANGED, (event) => onChange(event.payload));
}

/** mm:ss, minutes past 59 kept as minutes ("90:00"), as the menu bar shows them. */
export function clock(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(whole / 60);
  return `${String(minutes).padStart(2, "0")}:${String(whole % 60).padStart(2, "0")}`;
}

/**
 * The time to show at `now`: time left, rounded up so 00:00 never shows
 * before the end, or Overtime since the planned end. Mirrors the shell's
 * menu bar title, so the window and the menu bar agree.
 */
export function shownTime(view: PomodoroPhase, now: number): string | null {
  switch (view.phase) {
    case "focus":
    case "break":
      return clock(Math.ceil(Math.max(0, view.endsAt - now) / 1000));
    case "overtime":
      return `+${clock(Math.max(0, now - view.endsAt) / 1000)}`;
    default:
      return null;
  }
}
