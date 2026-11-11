import { useEffect, useRef } from "react";
import { invoke } from "./invoke";

/** How often the window asks whether the CLI or an MCP client changed something. */
export const OUTSIDE_CHANGE_POLL_MS = 3_000;

interface OutsideChanges {
  latest: number;
  changed: boolean;
  /** Tasks another process created since `seen`; never any on the first answer. */
  createdTasks?: string[];
}

/**
 * Calls `show` when another process changed the data. The window asks when it
 * regains focus and every few seconds; the question costs one small read.
 * `show` resolves false when it could not show them yet: they stay unseen and
 * the next check offers them again. A failed check says nothing, as with
 * pending approvals: the views report failures themselves.
 *
 * The first answer only says where the log ends, and the window's first reads
 * may have run before it: a change made in between would never be offered. So
 * the first `show` runs without a change, once, and its reads start after that answer.
 */
export function useOutsideChanges(
  show: () => Promise<boolean>,
  /** Tasks another process created, once they are shown (batch 3, Ticket 02: the new Task mark). */
  onCreated?: (taskIds: string[]) => void,
) {
  const latestShow = useRef(show);
  latestShow.current = show;
  const latestCreated = useRef(onCreated);
  latestCreated.current = onCreated;
  useEffect(() => {
    let seen: number | null = null;
    let checking = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try {
        const { latest, changed, createdTasks = [] } = await invoke<OutsideChanges>("outside_changes", { seen });
        if ((seen !== null && !changed) || await latestShow.current()) {
          // Only once shown: a check that could not show them offers them again.
          if (createdTasks.length > 0) latestCreated.current?.(createdTasks);
          seen = latest;
        }
      } catch {
        // Asked again on the next tick.
      } finally {
        checking = false;
      }
    };
    void check();
    const timer = window.setInterval(() => void check(), OUTSIDE_CHANGE_POLL_MS);
    window.addEventListener("focus", check);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", check);
    };
  }, []);
}
