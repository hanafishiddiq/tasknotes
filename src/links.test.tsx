import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task } from "./lib/task-api";
import type { Note } from "./lib/note-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

interface Link { id: string; taskId: string; noteId: string; linkedAt: number; unlinkedAt: number | null }

const BASE_TASK: Task = {
  id: "", title: "", description: "", status: "ready", priority: "medium", progress: 0,
  nextStep: "", revision: 1, createdAt: 100, updatedAt: 100, listId: null, dueDate: null, tags: [],
};

/** An in-memory stand-in for the core behind the Tauri seam: Tasks, Notes and their links. */
class Backend {
  tasks: Task[] = [];
  notes = new Map<string, Note>();
  links: Link[] = [];
  calls: { command: string; args: Record<string, unknown> }[] = [];
  private clock = 1_000;

  task(id: string, title: string, fields: Partial<Task> = {}): Task {
    const task = { ...BASE_TASK, id, title, ...fields };
    this.tasks.push(task);
    return task;
  }

  note(id: string, title: string): Note {
    const stamp = ++this.clock;
    const note = { id, markdown: `# ${title}`, title, revision: 1, createdAt: stamp, updatedAt: stamp };
    this.notes.set(id, note);
    return note;
  }

  link(taskId: string, noteId: string): Link {
    const existing = this.links.find((l) => l.taskId === taskId && l.noteId === noteId);
    if (existing) {
      existing.unlinkedAt = null;
      return { ...existing };
    }
    const link = { id: `link-${this.links.length + 1}`, taskId, noteId, linkedAt: ++this.clock, unlinkedAt: null };
    this.links.push(link);
    return { ...link };
  }

  private active() {
    return this.links.filter((l) => l.unlinkedAt === null).sort((a, b) => b.linkedAt - a.linkedAt);
  }

  commands(name: string) {
    return this.calls.filter((call) => call.command === name).map((call) => call.args);
  }

  handle(command: string, args: Record<string, unknown>) {
    this.calls.push({ command, args });
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_progress_entries": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      case "list_tasks": {
        const items = args.taskId ? this.tasks.filter((t) => t.id === args.taskId) : this.tasks;
        return { items: items.map((t) => ({ ...t })), nextCursor: null };
      }
      case "list_notes": {
        const items = [...this.notes.values()].sort((a, b) => b.updatedAt - a.updatedAt)
          .map(({ id, title, revision, updatedAt }) => ({ id, title, revision, updatedAt }));
        return { items, nextCursor: null };
      }
      case "get_note": return this.notes.get(args.id as string) ?? null;
      case "link_note": {
        if (!this.tasks.some((t) => t.id === args.taskId)) throw "Task not found";
        if (!this.notes.has(args.noteId as string)) throw "Note not found";
        return this.link(args.taskId as string, args.noteId as string);
      }
      case "unlink_note": {
        const link = this.links.find((l) => l.id === args.linkId && l.unlinkedAt === null);
        if (!link) throw "This link was not found or is already unlinked";
        link.unlinkedAt = ++this.clock;
        return { ...link };
      }
      case "list_task_notes": {
        const items = this.active().filter((l) => l.taskId === args.taskId).map((l) => {
          const note = this.notes.get(l.noteId)!;
          return { linkId: l.id, noteId: note.id, title: note.title, updatedAt: note.updatedAt };
        });
        return { items, nextCursor: null };
      }
      case "list_note_tasks": {
        const items = this.active().filter((l) => l.noteId === args.noteId).map((l) => {
          const task = this.tasks.find((t) => t.id === l.taskId)!;
          return { linkId: l.id, taskId: task.id, title: task.title, status: task.status };
        });
        return { items, nextCursor: null };
      }
      default: throw new Error("Unexpected native command " + command);
    }
  }
}

let backend: Backend;

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  backend = new Backend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) =>
    backend.handle(command, (args ?? {}) as Record<string, unknown>));
});

afterEach(() => cleanup());

async function openTask(title: string) {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: `Open ${title}` }));
  return user;
}

const linkedNotes = () => screen.findByRole("list", { name: "Linked Notes" });

describe("Task–Note links from a Task", () => {
  it("lists the Task's linked Notes and opens one in Notes", async () => {
    backend.task("task-1", "Book the ferry");
    backend.note("note-1", "Trip plan");
    backend.note("note-2", "Budget");
    backend.link("task-1", "note-1");
    const user = await openTask("Book the ferry");

    const list = await linkedNotes();
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
    await user.click(within(list).getByRole("button", { name: "Trip plan" }));

    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Trip plan"));
    const notes = screen.getByRole("list", { name: "Notes" });
    expect(within(notes).getByRole("button", { current: true })).toHaveTextContent("Trip plan");
  });

  it("links a Note picked from the recent Notes, once", async () => {
    backend.task("task-1", "Book the ferry");
    backend.note("note-1", "Trip plan");
    backend.note("note-2", "Budget");
    backend.link("task-1", "note-1");
    const user = await openTask("Book the ferry");
    await waitFor(async () => expect(within(await linkedNotes()).getAllByRole("listitem")).toHaveLength(1));

    await user.click(screen.getByRole("button", { name: "Link a Note" }));
    const picker = await screen.findByRole("list", { name: "Notes to link" });
    // Already linked Notes are not offered again.
    await waitFor(() => expect(within(picker).getAllByRole("button").map((b) => b.textContent)).toEqual(["Budget"]));
    await user.click(within(picker).getByRole("button", { name: "Budget" }));

    const list = await linkedNotes();
    await waitFor(() => expect(within(list).getAllByRole("listitem").map((li) => li.textContent))
      .toEqual(["Budget", "Trip plan"]));
    expect(backend.commands("link_note")).toEqual([{ taskId: "task-1", noteId: "note-2" }]);
    expect(screen.queryByRole("list", { name: "Notes to link" })).not.toBeInTheDocument();
  });

  it("unlinks a Note without touching it, and Undo brings the same link back", async () => {
    backend.task("task-1", "Book the ferry");
    backend.note("note-1", "Trip plan");
    const original = backend.link("task-1", "note-1");
    const user = await openTask("Book the ferry");
    const list = await linkedNotes();
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));

    await user.click(within(list).getByRole("button", { name: "Unlink Trip plan" }));

    await waitFor(() => expect(within(list).queryAllByRole("listitem")).toHaveLength(0));
    expect(screen.getByRole("status", { name: "Unlinked" })).toHaveTextContent("Unlinked “Trip plan”");
    expect(backend.commands("unlink_note")).toEqual([{ linkId: original.id }]);
    // Only the relationship changed: no Note or Task write went through the seam.
    expect(backend.calls.map((c) => c.command)).not.toContain("update_note");
    expect(backend.notes.get("note-1")!.revision).toBe(1);

    await user.click(screen.getByRole("button", { name: "Undo unlink" }));

    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
    expect(backend.links.filter((l) => l.unlinkedAt === null).map((l) => l.id)).toEqual([original.id]);
    expect(screen.queryByRole("status", { name: "Unlinked" })).not.toBeInTheDocument();
  });
});

