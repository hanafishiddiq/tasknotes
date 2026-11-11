import { useCallback, useEffect, useMemo, useRef, useState } from "react";

/**
 * How long a Task another process added stays marked once the person has seen
 * it: on screen while the window is in front (owner decision 2026-11-11, Q11/Q14).
 */
export const OUTSIDE_MARK_MS = 10_000;

export interface NewTaskMarks {
  /** Every Task marked now. */
  marked: ReadonlySet<string>;
  /** Marked Tasks another process added whose 10 seconds have not started: their rows report when seen. */
  waitingToBeSeen: ReadonlySet<string>;
  /** The person created this Task in the window. */
  addedHere(taskId: string): void;
  /** The CLI or an MCP client created these Tasks while the window was open. */
  addedOutside(taskIds: readonly string[]): void;
  /** A Task was opened: the person's own marks all go, and this Task's outside mark. */
  opened(taskId: string): void;
  /** This Task's row is on screen while the window is in front: its 10 seconds start, once. */
  seen(taskId: string): void;
  /** This Task was moved to Removed. */
  forget(taskId: string): void;
  /** What a Task's row shows: the mark, and while its 10 seconds wait, how it reports being seen. */
  rowProps(taskId: string): { justAdded: boolean; onSeen?: () => void };
}

/**
 * The pale-yellow mark on Tasks just added (batch 3, Ticket 02). A Task the
 * person adds in the window keeps it until any Task is opened (Q6); one the CLI
 * or an MCP client adds keeps it until 10 seconds after it is seen, or until it
 * is opened (Q11, Q14). View state only: nothing here is stored, so quitting
 * clears every mark and a Task added while the window was closed gets none (Q15).
 * Every function is stable across renders.
 */
export function useNewTaskMarks(): NewTaskMarks {
  const [own, setOwn] = useState<ReadonlySet<string>>(() => new Set());
  // An outside Task waits to be seen, then counts down.
  const [outside, setOutside] = useState<ReadonlyMap<string, "waiting" | "counting">>(() => new Map());
  const outsideNow = useRef(outside);
  outsideNow.current = outside;
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  useEffect(() => {
    const running = timers.current;
    return () => {
      for (const timer of running.values()) clearTimeout(timer);
      running.clear();
    };
  }, []);

  const dropOutside = useCallback((taskId: string) => {
    clearTimeout(timers.current.get(taskId));
    timers.current.delete(taskId);
    setOutside((now) => {
      if (!now.has(taskId)) return now;
      const next = new Map(now);
      next.delete(taskId);
      return next;
    });
  }, []);

  const addedHere = useCallback((taskId: string) => {
    setOwn((now) => new Set(now).add(taskId));
  }, []);

  const addedOutside = useCallback((taskIds: readonly string[]) => {
    setOutside((now) => {
      const fresh = taskIds.filter((taskId) => !now.has(taskId));
      if (fresh.length === 0) return now;
      const next = new Map(now);
      for (const taskId of fresh) next.set(taskId, "waiting");
      return next;
    });
  }, []);

  const opened = useCallback((taskId: string) => {
    setOwn((now) => now.size === 0 ? now : new Set());
    dropOutside(taskId);
  }, [dropOutside]);

  const seen = useCallback((taskId: string) => {
    // The state says what renders; the timer guards a second report in the same tick, before it renders.
    if (outsideNow.current.get(taskId) !== "waiting" || timers.current.has(taskId)) return;
    timers.current.set(taskId, setTimeout(() => dropOutside(taskId), OUTSIDE_MARK_MS));
    setOutside((now) => now.get(taskId) === "waiting" ? new Map(now).set(taskId, "counting") : now);
  }, [dropOutside]);

  const forget = useCallback((taskId: string) => {
    setOwn((now) => {
      if (!now.has(taskId)) return now;
      const next = new Set(now);
      next.delete(taskId);
      return next;
    });
    dropOutside(taskId);
  }, [dropOutside]);

  const marked = useMemo(() => new Set([...own, ...outside.keys()]), [own, outside]);
  const waitingToBeSeen = useMemo(
    () => new Set([...outside].filter(([, state]) => state === "waiting").map(([taskId]) => taskId)),
    [outside],
  );

  const rowProps = useCallback((taskId: string) => ({
    justAdded: marked.has(taskId),
    onSeen: waitingToBeSeen.has(taskId) ? () => seen(taskId) : undefined,
  }), [marked, waitingToBeSeen, seen]);

  return { marked, waitingToBeSeen, addedHere, addedOutside, opened, seen, forget, rowProps };
}
