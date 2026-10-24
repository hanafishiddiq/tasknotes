import { invoke } from "@tauri-apps/api/core";
import type { WorkStatus } from "./task-api";

/** A reversible Task–Note association; unlinking keeps the row, so Undo restores the same ID. */
export interface NoteLink {
  id: string;
  taskId: string;
  noteId: string;
  linkedAt: number;
  unlinkedAt: number | null;
}

export interface LinkedNote {
  linkId: string;
  noteId: string;
  title: string;
  updatedAt: number;
}

export interface LinkedTask {
  linkId: string;
  taskId: string;
  title: string;
  status: WorkStatus;
}

export interface LinkPage<T> {
  items: T[];
  nextCursor: string | null;
}

export const LINK_PAGE_SIZE = 50;

// The desktop talks only to the Tauri command seam. Link rules live in Rust.
export const linkApi = {
  link(taskId: string, noteId: string): Promise<NoteLink> {
    return invoke("link_note", { taskId, noteId });
  },

  unlink(linkId: string): Promise<NoteLink> {
    return invoke("unlink_note", { linkId });
  },

  notesOfTask(taskId: string, cursor: string | null = null): Promise<LinkPage<LinkedNote>> {
    return invoke("list_task_notes", { taskId, limit: LINK_PAGE_SIZE, cursor });
  },

  tasksOfNote(noteId: string, cursor: string | null = null): Promise<LinkPage<LinkedTask>> {
    return invoke("list_note_tasks", { noteId, limit: LINK_PAGE_SIZE, cursor });
  },
};
