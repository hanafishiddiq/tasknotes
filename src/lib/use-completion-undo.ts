import { useEffect, useMemo, useRef, useState } from "react";

/** How long "Task completed · Undo" stays, not counting time held under the pointer or focus. */
export const COMPLETION_UNDO_MS = 6000;
/** Time left once the pointer or focus lets go of an offer whose time was nearly up. */
const RELEASE_GRACE_MS = 1000;

type HoldReason = "hover" | "focus";

export interface CompletionUndo {
  /** The Task the Undo bar would take back, or null when the bar does not show. */
  taskId: string | null;
  /** The revision the completion made, which the Undo names. */
  revision: number | null;
  /** A Task was completed from its check: offer to take it back, replacing any earlier offer. */
  offer(taskId: string, revision: number, undo: () => Promise<void>): void;
  /** Takes the completion back once; the bar goes at once. */
  run(): void;
  /**
   * The bar goes: on leaving the Task view, or, given its Task, because that
   * Task changed again and the revision the offer names is gone.
   */
  hide(taskId?: string): void;
  /** The pointer or keyboard focus is on the bar (true) or left it (false). Stable across renders, as every action is. */
  hold(reason: HoldReason, held: boolean): void;
}

/**
 * The offer to take back the latest completion made from a Task's check, for a
 * few seconds (Ticket 16). Unlike a move to Removed it is not kept for ⌘Z: once
 * the bar has gone, the check reopens the Task. View state only.
 */
export function useCompletionUndo(): CompletionUndo {
  type Offer = { taskId: string; revision: number; undo: () => Promise<void> };
  const [offered, setOffered] = useState<Offer | null>(null);
  const current = useRef<Offer | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const remaining = useRef(COMPLETION_UNDO_MS);
  const startedAt = useRef(0);
  const held = useRef(new Set<HoldReason>());

  useEffect(() => () => clearTimeout(timer.current), []);

  // Only refs and the setter inside, so each action stays the same across renders.
  const actions = useMemo(() => {
    function show(offer: Offer | null) {
      current.current = offer;
      setOffered(offer);
    }
    function countDown(ms: number) {
      clearTimeout(timer.current);
      remaining.current = ms;
      startedAt.current = Date.now();
      timer.current = setTimeout(() => show(null), ms);
    }
    function stop() {
      clearTimeout(timer.current);
      held.current.clear();
      show(null);
    }
    return {
      offer(taskId: string, revision: number, undo: () => Promise<void>) {
        held.current.clear();
        show({ taskId, revision, undo });
        countDown(COMPLETION_UNDO_MS);
      },
      run() {
        const now = current.current;
        if (!now) return;
        stop();
        void now.undo();
      },
      hide(taskId?: string) {
        if (current.current && (taskId === undefined || taskId === current.current.taskId)) stop();
      },
      hold(reason: HoldReason, on: boolean) {
        if (!current.current) return;
        const wasHeld = held.current.size > 0;
        if (on) held.current.add(reason);
        else held.current.delete(reason);
        if (!wasHeld && held.current.size > 0) {
          clearTimeout(timer.current);
          remaining.current = Math.max(0, remaining.current - (Date.now() - startedAt.current));
        } else if (wasHeld && held.current.size === 0) {
          countDown(Math.max(remaining.current, RELEASE_GRACE_MS));
        }
      },
    };
  }, []);

  return { taskId: offered?.taskId ?? null, revision: offered?.revision ?? null, ...actions };
}
