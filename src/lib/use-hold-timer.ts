import { useEffect, useMemo, useRef } from "react";

export type HoldReason = "hover" | "focus";

/** Time left once the pointer or focus lets go of something whose time was nearly or already up. */
export const RELEASE_GRACE_MS = 1000;

export interface HoldTimer {
  /** Counts `ms` down afresh, letting go of any earlier hold. */
  start(ms: number): void;
  /** Stops counting without running out; holds are let go and ignored until the next start. */
  stop(): void;
  /**
   * The pointer or keyboard focus is on what the timer keeps up (true) or left it
   * (false). While either holds it, time stands still; once both let go it counts
   * on with the time that was left, never less than RELEASE_GRACE_MS. Holding
   * after the time ran out (a toast fading) counts too. Returns whether it is held now.
   */
  hold(reason: HoldReason, held: boolean): boolean;
}

/**
 * A countdown the pointer or focus can hold, shared by the short-lived bars and
 * toasts (the new Task toast, "Task completed · Undo"). `onExpire` runs when the
 * time runs out; the latest one given is used. Stable across renders.
 */
export function useHoldTimer(onExpire: () => void): HoldTimer {
  const expire = useRef(onExpire);
  useEffect(() => {
    expire.current = onExpire;
  });
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const remaining = useRef(0);
  const startedAt = useRef(0);
  const held = useRef(new Set<HoldReason>());
  const running = useRef(false);

  useEffect(() => () => clearTimeout(timer.current), []);

  return useMemo(() => {
    function countDown(ms: number) {
      clearTimeout(timer.current);
      remaining.current = ms;
      startedAt.current = Date.now();
      timer.current = setTimeout(() => {
        remaining.current = 0;
        expire.current();
      }, ms);
    }
    return {
      start(ms: number) {
        held.current.clear();
        running.current = true;
        countDown(ms);
      },
      stop() {
        clearTimeout(timer.current);
        held.current.clear();
        running.current = false;
      },
      hold(reason: HoldReason, on: boolean) {
        if (!running.current) return false;
        const wasHeld = held.current.size > 0;
        if (on) held.current.add(reason);
        else held.current.delete(reason);
        const isHeld = held.current.size > 0;
        if (isHeld && !wasHeld) {
          clearTimeout(timer.current);
          remaining.current = Math.max(0, remaining.current - (Date.now() - startedAt.current));
        } else if (!isHeld && wasHeld) {
          countDown(Math.max(remaining.current, RELEASE_GRACE_MS));
        }
        return isHeld;
      },
    };
  }, []);
}
