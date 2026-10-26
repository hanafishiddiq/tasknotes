import { invoke } from "@tauri-apps/api/core";
import type { Task, TaskList } from "./task-api";
import type { Note, NoteFolder } from "./note-api";

export type RemovedKind = "task" | "note" | "list" | "note_folder";

/** One record in Removed, as the core lists it. */
export interface RemovedItem {
  kind: RemovedKind;
  id: string;
  /** A Task's or Note's title, a List's or folder's name. */
  title: string;
  revision: number;
  removedAt: number;
  /** Its List, or a folder above it, is in Removed too and must come back first. */
  inRemovedContainer: boolean;
}

export interface RemovedPage {
  items: RemovedItem[];
  nextCursor: string | null;
}

export const REMOVED_PAGE_SIZE = 50;

const COMMAND: Record<RemovedKind, string> = {
  task: "set_task_removed",
  note: "set_note_removed",
  list: "set_list_removed",
  note_folder: "set_note_folder_removed",
};

/** Anything that can be moved to Removed: its id and the revision a change must name. */
export interface Revisioned {
  id: string;
  revision: number;
}

function setRemoved<T>(kind: RemovedKind, item: Revisioned, removed: boolean): Promise<T> {
  return invoke(COMMAND[kind], { input: { id: item.id, expectedRevision: item.revision, removed } });
}

// The desktop talks only to the Tauri command seam. What Removed hides, and
// what may be restored, is decided in Rust. There is no command that deletes.
export const removedApi = {
  /** One page of Removed; `text` searches it (blank lists everything). */
  list(text = "", cursor: string | null = null): Promise<RemovedPage> {
    return invoke("list_removed", { text, limit: REMOVED_PAGE_SIZE, cursor });
  },

  remove<T extends Task | Note | TaskList | NoteFolder>(kind: RemovedKind, item: Revisioned): Promise<T> {
    return setRemoved(kind, item, true);
  },

  restore<T extends Task | Note | TaskList | NoteFolder>(kind: RemovedKind, item: Revisioned): Promise<T> {
    return setRemoved(kind, item, false);
  },
};
