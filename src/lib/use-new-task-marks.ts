import { useCallback, useEffect, useMemo, useRef, useState } from "react";

/**
 * How long a Task just added stays marked once the person has seen it: on screen
 * while the window is in front (batch 3 Q11/Q14; 20 seconds for every new Task,
 * owner 2026-11-17).
 */
export const NEW_TASK_MARK_MS = 20_000;
/** How long the yellow takes to ease away once those seconds are up. */
export const MARK_FADE_MS = 800;

/** Who added the Task: the person in this window, or the CLI or an MCP client. */
type AddedBy = "here" | "outside";
/** A mark waits to be seen, counts down, then fades. */
type MarkState = "waiting" | "counting" | "fading";

export interface NewTaskMarks {
  /** Every Task marked now (not counting one that is fading). */
  marked: ReadonlySet<string>;
  /** Marked Tasks whose 20 seconds have not started: their rows report when seen. */
  waitingToBeSeen: ReadonlySet<string>;
  /** The person created this Task in the window. */
  addedHere(taskId: string): void;
  /** The CLI or an MCP client created these Tasks while the window was open. */
  addedOutside(taskIds: readonly string[]): void;
  /** A Task was opened: the person's own marks all go at once, and this Task's mark. */
  opened(taskId: string): void;
  /** This Task's row is on screen while the window is in front: its 20 seconds start, once. */
  seen(taskId: string): void;
  /** This Task was moved to Removed. */
  forget(taskId: string): void;
  /** What a Task's row shows: the mark or its fade, and while the mark waits, how it reports being seen. */
  rowProps(taskId: string): { justAdded: boolean; fading: boolean; onSeen?: () => void };
}

/**
 * The pale-yellow mark on Tasks just added (batch 3, Ticket 02; batch 4). Every
 * new Task, added here or by the CLI or an MCP client, keeps it until 20 seconds
 * after it is seen, then fades. Opening a Task ends marks sooner: any Task ends
 * the person's own (Q6), and a Task ends its own mark. View state only: nothing
 * here is stored, so quitting clears every mark and a Task added while the window
 * was closed gets none (Q15). Every function is stable across renders.
 */
export function useNewTaskMarks(): NewTaskMarks {
  const [marks, setMarks] = useState<ReadonlyMap<string, { by: AddedBy; state: MarkState }>>(() => new Map());
  const marksNow = useRef(marks);
  marksNow.current = marks;
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  useEffect(() => {
    const running = timers.current;
    return () => {
      for (const timer of running.values()) clearTimeout(timer);
      running.clear();
    };
  }, []);

  const drop = useCallback((taskIds: readonly string[]) => {
    for (const taskId of taskIds) {
      clearTimeout(timers.current.get(taskId));
      timers.current.delete(taskId);
    }
    setMarks((now) => {
      if (!taskIds.some((taskId) => now.has(taskId))) return now;
      const next = new Map(now);
      for (const taskId of taskIds) next.delete(taskId);
      return next;
    });
  }, []);

  const add = useCallback((taskIds: readonly string[], by: AddedBy) => {
    setMarks((now) => {
      const fresh = taskIds.filter((taskId) => !now.has(taskId));
      if (fresh.length === 0) return now;
      const next = new Map(now);
      for (const taskId of fresh) next.set(taskId, { by, state: "waiting" });
      return next;
    });
  }, []);

  const addedHere = useCallback((taskId: string) => add([taskId], "here"), [add]);
  const addedOutside = useCallback((taskIds: readonly string[]) => add(taskIds, "outside"), [add]);

  const opened = useCallback((taskId: string) => {
    const own = [...marksNow.current].filter(([, mark]) => mark.by === "here").map(([id]) => id);
    drop([...own, taskId]);
  }, [drop]);

  const setState = useCallback((taskId: string, state: MarkState) => {
    setMarks((now) => {
      const mark = now.get(taskId);
      return mark ? new Map(now).set(taskId, { ...mark, state }) : now;
    });
  }, []);

  const seen = useCallback((taskId: string) => {
    // The state says what renders; the timer guards a second report in the same tick, before it renders.
    if (marksNow.current.get(taskId)?.state !== "waiting" || timers.current.has(taskId)) return;
    timers.current.set(taskId, setTimeout(() => {
      setState(taskId, "fading");
      timers.current.set(taskId, setTimeout(() => drop([taskId]), MARK_FADE_MS));
    }, NEW_TASK_MARK_MS));
    setState(taskId, "counting");
  }, [drop, setState]);

  const forget = useCallback((taskId: string) => drop([taskId]), [drop]);

  const marked = useMemo(
    () => new Set([...marks].filter(([, mark]) => mark.state !== "fading").map(([taskId]) => taskId)),
    [marks],
  );
  const waitingToBeSeen = useMemo(
    () => new Set([...marks].filter(([, mark]) => mark.state === "waiting").map(([taskId]) => taskId)),
    [marks],
  );

  const rowProps = useCallback((taskId: string) => ({
    justAdded: marked.has(taskId),
    fading: marks.get(taskId)?.state === "fading",
    onSeen: waitingToBeSeen.has(taskId) ? () => seen(taskId) : undefined,
  }), [marks, marked, waitingToBeSeen, seen]);

  return { marked, waitingToBeSeen, addedHere, addedOutside, opened, seen, forget, rowProps };
}
