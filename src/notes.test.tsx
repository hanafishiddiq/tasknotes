import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Note } from "./lib/note-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

/** An in-memory stand-in for the core Note commands behind the Tauri seam. */
class NoteBackend {
  notes = new Map<string, Note>();
  calls: { command: string; args: unknown }[] = [];
  private clock = 1_000;
  private nextId = 1;

  seed(markdown: string, title: string): Note {
    const note = this.write({ id: `note-${this.nextId++}`, markdown, title, revision: 1 });
    return note;
  }

  private write(fields: Omit<Note, "createdAt" | "updatedAt">): Note {
    const stamp = ++this.clock;
    const previous = this.notes.get(fields.id);
    const note = { ...fields, createdAt: previous?.createdAt ?? stamp, updatedAt: stamp };
    this.notes.set(note.id, note);
    return note;
  }

  handle(command: string, args: Record<string, unknown>) {
    this.calls.push({ command, args });
    if (command === "list_notes") {
      const items = [...this.notes.values()]
        .sort((a, b) => b.updatedAt - a.updatedAt)
        .map(({ id, title, revision, updatedAt }) => ({ id, title, revision, updatedAt }));
      return { items, nextCursor: null };
    }
    if (command === "get_note") return this.notes.get(args.id as string) ?? null;
    if (command === "create_note") {
      const { markdown } = args.input as { markdown: string };
      return this.write({ id: `note-${this.nextId++}`, markdown, title: firstLine(markdown), revision: 1 });
    }
    if (command === "update_note") {
      const input = args.input as { id: string; markdown: string; expectedRevision: number };
      const current = this.notes.get(input.id);
      if (!current) throw "Note not found";
      if (current.revision !== input.expectedRevision) {
        throw "This Note changed elsewhere; your text was not saved over it";
      }
      return this.write({
        id: input.id, markdown: input.markdown, title: firstLine(input.markdown),
        revision: current.revision + 1,
      });
    }
    throw new Error("Unexpected Note command: " + command);
  }

  commands(name: string) {
    return this.calls.filter((call) => call.command === name).map((call) => call.args);
  }
}

// The real title rule lives in the core; the stand-in only needs a plausible title.
function firstLine(markdown: string) {
  return markdown.split("\n").find((line) => line.trim())?.replace(/^#+\s*/, "").trim() || "New Note";
}

let backend: NoteBackend;

// Autosave waits for a pause in typing, so saves need more than the default second.
const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists" || command === "list_tasks") return { items: [], nextCursor: null };
    return backend.handle(command, (args ?? {}) as Record<string, unknown>);
  });
});

afterEach(() => cleanup());

async function openNotes() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "All notes" }));
  return user;
}

describe("standalone Notes", () => {
  it("lists saved Notes and opens the most recently edited one", async () => {
    backend.seed("Groceries\n\n- Milk", "Groceries");
    backend.seed("# Release plan\n\nShip **v1** soon", "Release plan");
    await openNotes();

    const list = await screen.findByRole("list", { name: "Notes" });
    await waitFor(() => expect(within(list).getAllByRole("button")).toHaveLength(2));
    const items = within(list).getAllByRole("button").map((button) => button.textContent);
    expect(items[0]).toContain("Release plan");
    expect(items[1]).toContain("Groceries");
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Release plan"));
    expect(within(editor).getByRole("heading", { name: "Release plan" })).toBeInTheDocument();
    expect(within(editor).getByText("v1").tagName).toBe("STRONG");
    expect(backend.commands("list_notes")[0]).toEqual({ limit: 50, cursor: null });
  });

  it("creates a Note only once its first character is typed, then keeps saving it", async () => {
    backend.seed("Groceries", "Groceries");
    const user = await openNotes();
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Groceries"));
    await user.click(await screen.findByRole("button", { name: "New note" }));
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    expect(editor).toHaveTextContent("");
    expect(backend.commands("create_note")).toHaveLength(0);

    await user.click(editor);
    await user.type(editor, "Buy oat milk");
    await eventually(() => expect(backend.commands("create_note")).toEqual([{ input: { markdown: "Buy oat milk" } }]));
    const list = screen.getByRole("list", { name: "Notes" });
    await waitFor(() => expect(within(list).getAllByRole("button")[0]).toHaveTextContent("Buy oat milk"));
    expect(within(list).getAllByRole("button")[0]).toHaveAttribute("aria-current", "true");

    await user.type(editor, " today");
    await eventually(() => expect(backend.commands("update_note")).toContainEqual({
      input: { id: "note-2", markdown: "Buy oat milk today", expectedRevision: 1 },
    }));
    expect(backend.commands("create_note")).toHaveLength(1);
    expect(backend.notes.get("note-2")?.markdown).toBe("Buy oat milk today");
  });

  it("leaves nothing behind when a new Note is abandoned before typing", async () => {
    backend.seed("Groceries", "Groceries");
    const user = await openNotes();
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Groceries"));
    await user.click(screen.getByRole("button", { name: "New note" }));
    const draft = screen.getByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(draft).toHaveTextContent(""));
    await user.click(draft);
    await user.type(draft, "   ");
    await user.click(within(screen.getByRole("list", { name: "Notes" })).getByRole("button", { name: /Groceries/ }));

    await waitFor(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Groceries"));
    expect(backend.commands("create_note")).toHaveLength(0);
    expect(backend.commands("update_note")).toHaveLength(0);
    expect(within(screen.getByRole("list", { name: "Notes" })).getAllByRole("button")).toHaveLength(1);
  });

  it("saves pending typing before opening another Note", async () => {
    backend.seed("Groceries", "Groceries");
    backend.seed("Meeting notes", "Meeting notes");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Meeting notes"));
    await user.click(editor);
    await user.type(editor, " with Dina");
    // Switch right away, before the autosave pause has passed.
    await user.click(within(screen.getByRole("list", { name: "Notes" })).getByRole("button", { name: /Groceries/ }));

    await waitFor(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Groceries"));
    expect(backend.notes.get("note-2")?.markdown).toBe("Meeting notes with Dina");
    const order = backend.calls.map((call) => call.command);
    expect(order.lastIndexOf("update_note")).toBeLessThan(order.lastIndexOf("get_note"));
  });

  it("reopens what was typed after the app restarts", async () => {
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await user.click(editor);
    await user.type(editor, "Packing list");
    await eventually(() => expect(backend.notes.size).toBe(1));
    cleanup();

    await openNotes();
    const reopened = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(reopened).toHaveTextContent("Packing list"));
    expect(within(screen.getByRole("list", { name: "Notes" })).getByRole("button", { name: /Packing list/ }))
      .toHaveAttribute("aria-current", "true");
  });

  it("keeps the typed text on screen when a save fails, and saves it on the next edit", async () => {
    backend.seed("Groceries", "Groceries");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Groceries"));
    const handle = backend.handle.bind(backend);
    let failNext = true;
    backend.handle = (command, args) => {
      if (command === "update_note" && failNext) {
        failNext = false;
        throw "database is locked";
      }
      return handle(command, args);
    };
    await user.click(editor);
    await user.type(editor, " and eggs");
    expect(await screen.findByRole("alert", {}, { timeout: 3000 }))
      .toHaveTextContent("Couldn't save this Note: database is locked");
    expect(editor).toHaveTextContent("Groceries and eggs");

    await user.type(editor, "!");
    await eventually(() => expect(backend.notes.get("note-1")?.markdown).toBe("Groceries and eggs!"));
    await eventually(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });
});
