import { invoke } from "./invoke";

export interface Note {
  id: string;
  markdown: string;
  /** Derived by the core from the first line. */
  title: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
  /** null when the Note is not in a folder. */
  folderId: string | null;
  pinned: boolean;
  /** In Removed. Only a Note read by its id can say so; lists leave it out. */
  removed: boolean;
  /** Exclude from AI: AI interfaces (CLI, MCP) treat the Note as missing. */
  excludeFromAi: boolean;
}

export interface NoteSummary {
  id: string;
  title: string;
  revision: number;
  updatedAt: number;
  folderId: string | null;
  pinned: boolean;
  /** Search hits only: a few words around the match, matched words between U+0002 and U+0003. */
  snippet: string | null;
}

export interface NotePage {
  items: NoteSummary[];
  nextCursor: string | null;
}

/** Every Note, or only the Notes directly in one folder. */
export type NoteScope = { kind: "all" } | { kind: "folder"; folderId: string };

export const ALL_NOTES: NoteScope = { kind: "all" };

export interface NoteFolder {
  id: string;
  parentId: string | null;
  name: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
  hasChildren: boolean;
}

export interface NoteFolderPage {
  items: NoteFolder[];
  nextCursor: string | null;
}

export const NOTE_PAGE_SIZE = 50;
export const FOLDER_PAGE_SIZE = 50;
/** How many Notes global search shows beside Tasks. */
export const GLOBAL_NOTE_HITS = 5;

// The desktop talks only to the Tauri command seam. Note rules live in Rust.
export const noteApi = {
  list(cursor: string | null = null, folder: NoteScope = ALL_NOTES, text = ""): Promise<NotePage> {
    return invoke("list_notes", { limit: NOTE_PAGE_SIZE, cursor, folder, text });
  },

  /** The first few matches in every folder, for global search. */
  search(text: string): Promise<NotePage> {
    return invoke("list_notes", { limit: GLOBAL_NOTE_HITS, cursor: null, folder: ALL_NOTES, text });
  },

  get(id: string): Promise<Note | null> {
    return invoke("get_note", { id });
  },

  create(markdown: string, folderId: string | null = null): Promise<Note> {
    return invoke("create_note", { input: { markdown, folderId } });
  },

  update(id: string, markdown: string, expectedRevision: number): Promise<Note> {
    return invoke("update_note", { input: { id, markdown, expectedRevision } });
  },

  move(id: string, folderId: string | null): Promise<Note> {
    return invoke("move_note", { input: { id, folderId } });
  },

  setPinned(id: string, pinned: boolean): Promise<Note> {
    return invoke("set_note_pinned", { input: { id, pinned } });
  },

  setExcludeFromAi(id: string, excludeFromAi: boolean): Promise<Note> {
    return invoke("set_note_exclude_from_ai", { input: { id, excludeFromAi } });
  },

  folders(parentId: string | null, cursor: string | null = null): Promise<NoteFolderPage> {
    return invoke("list_note_folders", { parentId, limit: FOLDER_PAGE_SIZE, cursor });
  },

  createFolder(name: string, parentId: string | null): Promise<NoteFolder> {
    return invoke("create_note_folder", { input: { name, parentId } });
  },

  renameFolder(folder: NoteFolder, name: string): Promise<NoteFolder> {
    return invoke("rename_note_folder", {
      input: { id: folder.id, expectedRevision: folder.revision, name },
    });
  },

  moveFolder(folder: NoteFolder, parentId: string | null): Promise<NoteFolder> {
    return invoke("move_note_folder", {
      input: { id: folder.id, expectedRevision: folder.revision, parentId },
    });
  },
};
