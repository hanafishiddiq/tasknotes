import { useEffect, useRef, useState, type DependencyList } from "react";
import { taskApi, type UnfinishedCounts } from "./task-api";

/**
 * The sidebar's numbers, read from the core again whenever `readAgainWhen`
 * changes: the Tasks or Lists on hand, or what came back from Removed. Only
 * the newest answer is shown. A failed read keeps the numbers already shown
 * (none at first) until the next change reads again: a number is a hint, and
 * the place itself always lists its Tasks.
 */
export function useUnfinishedCounts(readAgainWhen: DependencyList): UnfinishedCounts | null {
  const [counts, setCounts] = useState<UnfinishedCounts | null>(null);
  const latest = useRef(0);
  useEffect(() => {
    const read = ++latest.current;
    taskApi.unfinishedCounts()
      .then((next) => { if (read === latest.current) setCounts(next); })
      .catch(() => {});
  }, readAgainWhen);
  return counts;
}
