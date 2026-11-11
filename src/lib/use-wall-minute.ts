import { useEffect, useState } from "react";

/**
 * The wall clock, read again as each minute begins and when the window comes
 * back: enough to draw "now" on the Pomodoro day rail. It times nothing; the
 * timer's seconds come from the shell.
 */
export function useWallMinute(): Date {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    let scheduled: ReturnType<typeof setTimeout> | undefined;

    function refresh() {
      if (scheduled !== undefined) clearTimeout(scheduled);
      const read = new Date();
      setNow(read);
      scheduled = setTimeout(refresh, 60_000 - (read.getTime() % 60_000) + 50);
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

  return now;
}
