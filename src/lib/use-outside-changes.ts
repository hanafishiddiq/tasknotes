import { useEffect, useRef } from "react";
import { invoke } from "./invoke";

/** How often the window asks whether the CLI or an MCP client changed something. */
export const OUTSIDE_CHANGE_POLL_MS = 3_000;

interface OutsideChanges {
  latest: number;
  changed: boolean;
}

/**
 * Calls `show` when another process changed the data. The window asks when it
 * regains focus and every few seconds; the question costs one small read.
 * `show` resolves false when it could not show them yet: they stay unseen and
 * the next check offers them again. A failed check says nothing, as with
 * pending approvals: the views report failures themselves.
 */
export function useOutsideChanges(show: () => Promise<boolean>) {
  const latestShow = useRef(show);
  latestShow.current = show;
  useEffect(() => {
    let seen: number | null = null;
    let checking = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try {
        const { latest, changed } = await invoke<OutsideChanges>("outside_changes", { seen });
        if (!changed || await latestShow.current()) seen = latest;
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
