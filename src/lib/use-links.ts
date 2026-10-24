import { useEffect, useRef, useState } from "react";
import { errorMessage } from "./error-message";
import { linkApi, type LinkPage } from "./link-api";

interface LinkRow {
  linkId: string;
  title: string;
}

/**
 * The links on one side of a Task–Note relationship, as either page shows them.
 * `load` reads a page for `owner`; Undo restores the same link in its old
 * place. The links are read
 * again whenever `shown` turns true, since the other side may have changed them.
 */
export function useLinks<T extends LinkRow>(
  owner: string,
  shown: boolean,
  load: (cursor: string | null) => Promise<LinkPage<T>>,
) {
  const [items, setItems] = useState<T[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The link just ended, kept so Undo can bring it back.
  const [undoable, setUndoable] = useState<T | null>(null);
  // Answers for an owner that is no longer shown are dropped.
  const latestOwner = useRef(owner);
  latestOwner.current = owner;

  useEffect(() => {
    if (!shown) return;
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
  }, [owner, shown]);

  async function refresh() {
    const asked = owner;
    const page = await load(null);
    if (asked !== latestOwner.current) return;
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
    if (item && await change(() => linkApi.restore(item.linkId), "Couldn't undo: ")) setUndoable(null);
  }

  async function loadMore() {
    if (nextCursor === null) return;
    const asked = owner;
    try {
      const page = await load(nextCursor);
      if (asked !== latestOwner.current) return;
      setItems((current) => [...current, ...page.items]);
      setNextCursor(page.nextCursor);
    } catch (reason) {
      setError("Couldn't load more links: " + errorMessage(reason));
    }
  }

  return { items, nextCursor, error, setError, busy, undoable, change, unlink, undo, loadMore };
}
