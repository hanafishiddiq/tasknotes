import { useCallback, useEffect, useState } from "react";

export const SIDEBAR_MIN = 210;
export const SIDEBAR_MAX = 400;
export const SIDEBAR_DEFAULT = 260;
const STORAGE_KEY = "tasknotes.sidebarWidth";

export const clampSidebarWidth = (width: number) =>
  Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(width)));

// A per-device display preference like the theme, not Tasknotes data: when
// storage is unavailable or unreadable, the default width applies.
function storedWidth(): number {
  try {
    const value = Number.parseFloat(window.localStorage.getItem(STORAGE_KEY) ?? "");
    return Number.isFinite(value) ? clampSidebarWidth(value) : SIDEBAR_DEFAULT;
  } catch {
    return SIDEBAR_DEFAULT;
  }
}

export interface SidebarWidth {
  width: number;
  /** Any width; it is kept between the limits. */
  setWidth: (width: number) => void;
}

export function useSidebarWidth(): SidebarWidth {
  const [width, setRaw] = useState(storedWidth);
  const setWidth = useCallback((next: number) => setRaw(clampSidebarWidth(next)), []);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, String(width));
    } catch {
      // Keep the in-memory width for this session.
    }
  }, [width]);

  return { width, setWidth };
}
