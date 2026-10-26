import type { Note, NoteFolder, NoteScope, NoteSummary } from "../lib/note-api";

/**
 * An in-memory stand-in for the core Note and folder commands behind the Tauri
 * seam. It mimics what the desktop observes (ordering, scopes, pages); the
 * rules themselves (cycles, depth, names, the title) are the core's and are
 * tested there. A test makes a command fail with `refuse`.
 */
export class NoteBackend {
  notes = new Map<string, Note>();
  folders = new Map<string, NoteFolder>();
  calls: { command: string; args: unknown }[] = [];
  /** When each Removed Note or folder was removed; the record itself stays. */
  removedAt = new Map<string, number>();
  /** The next call of a command throws this message, as the core would. */
  refusals = new Map<string, string>();
  private clock = 1_000;
  private nextId = 1;

  seed(markdown: string, title: string, fields: Partial<Note> = {}): Note {
    return this.write({
      id: `note-${this.nextId++}`, markdown, title, revision: 1, folderId: null, pinned: false, ...fields,
    });
  }

  folder(name: string, parent: NoteFolder | null = null): NoteFolder {
    const stamp = ++this.clock;
    const folder: NoteFolder = {
      id: `folder-${this.nextId++}`, parentId: parent?.id ?? null, name, revision: 1,
      createdAt: stamp, updatedAt: stamp, hasChildren: false,
    };
    this.folders.set(folder.id, folder);
    return folder;
  }

  refuse(command: string, message: string) {
    this.refusals.set(command, message);
  }

  private write(fields: Omit<Note, "createdAt" | "updatedAt">, touch = true): Note {
    const previous = this.notes.get(fields.id);
    const stamp = touch || !previous ? ++this.clock : previous.updatedAt;
    const note = { ...fields, createdAt: previous?.createdAt ?? stamp, updatedAt: stamp };
    this.notes.set(note.id, note);
    return note;
  }

  private withChildren(folder: NoteFolder): NoteFolder {
    const hasChildren = [...this.folders.values()]
      .some((other) => other.parentId === folder.id && !this.removedAt.has(other.id));
    return { ...folder, hasChildren };
  }

  /** A folder that is Removed or sits below a Removed folder, as the core hides it. */
  folderHidden(id: string | null): boolean {
    for (let at = id; at !== null; at = this.folders.get(at)?.parentId ?? null) {
      if (this.removedAt.has(at)) return true;
    }
    return false;
  }

  private noteShown(note: Note): boolean {
    return !this.removedAt.has(note.id) && !this.folderHidden(note.folderId);
  }

