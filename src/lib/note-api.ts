import { invoke } from "@tauri-apps/api/core";

export interface Note {
  id: string;
  markdown: string;
  /** Derived by the core from the first line. */
  title: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface NoteSummary {
  id: string;
  title: string;
  revision: number;
  updatedAt: number;
}

export interface NotePage {
  items: NoteSummary[];
  nextCursor: string | null;
}

export const NOTE_PAGE_SIZE = 50;

// The desktop talks only to the Tauri command seam. Note rules live in Rust.
export const noteApi = {
  list(cursor: string | null = null): Promise<NotePage> {
    return invoke("list_notes", { limit: NOTE_PAGE_SIZE, cursor });
  },

  get(id: string): Promise<Note | null> {
    return invoke("get_note", { id });
  },

  create(markdown: string): Promise<Note> {
    return invoke("create_note", { input: { markdown } });
  },

  update(id: string, markdown: string, expectedRevision: number): Promise<Note> {
    return invoke("update_note", { input: { id, markdown, expectedRevision } });
  },
};
