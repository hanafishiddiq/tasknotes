import { useCallback, useEffect, useState } from "react";
import { viewKey, type TaskView } from "./task-api";

const KEY = "tasknotes.completedShown";

function read(): Record<string, true> {
  try {
    const parsed: unknown = JSON.parse(window.localStorage.getItem(KEY) ?? "{}");
    if (!parsed || typeof parsed !== "object") return {};
    return Object.fromEntries(Object.entries(parsed).filter(([, shown]) => shown === true)) as Record<string, true>;
  } catch {
    return {};
  }
}

/**
 * Which views show their Completed Tasks (batch 4, owner 2026-11-17): hidden
 * unless the person asked, remembered per view in this window's storage. A
 * display choice, not domain data: unreadable storage hides them everywhere.
 */
export function useCompletedShown(): [(view: TaskView) => boolean, (view: TaskView, shown: boolean) => void] {
  const [shownViews, setShownViews] = useState<Record<string, true>>(read);
  useEffect(() => {
    try {
      window.localStorage.setItem(KEY, JSON.stringify(shownViews));
    } catch {
      // Keep the choice for this session.
    }
  }, [shownViews]);
  const isShown = useCallback((view: TaskView) => shownViews[viewKey(view)] === true, [shownViews]);
  const setShown = useCallback((view: TaskView, shown: boolean) => {
    setShownViews((now) => {
      const { [viewKey(view)]: _was, ...rest } = now;
      return shown ? { ...rest, [viewKey(view)]: true } : rest;
    });
  }, []);
  return [isShown, setShown];
}
