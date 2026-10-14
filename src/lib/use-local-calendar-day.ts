import { useEffect, useState } from "react";
import { localCalendarDay } from "./deadlines";

/**
 * A date-only deadline remains correct at local midnight without a user click.
 * The 60s cap also notices system time-zone changes while the window stays open.
 * Focus/visibility resume refreshes immediately after suspended background timers.
 */
export function useLocalCalendarDay(): string {
  const [today, setToday] = useState(() => localCalendarDay());

  useEffect(() => {
    let scheduled: ReturnType<typeof setTimeout> | undefined;

    function refresh() {
      if (scheduled !== undefined) clearTimeout(scheduled);
      const now = new Date();
      const day = localCalendarDay(now);
      setToday((current) => current === day ? current : day);
      const nextMidnight = new Date(
        now.getFullYear(), now.getMonth(), now.getDate() + 1, 0, 0, 0, 50,
      );
      const delay = Math.max(50, Math.min(60_000, nextMidnight.getTime() - now.getTime()));
      scheduled = setTimeout(refresh, delay);
    }

    function refreshOnVisible() {
      if (!document.hidden) refresh();
    }

    refresh();
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refreshOnVisible);

    return () => {
      if (scheduled !== undefined) clearTimeout(scheduled);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refreshOnVisible);
    };
  }, []);

  return today;
}
