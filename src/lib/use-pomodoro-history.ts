import { useEffect, useRef, useState } from "react";
import { errorMessage } from "./error-message";
import { pomodoroApi, type PomodoroDay, type PomodoroTotals } from "./pomodoro-api";

export interface PomodoroHistory {
  /** Newest day first, as the core counted them. */
  days: PomodoroDay[];
  /** Every session ever stored, counted as the days are; null until read. */
  totals: PomodoroTotals | null;
  loading: boolean;
  loadingMore: boolean;
  /** Older days wait to be loaded. */
  hasMore: boolean;
  error: string | null;
  loadMore: () => Promise<void>;
}

/**
 * The Pomodoro page's history and all-time totals, read from the core while
 * the page shows and again whenever `refreshKey` changes (a session was
 * stored, or something changed outside), keeping the older days the person
 * loaded. It keeps no count of its own.
 */
export function usePomodoroHistory(hidden: boolean, refreshKey: number): PomodoroHistory {
  const [days, setDays] = useState<PomodoroDay[]>([]);
  const [totals, setTotals] = useState<PomodoroTotals | null>(null);
  const [cursor, setCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Days on screen, so reading again keeps the older ones the person loaded.
  const shown = useRef(0);
  shown.current = days.length;
  // Bumped by every read from the top. An older page asked for before it
  // continues a list that is gone, so its answer is dropped: appending it
  // could skip the day that the new first page no longer reaches.
  const generation = useRef(0);

  useEffect(() => {
    if (hidden) return;
    const read = ++generation.current;
    const current = () => read === generation.current;
    const keep = shown.current;
    (async () => {
      const [first, all] = await Promise.all([pomodoroApi.history(), pomodoroApi.totals()]);
      let page = first;
      let items = page.items;
      while (items.length < keep && page.nextCursor !== null) {
        page = await pomodoroApi.history(page.nextCursor);
        items = [...items, ...page.items];
      }
      if (!current()) return;
      setDays(items);
      setTotals(all);
      setCursor(page.nextCursor);
      setError(null);
    })()
      .catch((reason) => { if (current()) setError("Couldn't load the Pomodoro history: " + errorMessage(reason)); })
      .finally(() => { if (current()) setLoading(false); });
    return () => { generation.current++; };
  }, [hidden, refreshKey]);

  async function loadMore() {
    if (!cursor || loadingMore) return;
    const read = generation.current;
    setLoadingMore(true);
    try {
      const page = await pomodoroApi.history(cursor);
      // The core's cursor starts after the last day shown, and a read from the
      // top since then drops this answer, so no day can come twice.
      if (read !== generation.current) return;
      setDays((current) => [...current, ...page.items]);
      setCursor(page.nextCursor);
    } catch (reason) {
      if (read === generation.current) setError("Couldn't load more days: " + errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  return { days, totals, loading, loadingMore, hasMore: cursor !== null, error, loadMore };
}
