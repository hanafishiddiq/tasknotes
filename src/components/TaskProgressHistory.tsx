import { useEffect, useState } from "react";
import { History, LoaderCircle } from "lucide-react";
import { taskApi, type ProgressEntry } from "../lib/task-api";
import { STATUS_VIEW } from "../lib/task-visuals";

const formatDate = new Intl.DateTimeFormat("en", {
  dateStyle: "medium",
  timeStyle: "short",
});

function message(error: unknown) {
  if (typeof error === "string") return error;
  return error instanceof Error
    ? error.message
    : "Unable to read local history.";
}

/** The timeline is fetched only while its parent Task is expanded. */
export default function TaskProgressHistory({
  taskId,
  revision,
}: {
  taskId: string;
  revision: number;
}) {
  const [entries, setEntries] = useState<ProgressEntry[]>([]);
  const [cursor, setCursor] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [moreLoading, setMoreLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    setEntries([]);
    setCursor(null);
    taskApi
      .history(taskId)
      .then((page) => {
        if (!active) return;
        setEntries(page.items);
        setCursor(page.nextCursor);
      })
      .catch((reason) => {
        if (active) setError(message(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [taskId, revision, retry]);

  async function loadMore() {
    if (!cursor || moreLoading) return;
    setMoreLoading(true);
    setError(null);
    try {
      const page = await taskApi.history(taskId, cursor);
      setEntries((previous) => {
        const existingIds = new Set(previous.map((entry) => entry.id));
        return [
          ...previous,
          ...page.items.filter((entry) => !existingIds.has(entry.id)),
        ];
      });
      setCursor(page.nextCursor);
    } catch (reason) {
      setError(message(reason));
    } finally {
      setMoreLoading(false);
    }
  }

  return (
    <section className="progress-history" aria-label="Progress history">
      <div className="progress-history-heading">
        <History size={15} aria-hidden="true" />
        <h3>Recent progress</h3>
      </div>
      {loading ? (
        <p role="status" className="history-hint">
          <LoaderCircle size={14} className="spin" /> Loading progress
          history...
        </p>
      ) : error && entries.length === 0 ? (
        <div className="history-hint" role="alert">
          Couldn't load progress history: {error}
          <button type="button" onClick={() => setRetry((value) => value + 1)}>
            Retry history
          </button>
        </div>
      ) : entries.length === 0 ? (
        <p className="history-hint">
          No progress recorded yet. Your next update will appear here.
        </p>
      ) : (
        <ol className="history-timeline">
          {entries.map((entry) => (
            <li key={entry.id} className="history-entry">
              <div className="history-entry-summary">
                <span className="history-entry-date">
                  {formatDate.format(new Date(entry.createdAt))}
                </span>
                <span>{STATUS_VIEW[entry.status].label}</span>
                <span>
                  {entry.previousProgress === entry.progress
                    ? `${entry.progress}%`
                    : `${entry.previousProgress}% → ${entry.progress}%`}
                </span>
              </div>
              {entry.note && <p className="history-entry-note">{entry.note}</p>}
              {entry.nextStep && (
                <p className="history-entry-next">Next: {entry.nextStep}</p>
              )}
            </li>
          ))}
        </ol>
      )}
      {error && entries.length > 0 && (
        <p role="alert" className="history-error">
          Couldn't load older entries: {error}
        </p>
      )}
      {!loading && cursor !== null && (
        <button
          type="button"
          className="history-load-more"
          disabled={moreLoading}
          onClick={() => void loadMore()}
        >
          {moreLoading ? "Loading..." : "Load older progress"}
        </button>
      )}
    </section>
  );
}
