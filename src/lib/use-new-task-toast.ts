import { useCallback, useEffect, useRef, useState } from "react";
import { useHoldTimer, type HoldReason } from "./use-hold-timer";

/** How long the toast stays before it fades, not counting time held under the pointer or focus. */
export const TOAST_MS = 6000;
/** The fade out, matching DESIGN.md's 120ms fades. */
export const TOAST_FADE_MS = 120;
/** How long an opened Task keeps its yellow flash. */
export const FLASH_MS = 1500;

export interface NewTaskToastState {
  /** The Task the toast points to, or null when no toast shows. */
  taskId: string | null;
  /** Changes with every add, so the toast is announced again for the latest Task. */
  seq: number;
  /** Fading out; it leaves after TOAST_FADE_MS. */
  leaving: boolean;
  /** The Task flashing yellow after the toast opened it. */
  flashId: string | null;
  /** The Task the toast just opened, for the list to scroll to on its next try only. */
  reveal: { taskId: string } | null;
  /** A Task was just created in this window: point the toast at it. */
  added(taskId: string): void;
  /** The pointer or keyboard focus is on the toast (true) or left it (false). */
  hold(reason: HoldReason, held: boolean): void;
  /** Closes the toast and flashes its Task; returns that Task, for the view to open. */
  open(): string | null;
  /** The view now shows this Task: the list scrolls to it once, if it is there. */
  revealTask(taskId: string): void;
  /**
   * Drops `reveal`, hit or miss, and on any change of view: it is never replayed,
   * so the list does not move by itself. Stable across renders.
   */
  clearReveal(): void;
  /** This Task was moved to Removed: a toast, flash or scroll for it goes, as there is nothing to open. */
  forget(taskId: string): void;
}

/**
 * "Task added · Click to edit": one toast for the latest Task created in this
 * window. View state only; nothing here is stored.
 */
export function useNewTaskToast(): NewTaskToastState {
  const [shown, setShown] = useState<{ taskId: string; seq: number; leaving: boolean } | null>(null);
  const [flashId, setFlashId] = useState<string | null>(null);
  const [reveal, setReveal] = useState<{ taskId: string } | null>(null);
  const current = useRef(shown);
  current.current = shown;
  const seq = useRef(0);
  const fadeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // Time up: the toast fades, then leaves, unless the pointer or focus catches it.
  const timer = useHoldTimer(() => {
    setShown((now) => now && { ...now, leaving: true });
    fadeTimer.current = setTimeout(() => {
      timer.stop();
      setShown(null);
    }, TOAST_FADE_MS);
  });

  useEffect(() => () => {
    clearTimeout(fadeTimer.current);
    clearTimeout(flashTimer.current);
  }, []);

  /** The toast goes now, and its time with it. */
  function close() {
    clearTimeout(fadeTimer.current);
    timer.stop();
    setShown(null);
  }

  function added(taskId: string) {
    seq.current += 1;
    // A scroll still waiting for an earlier Task is dropped, so it never fires by itself later.
    setReveal(null);
    setShown({ taskId, seq: seq.current, leaving: false });
    // The previous toast is replaced, and with it any pointer or focus on it.
    clearTimeout(fadeTimer.current);
    timer.start(TOAST_MS);
  }

  function hold(reason: HoldReason, isHeld: boolean) {
    if (!current.current) return;
    if (timer.hold(reason, isHeld) && current.current.leaving) {
      // Caught while fading: it stays.
      clearTimeout(fadeTimer.current);
      setShown((now) => now && { ...now, leaving: false });
    }
  }

  function open(): string | null {
    const taskId = current.current?.taskId ?? null;
    close();
    if (!taskId) return null;
    clearTimeout(flashTimer.current);
    setFlashId(taskId);
    flashTimer.current = setTimeout(() => setFlashId(null), FLASH_MS);
    return taskId;
  }

  const clearReveal = useCallback(() => setReveal(null), []);

  function forget(taskId: string) {
    if (current.current?.taskId === taskId) close();
    setFlashId((now) => now === taskId ? null : now);
    setReveal((now) => now?.taskId === taskId ? null : now);
  }

  return {
    taskId: shown?.taskId ?? null, seq: shown?.seq ?? 0, leaving: shown?.leaving ?? false,
    flashId, reveal, added, hold, open, revealTask: (taskId) => setReveal({ taskId }), clearReveal, forget,
  };
}
