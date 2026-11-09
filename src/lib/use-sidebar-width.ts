import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

export const SIDEBAR_MIN = 210;
export const SIDEBAR_MAX = 400;
export const SIDEBAR_DEFAULT = 260;
/** The main view keeps at least this much of a narrow window. */
export const MAIN_MIN = 480;
const STORAGE_KEY = "tasknotes.sidebarWidth";

export const clampSidebarWidth = (width: number, max = SIDEBAR_MAX) =>
  Math.min(max, Math.max(SIDEBAR_MIN, Math.round(width)));

/** The widest the sidebar may be in a window this wide: never below the minimum. */
export const sidebarMaxFor = (windowWidth: number) =>
  Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, windowWidth - MAIN_MIN));

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

function store(width: number) {
  try {
    window.localStorage.setItem(STORAGE_KEY, String(width));
  } catch {
    // Keep the in-memory width for this session.
  }
}

export interface SidebarWidth {
  /** The width shown: the remembered preference, narrowed to fit the window. */
  width: number;
  /** The widest the sidebar may be in this window. */
  max: number;
  /** The shell whose `--sidebar-width` drives the layout. */
  shellRef: RefObject<HTMLDivElement | null>;
  /**
   * Shows a width while the edge is dragged, without rendering React or
   * storing it; returns the width kept between the limits.
   */
  preview: (width: number) => number;
  /** Commits a width (kept between the limits) and remembers it on this device. */
  setWidth: (width: number) => void;
}

export function useSidebarWidth(): SidebarWidth {
  const [preferred, setPreferred] = useState(storedWidth);
  const [max, setMax] = useState(() => sidebarMaxFor(window.innerWidth));
  const shellRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const fit = () => setMax(sidebarMaxFor(window.innerWidth));
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, []);

  const preview = useCallback((next: number) => {
    const width = clampSidebarWidth(next, max);
    shellRef.current?.style.setProperty("--sidebar-width", `${width}px`);
    return width;
  }, [max]);

  const setWidth = useCallback((next: number) => {
    const width = clampSidebarWidth(next, max);
    setPreferred(width);
    store(width);
  }, [max]);

  return { width: Math.min(preferred, max), max, shellRef, preview, setWidth };
}
