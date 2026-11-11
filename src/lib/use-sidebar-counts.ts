import { useEffect, useState } from "react";

/** Where the sidebar shows each folder's number of unfinished Tasks (batch 3, Q48). */
export type SidebarCounts = "every" | "open" | "none";

/** The choices, in the order Settings offers them; every folder is the default (Q46). */
export const SIDEBAR_COUNTS: readonly SidebarCounts[] = ["every", "open", "none"];

export const SIDEBAR_COUNT_LABELS: Record<SidebarCounts, string> = {
  every: "Every folder", open: "Open folder only", none: "None",
};

const STORAGE_KEY = "tasknotes.sidebarCounts";

// A per-window display preference, not domain data: without storage the
// sidebar simply shows every count.
function stored(): SidebarCounts {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    return value === "open" || value === "none" ? value : "every";
  } catch {
    return "every";
  }
}

export function useSidebarCounts(): [SidebarCounts, (next: SidebarCounts) => void] {
  const [choice, setChoice] = useState<SidebarCounts>(stored);
  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, choice);
    } catch {
      // Keep the in-memory choice for this session.
    }
  }, [choice]);
  return [choice, setChoice];
}
