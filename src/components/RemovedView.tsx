import { useEffect, useState } from "react";
import { Folder, ListChecks, LoaderCircle, NotebookText, RotateCcw, SquareCheck, Trash2 } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { removedApi, type RemovedItem, type RemovedKind } from "../lib/removed-api";

const KIND: Record<RemovedKind, { label: string; container: string; icon: typeof Folder }> = {
  task: { label: "Task", container: "Restore its List first", icon: SquareCheck },
  note: { label: "Note", container: "Restore its folder first", icon: NotebookText },
  list: { label: "List", container: "", icon: ListChecks },
  note_folder: { label: "Folder", container: "Restore the folder above it first", icon: Folder },
};

const REMOVED_ON = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

interface RemovedViewProps {
  hidden: boolean;
  /** Asked again whenever this changes, e.g. after something new was removed. */
  refreshKey: number;
  /** Called after a restore so other views can show the item again. */
  onRestored: (item: RemovedItem) => void;
}

/**
 * Everything in Removed, like Apple Notes' Recently Deleted except that
 * nothing is ever deleted from it: an item leaves only by being restored.
 */
export default function RemovedView({ hidden, refreshKey, onRestored }: RemovedViewProps) {
  const [items, setItems] = useState<RemovedItem[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function reload() {
    const page = await removedApi.list();
    setItems(page.items);
    setCursor(page.nextCursor);
  }

  useEffect(() => {
    if (hidden) return;
    let active = true;
    setLoading(true);
    removedApi.list()
      .then((page) => {
        if (!active) return;
        setItems(page.items);
        setCursor(page.nextCursor);
        setError(null);
      })
      .catch((reason) => { if (active) setError("Couldn't load Removed: " + errorMessage(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [hidden, refreshKey]);

  async function loadMore() {
    if (!cursor || loadingMore) return;
    setLoadingMore(true);
    try {
      const page = await removedApi.list(cursor);
      setItems((current) => {
        const seen = new Set(current.map((item) => item.id));
        return [...current, ...page.items.filter((item) => !seen.has(item.id))];
      });
      setCursor(page.nextCursor);
    } catch (reason) {
      setError("Couldn't load more: " + errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  async function restore(item: RemovedItem) {
    if (restoring) return;
    setRestoring(item.id);
    setError(null);
    try {
      await removedApi.restore(item);
      setItems((current) => current.filter((other) => other.id !== item.id));
      onRestored(item);
      // A restored container can make items below it restorable.
      if (item.kind === "list" || item.kind === "note_folder") {
        await reload().catch((reason) => setError("Restored, but Removed could not refresh: " + errorMessage(reason)));
      }
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setRestoring(null);
    }
  }

  return (
    <main className="main-view" hidden={hidden}>
      <div className="main-scroll">
        <section className="content-wrap" aria-label="Removed items">
          <div className="page-title"><h1>Removed</h1></div>
          <p className="removed-hint">
            Removed items stay here until you restore them. Nothing is deleted automatically.
          </p>
          {error && <div role="alert" className="error-banner"><span>{error}</span></div>}
          {loading ? (
            <div className="loading-state" role="status"><LoaderCircle className="spin" size={16} /> Loading Removed...</div>
          ) : items.length === 0 ? (
            <div className="empty-state">
              <Trash2 size={26} strokeWidth={1.5} aria-hidden="true" />
              <h3>Nothing in Removed</h3>
            </div>
          ) : (
            <ul className="removed-list">
              {items.map((item) => {
                const kind = KIND[item.kind];
                const Icon = kind.icon;
                return (
                  <li key={item.id} className="removed-item">
                    <Icon size={16} aria-hidden="true" />
                    <span className="removed-item-text">
                      <span className="removed-item-title">{item.title}</span>
                      <span className="removed-item-meta">
                        <span>{kind.label}</span> · removed {REMOVED_ON.format(item.removedAt)}
                      </span>
                    </span>
                    {item.inRemovedContainer ? (
                      <span className="removed-item-blocked">{kind.container}</span>
                    ) : (
                      <button type="button" className="button button-small" disabled={restoring !== null}
                        aria-label={`Restore ${item.title}`} onClick={() => void restore(item)}>
                        <RotateCcw size={13} aria-hidden="true" /> Restore
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          {cursor && (
            <button className="button button-small" type="button" disabled={loadingMore}
              onClick={() => void loadMore()}>{loadingMore ? "Loading..." : "Load more"}</button>
          )}
        </section>
      </div>
    </main>
  );
}
