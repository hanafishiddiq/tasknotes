import { useCallback, useEffect, useRef, useState } from "react";

/** How long the toast stays before it fades, not counting time held under the pointer or focus. */
export const TOAST_MS = 6000;
/** The fade out, matching DESIGN.md's 120ms fades. */
export const TOAST_FADE_MS = 120;
/** How long an opened Task keeps its yellow flash. */
export const FLASH_MS = 1500;
/** Time left once the pointer or focus lets go of a toast whose time was already up. */
const RELEASE_GRACE_MS = 1000;

type HoldReason = "hover" | "focus";

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
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // Time left on the toast, and when it last started counting down.
  const remaining = useRef(TOAST_MS);
  const startedAt = useRef(0);
  const held = useRef(new Set<HoldReason>());

  useEffect(() => () => {
    clearTimeout(timer.current);
    clearTimeout(flashTimer.current);
  }, []);

  function countDown(ms: number) {
    clearTimeout(timer.current);
    remaining.current = ms;
    startedAt.current = Date.now();
    timer.current = setTimeout(() => {
      remaining.current = 0;
      setShown((now) => now && { ...now, leaving: true });
      timer.current = setTimeout(() => setShown(null), TOAST_FADE_MS);
    }, ms);
  }

  function added(taskId: string) {
    seq.current += 1;
    // The previous toast is replaced, and with it any pointer or focus on it.
    held.current.clear();
    // A scroll still waiting for an earlier Task is dropped, so it never fires by itself later.
    setReveal(null);
    setShown({ taskId, seq: seq.current, leaving: false });
    countDown(TOAST_MS);
  }

  function hold(reason: HoldReason, isHeld: boolean) {
    if (!current.current) return;
    const wasHeld = held.current.size > 0;
    if (isHeld) held.current.add(reason);
    else held.current.delete(reason);
    const nowHeld = held.current.size > 0;
    if (nowHeld && !wasHeld) {
      clearTimeout(timer.current);
      remaining.current = Math.max(0, remaining.current - (Date.now() - startedAt.current));
      // Caught while fading: it stays.
      if (current.current.leaving) setShown((now) => now && { ...now, leaving: false });
    } else if (!nowHeld && wasHeld) {
      countDown(remaining.current > 0 ? remaining.current : RELEASE_GRACE_MS);
    }
  }

  function open(): string | null {
    const taskId = current.current?.taskId ?? null;
    clearTimeout(timer.current);
    held.current.clear();
    setShown(null);
    if (!taskId) return null;
    clearTimeout(flashTimer.current);
    setFlashId(taskId);
    flashTimer.current = setTimeout(() => setFlashId(null), FLASH_MS);
    return taskId;
  }

  const clearReveal = useCallback(() => setReveal(null), []);

  return {
    taskId: shown?.taskId ?? null, seq: shown?.seq ?? 0, leaving: shown?.leaving ?? false,
    flashId, reveal, added, hold, open, revealTask: (taskId) => setReveal({ taskId }), clearReveal,
  };
}
