import { useEffect, useMemo, useRef, useState } from "react";
import type { Task } from "./task-api";
import { useHoldTimer, type HoldReason } from "./use-hold-timer";

/** How long "Task completed · Undo" stays, not counting time held under the pointer or focus. */
export const COMPLETION_UNDO_MS = 6000;

export interface CompletionUndo {
  /** The Task the Undo bar would take back, or null when the bar does not show. */
  taskId: string | null;
  /** The revision the completion made, which the Undo names. */
  revision: number | null;
  /** A Task was completed from its check: offer to take it back, replacing any earlier offer. */
  offer(taskId: string, revision: number, undo: () => Promise<void>): void;
  /** Takes the completion back once; the bar goes at once. */
  run(): void;
  /** The bar goes, as on leaving the Task view. */
  hide(): void;
  /** The pointer or keyboard focus is on the bar (true) or left it (false). Stable across renders, as every action is. */
  hold(reason: HoldReason, held: boolean): void;
}

/**
 * The offer to take back the latest completion made from a Task's check, for a
 * few seconds (Ticket 16). Unlike a move to Removed it is not kept for ⌘Z: once
 * the bar has gone, the check reopens the Task. The Undo names the revision its
 * completion made, so once `shown` holds that Task at another revision (changed
 * here, elsewhere, or reopened) the offer is gone rather than refused. View state only.
 */
export function useCompletionUndo(shown: readonly Task[]): CompletionUndo {
  type Offer = { taskId: string; revision: number; undo: () => Promise<void> };
  const [offered, setOffered] = useState<Offer | null>(null);
  const current = useRef<Offer | null>(null);
  const timer = useHoldTimer(() => {
    timer.stop();
    current.current = null;
    setOffered(null);
  });

  // Only refs, the setter and the stable timer inside, so each action stays the same across renders.
  const actions = useMemo(() => {
    function show(offer: Offer | null) {
      current.current = offer;
      setOffered(offer);
    }
    function stop() {
      timer.stop();
      show(null);
    }
    return {
      offer(taskId: string, revision: number, undo: () => Promise<void>) {
        show({ taskId, revision, undo });
        timer.start(COMPLETION_UNDO_MS);
      },
      run() {
        const now = current.current;
        if (!now) return;
        stop();
        void now.undo();
      },
      hide() {
        if (current.current) stop();
      },
      hold(reason: HoldReason, on: boolean) {
        if (current.current) timer.hold(reason, on);
      },
    };
  }, [timer]);

  const changed = offered !== null &&
    shown.some((task) => task.id === offered.taskId && task.revision !== offered.revision);
  useEffect(() => {
    if (changed) actions.hide();
  }, [changed, actions]);

  return { taskId: offered?.taskId ?? null, revision: offered?.revision ?? null, ...actions };
}
