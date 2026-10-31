import { useState } from "react";
import { errorMessage } from "./error-message";

/** A move to Removed that can still be taken back. */
export interface Undoable {
  /** What was moved, e.g. "task:<id>", so restoring it elsewhere can drop it here. */
  key: string;
  title: string;
  undo: () => Promise<void>;
}

/** How many recent moves to Removed stay undoable. */
export const UNDO_DEPTH = 10;

export interface UndoControl {
  /** The move Undo takes back next, if any. */
  next: Undoable | null;
  /**
   * Whether the notice for `next` is on screen; it leaves on navigation, the
   * stack stays. A failed Undo takes its place in the bar until dismissed.
   */
  noticeShown: boolean;
  undoing: boolean;
  error: string | null;
  push: (item: Undoable) => void;
  /** Takes back the newest move; false when there is nothing to undo. */
  run: () => boolean;
  /** Drops an item restored some other way (the Removed view). */
  forget: (key: string) => void;
  hideNotice: () => void;
  dismissError: () => void;
}

/**
 * The recent moves to Removed, newest first. ⌘Z, the notice and the palette
 * each take back the newest; moving between views keeps them.
 */
export function useUndo(): UndoControl {
  const [stack, setStack] = useState<Undoable[]>([]);
  const [noticeShown, setNoticeShown] = useState(false);
  const [undoing, setUndoing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const next = stack[0] ?? null;

  function push(item: Undoable) {
    setStack((current) => [item, ...current.filter((other) => other.key !== item.key)].slice(0, UNDO_DEPTH));
    setNoticeShown(true);
    setError(null);
  }

  function run(): boolean {
    if (!next || undoing) return false;
    setStack((current) => current.slice(1));
    setError(null);
    setUndoing(true);
    next.undo()
      .catch((reason) => setError("Couldn't undo: " + errorMessage(reason)))
      .finally(() => setUndoing(false));
    return true;
  }

  return {
    next, noticeShown: noticeShown && next !== null && error === null, undoing, error, push, run,
    forget: (key) => setStack((current) => current.filter((item) => item.key !== key)),
    hideNotice: () => setNoticeShown(false),
    dismissError: () => setError(null),
  };
}
