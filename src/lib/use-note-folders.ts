import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { errorMessage } from "./error-message";
import { noteApi, type NoteFolder } from "./note-api";
import { removedApi } from "./removed-api";

/** Which folders are open in the sidebar; a view preference kept per window, like the theme. */
export const EXPANDED_FOLDERS_KEY = "tasknotes.noteFolders.expanded";

/** One loaded page (or more) of a folder's subfolders; the key "" is the top level. */
interface Level {
  items: NoteFolder[];
  cursor: string | null;
}

export interface FolderOption {
  id: string;
  /** "Work / Meetings": where the folder sits in the tree. */
  label: string;
}

const key = (parentId: string | null) => parentId ?? "";

function readExpanded(): Set<string> {
  try {
    const stored = JSON.parse(window.localStorage.getItem(EXPANDED_FOLDERS_KEY) ?? "[]");
    return new Set(Array.isArray(stored) ? stored.filter((id) => typeof id === "string") : []);
  } catch {
    return new Set();
  }
}

function writeExpanded(expanded: Set<string>) {
  try {
    window.localStorage.setItem(EXPANDED_FOLDERS_KEY, JSON.stringify([...expanded]));
  } catch {
    // Storage unavailable: the tree still works, it just opens collapsed next time.
  }
}

/**
 * The Note Folder tree as the sidebar shows it: one bounded page of children
 * per open folder. Every rule (names, depth, cycles) is the core's; this only
 * loads, remembers what is open and reports the core's answer.
 */
export function useNoteFolders(enabled: boolean) {
  const [levels, setLevels] = useState<Record<string, Level>>({});
  const [expanded, setExpanded] = useState<Set<string>>(readExpanded);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const expandedRef = useRef(expanded);
  expandedRef.current = expanded;

  /** A level and every open folder below it, first page each. */
  const loadTree = useCallback(async (parentId: string | null): Promise<Record<string, Level>> => {
    const page = await noteApi.folders(parentId);
    let loaded: Record<string, Level> = { [key(parentId)]: { items: page.items, cursor: page.nextCursor } };
    for (const folder of page.items) {
      if (folder.hasChildren && expandedRef.current.has(folder.id)) {
        loaded = { ...loaded, ...(await loadTree(folder.id)) };
      }
    }
    return loaded;
  }, []);

  const reload = useCallback(async () => {
    const tree = await loadTree(null);
    setLevels(tree);
  }, [loadTree]);

  useEffect(() => {
    if (!enabled) return;
    reload().catch((reason) => setError("Couldn't load folders: " + errorMessage(reason)));
  }, [enabled, reload]);

  const remember = (next: Set<string>) => {
    expandedRef.current = next;
    setExpanded(next);
    writeExpanded(next);
  };

  async function toggle(folder: NoteFolder) {
    const next = new Set(expandedRef.current);
    if (next.has(folder.id)) {
      next.delete(folder.id);
      remember(next);
      return;
    }
    next.add(folder.id);
    remember(next);
    try {
      const subtree = await loadTree(folder.id);
      setLevels((current) => ({ ...current, ...subtree }));
    } catch (reason) {
      setError("Couldn't load folders: " + errorMessage(reason));
    }
  }

  async function loadMore(parentId: string | null) {
    const level = levels[key(parentId)];
    if (!level?.cursor) return;
    try {
      const page = await noteApi.folders(parentId, level.cursor);
      setLevels((current) => ({
        ...current,
        [key(parentId)]: { items: [...level.items, ...page.items], cursor: page.nextCursor },
      }));
    } catch (reason) {
      setError("Couldn't load more folders: " + errorMessage(reason));
    }
  }

  /** Runs one folder write, then reloads the tree; resolves false when the core refused it. */
  async function write(change: () => Promise<unknown>, open?: string | null): Promise<boolean> {
    if (busy) return false;
    setBusy(true);
    setError(null);
    try {
      await change();
      if (open) remember(new Set(expandedRef.current).add(open));
      try {
        await reload();
      } catch (reason) {
        setError("Saved, but the folders could not refresh: " + errorMessage(reason));
      }
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      return false;
    } finally {
      setBusy(false);
    }
  }

  const create = (name: string, parentId: string | null) =>
    write(() => noteApi.createFolder(name, parentId), parentId);
  const rename = (folder: NoteFolder, name: string) => write(() => noteApi.renameFolder(folder, name));
  const move = (folder: NoteFolder, parentId: string | null) =>
    write(() => noteApi.moveFolder(folder, parentId), parentId);

  /** Every loaded folder in tree order, with its path, for pickers. */
  const options = useMemo(() => {
    const out: FolderOption[] = [];
    const walk = (parentId: string | null, path: string) => {
      for (const folder of levels[key(parentId)]?.items ?? []) {
        const label = path ? `${path} / ${folder.name}` : folder.name;
        out.push({ id: folder.id, label });
        walk(folder.id, label);
      }
    };
    walk(null, "");
    return out;
  }, [levels]);

  const byId = useMemo(() => {
    const map = new Map<string, NoteFolder>();
    for (const level of Object.values(levels)) for (const folder of level.items) map.set(folder.id, folder);
    return map;
  }, [levels]);

  /** Moves a folder, with everything in it, to Removed. */
  const remove = (folder: NoteFolder) => write(() => removedApi.remove<NoteFolder>("note_folder", folder));

  return {
    reload,
    remove,
    level: (parentId: string | null): Level | undefined => levels[key(parentId)],
    isExpanded: (id: string) => expanded.has(id),
    byId, options, error, busy, toggle, loadMore, create, rename, move,
    dismissError: () => setError(null),
  };
}

export type NoteFolders = ReturnType<typeof useNoteFolders>;
