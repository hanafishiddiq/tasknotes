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

function setRemoved<T>(kind: RemovedKind, id: string, expectedRevision: number, removed: boolean): Promise<T> {
  return invoke(COMMAND[kind], { input: { id, expectedRevision, removed } });
}

// The desktop talks only to the Tauri command seam. What Removed hides, and
// what may be restored, is decided in Rust. There is no command that deletes.
export const removedApi = {
  list(cursor: string | null = null): Promise<RemovedPage> {
    return invoke("list_removed", { limit: REMOVED_PAGE_SIZE, cursor });
  },

  restore(item: RemovedItem): Promise<Task | Note | TaskList | NoteFolder> {
    return setRemoved(item.kind, item.id, item.revision, false);
  },

  removeTask(task: Task): Promise<Task> {
    return setRemoved("task", task.id, task.revision, true);
  },

  restoreTask(task: Task): Promise<Task> {
    return setRemoved("task", task.id, task.revision, false);
  },

  removeNote(id: string, revision: number): Promise<Note> {
    return setRemoved("note", id, revision, true);
  },

  removeFolder(folder: NoteFolder): Promise<NoteFolder> {
    return setRemoved("note_folder", folder.id, folder.revision, true);
  },
};
