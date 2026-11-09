import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage } from "./error-message";
import {
  listenForPomodoro, pomodoroApi, type PomodoroCommand, type PomodoroDurations, type PomodoroView,
} from "./pomodoro-api";

export interface PomodoroControl {
  /** Null until the shell answers, and where there is no shell timer. */
  view: PomodoroView | null;
  /** The shell's clock minus this window's, so countdowns match the menu bar. */
  skew: number;
  error: string | null;
  /** The one-time notification-style tip is showing. */
  tip: boolean;
  run: (command: PomodoroCommand) => Promise<void>;
  dismissTip: () => void;
  openNotificationSettings: () => Promise<void>;
  /** Saves the lengths; resolves to the refusal, or null when saved. */
  setDurations: (durations: PomodoroDurations) => Promise<string | null>;
}

interface Options {
  /** Silent mode: the focus-end notification makes no sound. */
  silent: boolean;
  /** A Task gained a Pomodoro Progress Entry. */
  onTaskChanged: (taskId: string) => void;
}

/**
 * The window's view of the shell's Pomodoro timer. The timer itself runs in
 * the shell, so it keeps going with the window closed; this only shows it
 * and sends clicks. It never touches sound: Ambient rain is independent.
 */
export function usePomodoro({ silent, onTaskChanged }: Options): PomodoroControl {
  const [view, setView] = useState<PomodoroView | null>(null);
  const [skew, setSkew] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [tip, setTip] = useState(false);
  const tipAsked = useRef(false);
  const taskChanged = useRef(onTaskChanged);
  taskChanged.current = onTaskChanged;

  const show = useCallback((next: PomodoroView) => {
    setView(next);
    setSkew(next.now - Date.now());
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listenForPomodoro((changed) => {
      if (!active) return;
      show(changed.view);
      if (changed.taskChanged) taskChanged.current(changed.taskChanged);
    }).then((stop) => {
      if (active) unlisten = stop; else stop();
    }).catch(() => {});
    pomodoroApi.state().then((current) => { if (active) show(current); }).catch(() => {});
    return () => {
      active = false;
      unlisten?.();
    };
  }, [show]);

  const available = view !== null;
  useEffect(() => {
    if (available) pomodoroApi.setSilent(silent).catch(() => {});
  }, [available, silent]);

  const run = useCallback(async (command: PomodoroCommand) => {
    setError(null);
    try {
      show(await pomodoroApi.command(command));
    } catch (reason) {
      setError(errorMessage(reason));
    }
    if (command.action === "start_focus" && !tipAsked.current) {
      tipAsked.current = true;
      pomodoroApi.takeTip().then((due) => { if (due) setTip(true); }).catch(() => {});
    }
  }, [show]);

  const openNotificationSettings = useCallback(async () => {
    try {
      await pomodoroApi.openNotificationSettings();
      setTip(false);
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }, []);

  const setDurations = useCallback(async (durations: PomodoroDurations) => {
    try {
      show(await pomodoroApi.setDurations(durations));
      return null;
    } catch (reason) {
      return errorMessage(reason);
    }
  }, [show]);

  return {
    view, skew, error, tip, run, openNotificationSettings, setDurations,
    dismissTip: () => setTip(false),
  };
}
