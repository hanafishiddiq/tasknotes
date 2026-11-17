import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage } from "./error-message";
import {
  listenForPomodoro, pomodoroApi, type NotificationPermission, type PomodoroCommand, type PomodoroDurations,
  type PomodoroView,
} from "./pomodoro-api";

export interface PomodoroControl {
  /** Null until the shell answers, and where there is no shell timer. */
  view: PomodoroView | null;
  /** Why the last click could not reach the shell. */
  error: string | null;
  /** The one-time notification-style tip is showing. */
  tip: boolean;
  /** Notifications are off or never allowed: Pomodoro runs, but cannot say when a focus or break ends. */
  notificationsOff: boolean;
  /** Waiting for macOS (its prompt may sit unanswered); the button waits too. */
  turningOn: boolean;
  turnOnNotifications: () => Promise<void>;
  run: (command: PomodoroCommand) => Promise<void>;
  dismissTip: () => void;
  openNotificationSettings: () => Promise<void>;
  /** Saves the lengths; resolves to the refusal, or null when saved. */
  setDurations: (durations: PomodoroDurations) => Promise<string | null>;
  /** Saves Silent; resolves to the refusal, or null when saved. */
  setSilent: (silent: boolean) => Promise<string | null>;
}

interface Options {
  /** A Task gained a Pomodoro Progress Entry. */
  onTaskChanged: (taskId: string) => void;
  /** The shell stored an ended session. */
  onSessionStored?: () => void;
}

/**
 * The window's view of the shell's Pomodoro timer. The timer runs in the
 * shell, so it keeps going with the window closed; the shell sends what to
 * show whenever it changes, so this keeps no clock of its own, only shows
 * the view and sends clicks. It never touches sound: Ambient rain and the
 * interaction sounds are independent.
 */
export function usePomodoro({ onTaskChanged, onSessionStored }: Options): PomodoroControl {
  const [view, setView] = useState<PomodoroView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tip, setTip] = useState(false);
  const tipAsked = useRef(false);
  const [permission, setPermission] = useState<NotificationPermission>("unknown");
  const notificationsOff = permission === "off" || permission === "not_asked";
  const [turningOn, setTurningOn] = useState(false);
  const offNow = useRef(notificationsOff);
  offNow.current = notificationsOff;
  const taskChanged = useRef(onTaskChanged);
  taskChanged.current = onTaskChanged;
  const sessionStored = useRef(onSessionStored);
  sessionStored.current = onSessionStored;

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listenForPomodoro((changed) => {
      if (!active) return;
      setView(changed.view);
      if (changed.taskChanged) taskChanged.current(changed.taskChanged);
      if (changed.sessionStored) sessionStored.current?.();
    }).then((stop) => {
      if (active) unlisten = stop; else stop();
    }).catch(() => {});
    pomodoroApi.state().then((current) => { if (active) setView(current); }).catch(() => {});
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  // Read on opening, and again whenever the window comes back to the front:
  // the person may have just turned them on in System Settings.
  useEffect(() => {
    let active = true;
    const read = () => {
      pomodoroApi.notificationPermission()
        .then((now) => { if (active) setPermission(now); })
        .catch(() => {});
    };
    read();
    window.addEventListener("focus", read);
    return () => {
      active = false;
      window.removeEventListener("focus", read);
    };
  }, []);

  const turnOnNotifications = useCallback(async () => {
    setError(null);
    setTurningOn(true);
    try {
      setPermission(await pomodoroApi.turnOnNotifications());
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setTurningOn(false);
    }
  }, []);

  const run = useCallback(async (command: PomodoroCommand) => {
    setError(null);
    try {
      setView(await pomodoroApi.command(command));
    } catch (reason) {
      setError(errorMessage(reason));
    }
    // The tip about the Persistent style waits until notifications are on at all.
    if ((command.action === "start_focus" || command.action === "start_next_focus") && !tipAsked.current
      && !offNow.current) {
      tipAsked.current = true;
      pomodoroApi.takeTip().then((due) => { if (due) setTip(true); }).catch(() => {});
    }
  }, []);

  const openNotificationSettings = useCallback(async () => {
    try {
      await pomodoroApi.openNotificationSettings();
      setTip(false);
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }, []);

  const save = useCallback(async (change: () => Promise<PomodoroView>) => {
    try {
      setView(await change());
      return null;
    } catch (reason) {
      return errorMessage(reason);
    }
  }, []);

  return {
    view, error, tip, run, openNotificationSettings, notificationsOff, turningOn, turnOnNotifications,
    setDurations: (durations) => save(() => pomodoroApi.setDurations(durations)),
    setSilent: (silent) => save(() => pomodoroApi.setSilent(silent)),
    dismissTip: () => setTip(false),
  };
}