  handle(command: string, args: Record<string, unknown>) {
    this.calls.push({ command, args });
    const refusal = this.refusals.get(command);
    if (refusal) {
      this.refusals.delete(command);
      throw refusal;
    }
    if (command === "list_notes") {
      const scope = (args.folder as NoteScope | undefined) ?? { kind: "all" };
      const text = String(args.text ?? "");
      const words = text.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
      // As in the core: text without words ("!!!") matches nothing.
      if (text.trim() && words.length === 0) return { items: [], nextCursor: null };
      // As in the core: a folder hidden by Removed cannot be listed.
      if (scope.kind === "folder" && this.folderHidden(scope.folderId)) {
        throw "This folder, or a folder it is in, is in Removed; restore that folder first";
      }
      const all: NoteSummary[] = [...this.notes.values()]
        .filter((note) => this.noteShown(note))
        .filter((note) => scope.kind === "all" || note.folderId === scope.folderId)
        .filter((note) => words.every((word) => note.markdown.toLowerCase().includes(word)))
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt)
        .map(({ id, title, revision, updatedAt, folderId, pinned, markdown }) => ({
          id, title, revision, updatedAt, folderId, pinned,
          snippet: words.length ? snippetOf(markdown, words[0]) : null,
        }));
      return page(all, args);
    }
    if (command === "get_note") return this.notes.get(args.id as string) ?? null;
    // An open Note lists its linked Tasks (Ticket 08); these tests link none.
    if (command === "list_note_tasks") return { items: [], nextCursor: null };
    if (command === "create_note") {
      const { markdown, folderId } = args.input as { markdown: string; folderId?: string | null };
      return this.write({
        id: `note-${this.nextId++}`, markdown, title: firstLine(markdown), revision: 1,
        folderId: folderId ?? null, pinned: false,
      });
    }
    if (command === "update_note") {
      const input = args.input as { id: string; markdown: string; expectedRevision: number };
      const current = this.notes.get(input.id);
      if (!current) throw "Note not found";
      if (current.revision !== input.expectedRevision) {
        throw "This Note changed elsewhere; your text was not saved over it";
      }
      return this.write({
        ...current, markdown: input.markdown, title: firstLine(input.markdown), revision: current.revision + 1,
      });
    }
    if (command === "move_note" || command === "set_note_pinned") {
      const input = args.input as { id: string; folderId?: string | null; pinned?: boolean };
      const current = this.notes.get(input.id);
      if (!current) throw "Note not found";
      const placed = command === "move_note"
        ? { ...current, folderId: input.folderId ?? null }
        : { ...current, pinned: Boolean(input.pinned) };
      return this.write(placed, false);
    }
    if (command === "list_note_folders") {
      const parentId = (args.parentId as string | null | undefined) ?? null;
      const items = [...this.folders.values()]
        .filter((folder) => folder.parentId === parentId && !this.removedAt.has(folder.id))
        .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()))
        .map((folder) => this.withChildren(folder));
      return page(items, args);
    }
    if (command === "create_note_folder") {
      const input = args.input as { name: string; parentId: string | null };
      const parent = input.parentId ? this.folders.get(input.parentId) ?? null : null;
      return this.withChildren(this.folder(input.name.trim(), parent));
    }
    if (command === "rename_note_folder" || command === "move_note_folder") {
      const input = args.input as { id: string; expectedRevision: number; name?: string; parentId?: string | null };
      const current = this.folders.get(input.id);
      if (!current) throw "Folder not found";
      if (current.revision !== input.expectedRevision) throw "This folder changed elsewhere; reload before saving";
      const next: NoteFolder = {
        ...current,
        name: command === "rename_note_folder" ? input.name!.trim() : current.name,
        parentId: command === "move_note_folder" ? input.parentId ?? null : current.parentId,
        revision: current.revision + 1, updatedAt: ++this.clock,
      };
      this.folders.set(next.id, next);
      return this.withChildren(next);
    }
    if (command === "set_note_removed" || command === "set_note_folder_removed") {
      const input = args.input as { id: string; expectedRevision: number; removed: boolean };
      const isNote = command === "set_note_removed";
      const current = isNote ? this.notes.get(input.id) : this.folders.get(input.id);
      if (!current) throw isNote ? "Note not found" : "Folder not found";
      if (current.revision !== input.expectedRevision) {
        throw isNote ? "This Note changed elsewhere; your text was not saved over it"
          : "This folder changed elsewhere; reload before saving";
      }
      if (input.removed) this.removedAt.set(input.id, ++this.clock);
      else this.removedAt.delete(input.id);
      const next = { ...current, revision: current.revision + 1 };
      if (isNote) this.notes.set(next.id, next as Note);
      else this.folders.set(next.id, next as NoteFolder);
      return isNote ? next : this.withChildren(next as NoteFolder);
    }
    if (command === "list_removed") {
      const items = [...this.removedAt.entries()].map(([id, removedAt]) => {
        const note = this.notes.get(id);
        const folder = this.folders.get(id);
        const parent = note ? note.folderId : folder!.parentId;
        return {
          kind: note ? "note" : "note_folder", id, title: note ? note.title : folder!.name,
          revision: (note ?? folder)!.revision, removedAt, inRemovedContainer: this.folderHidden(parent),
        };
      }).sort((a, b) => b.removedAt - a.removedAt);
      return page(items, args);
    }
    throw new Error("Unexpected Note command: " + command);
  }

  commands(name: string) {
    return this.calls.filter((call) => call.command === name).map((call) => call.args);
  }

  /** Another writer (the CLI, an agent) saves a newer revision. */
  editElsewhere(id: string, markdown: string): Note {
    const current = this.notes.get(id)!;
    return this.write({ ...current, markdown, title: firstLine(markdown), revision: current.revision + 1 });
  }
}

function page<T>(all: T[], args: Record<string, unknown>) {
  const start = args.cursor ? Number(args.cursor) : 0;
  const end = start + (args.limit as number);
  return { items: all.slice(start, end), nextCursor: end < all.length ? String(end) : null };
}

function snippetOf(markdown: string, word: string) {
  const text = markdown.replace(/\s+/g, " ");
  const at = text.toLowerCase().indexOf(word);
  const end = text.indexOf(" ", at) === -1 ? text.length : text.indexOf(" ", at);
  return `${text.slice(Math.max(0, at - 20), at)}\u0002${text.slice(at, end)}\u0003${text.slice(end, end + 20)}`;
}

// The real title rule lives in the core; the stand-in only needs a plausible title.
export function firstLine(markdown: string) {
  return markdown.split("\n").find((line) => line.trim())?.replace(/^#+\s*/, "").trim() || "New Note";
}
