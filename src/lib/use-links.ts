import { useEffect, useState } from "react";
import { errorMessage } from "./error-message";
import { linkApi, type LinkPage } from "./link-api";

interface LinkRow {
  linkId: string;
  title: string;
}

/**
 * The links on one side of a Task–Note relationship, as either page shows them.
 * `load` reads a page for `owner`; `relink` links the pair of an unlinked row
 * again, which the core answers with the same link (Undo).
 */
export function useLinks<T extends LinkRow>(
  owner: string,
  load: (cursor: string | null) => Promise<LinkPage<T>>,
  relink: (item: T) => Promise<unknown>,
) {
  const [items, setItems] = useState<T[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The link just ended, kept so Undo can bring it back.
  const [undoable, setUndoable] = useState<T | null>(null);

  useEffect(() => {
    let active = true;
    setItems([]);
    setNextCursor(null);
    setUndoable(null);
    setError(null);
    load(null)
      .then((page) => {
        if (!active) return;
        setItems(page.items);
        setNextCursor(page.nextCursor);
      })
      .catch((reason) => { if (active) setError("Couldn't load links: " + errorMessage(reason)); });
    return () => { active = false; };
    // load is rebuilt each render; the owner is what decides which links to read.
  }, [owner]);

  async function refresh() {
    const page = await load(null);
    setItems(page.items);
    setNextCursor(page.nextCursor);
  }

  /** Runs one link change, then reloads the first page. Resolves false when it failed. */
  async function change(action: () => Promise<unknown>, failure: string): Promise<boolean> {
    setBusy(true);
    setError(null);
    try {
      await action();
      await refresh();
      return true;
    } catch (reason) {
      setError(failure + errorMessage(reason));
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function unlink(item: T) {
    if (await change(() => linkApi.unlink(item.linkId), "Couldn't unlink: ")) setUndoable(item);
  }

  async function undo() {
    const item = undoable;
    if (item && await change(() => relink(item), "Couldn't undo: ")) setUndoable(null);
  }

  async function loadMore() {
    if (nextCursor === null) return;
    try {
      const page = await load(nextCursor);
      setItems((current) => [...current, ...page.items]);
      setNextCursor(page.nextCursor);
    } catch (reason) {
      setError("Couldn't load more links: " + errorMessage(reason));
    }
  }

  return { items, nextCursor, error, setError, busy, undoable, change, unlink, undo, loadMore };
}
