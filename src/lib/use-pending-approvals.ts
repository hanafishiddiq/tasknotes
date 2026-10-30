import { useCallback, useEffect, useState } from "react";
import { approvalApi } from "./approval-api";

/** How often the window asks whether an AI client requested something. */
export const APPROVAL_POLL_MS = 30_000;

/**
 * How many Bulk Change requests wait for the person. Requests are written by
 * other processes (the CLI, an MCP client), so the window asks on start, when
 * it regains focus and every 30 s. A failed ask shows nothing rather than an
 * error: the count is a hint, and the view itself reports failures.
 */
export function usePendingApprovals() {
  const [count, setCount] = useState(0);
  const refresh = useCallback(async () => {
    try {
      const value = await approvalApi.count();
      setCount(typeof value === "number" ? value : 0);
    } catch {
      setCount(0);
    }
  }, []);
  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), APPROVAL_POLL_MS);
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [refresh]);
  return { count, refresh };
}
