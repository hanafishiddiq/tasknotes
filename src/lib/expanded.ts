/**
 * Which rows of a sidebar tree a person opened: a view preference kept per
 * window, like the theme. Lists and Note Folders keep theirs the same way,
 * as the ids of the open rows, so both start collapsed (owner decision Q34).
 */

export function readExpanded(key: string): Set<string> {
  try {
    const stored: unknown = JSON.parse(window.localStorage.getItem(key) ?? "[]");
    return new Set(Array.isArray(stored) ? stored.filter((id): id is string => typeof id === "string") : []);
  } catch {
    return new Set();
  }
}

export function writeExpanded(key: string, expanded: Set<string>) {
  try {
    window.localStorage.setItem(key, JSON.stringify([...expanded]));
  } catch {
    // Storage unavailable: the tree still works, it just opens collapsed next time.
  }
}
