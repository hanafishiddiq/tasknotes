import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Note } from "./lib/note-api";
import { tauriEvents } from "./test/tauri-events";

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

  /** Another writer (the CLI, an agent) saves a newer revision. */
  editElsewhere(id: string, markdown: string): Note {
    const current = this.notes.get(id)!;
    return this.write({ id, markdown, title: firstLine(markdown), revision: current.revision + 1 });
  }
}

// The real title rule lives in the core; the stand-in only needs a plausible title.
function firstLine(markdown: string) {
  return markdown.split("\n").find((line) => line.trim())?.replace(/^#+\s*/, "").trim() || "New Note";
}

let backend: NoteBackend;

// Autosave waits for a pause in typing, so saves need more than the default second.
const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

// The app loads the editor lazily; importing it once here keeps that one-time
// module transform out of each test's wait for the Notes view.
beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists" || command === "list_tasks") return { items: [], nextCursor: null };
    if (command === "quit_ready") return null;
    return backend.handle(command, (args ?? {}) as Record<string, unknown>);
  });
});

afterEach(() => cleanup());

/** The desktop shell asks the page before it quits (Cmd+Q, closing the window). */
const quitRequested = () => tauriEvents.emit("tasknotes://quit-requested");

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

  it("pauses autosave and asks what to do when the Note changed elsewhere", async () => {
    backend.seed("Groceries", "Groceries");
    backend.seed("Trip plan\n\nBook the train", "Trip plan");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Book the train"));
    backend.editElsewhere("note-2", "Trip plan\n\nBook the ferry");

    await user.click(editor);
    await user.keyboard("My ");
    const banner = await screen.findByRole("alert", {}, { timeout: 3000 });
    expect(banner).toHaveTextContent("This Note was changed somewhere else");
    expect(within(banner).getByRole("button", { name: "Load latest" })).toBeInTheDocument();
    expect(within(banner).getByRole("button", { name: "Save mine as a new Note" })).toBeInTheDocument();
    expect(editor).toHaveTextContent("My Trip plan");
    expect(backend.notes.get("note-2")?.markdown).toBe("Trip plan\n\nBook the ferry");

    // Further typing is not saved over the other version, and the Note is not left behind.
    await user.keyboard("own ");
    await new Promise((resolve) => setTimeout(resolve, 900));
    expect(backend.commands("update_note")).toHaveLength(1);
    await user.click(within(screen.getByRole("list", { name: "Notes" })).getByRole("button", { name: /Groceries/ }));
    expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("My own Trip plan");
    expect(screen.getByRole("alert")).toHaveTextContent("This Note was changed somewhere else");
    expect(backend.commands("update_note")).toHaveLength(1);
  });

  it("loads the other version on Load latest and saves new edits on top of it", async () => {
    backend.seed("Trip plan\n\nBook the train", "Trip plan");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Book the train"));
    backend.editElsewhere("note-1", "Ferry trip\n\nBook the ferry");
    await user.click(editor);
    await user.keyboard("My ");
    const banner = await screen.findByRole("alert", {}, { timeout: 3000 });

    await user.click(within(banner).getByRole("button", { name: "Load latest" }));
    const reloaded = screen.getByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(reloaded).toHaveTextContent("Book the ferry"));
    expect(reloaded).not.toHaveTextContent("My Trip plan");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Notes" })).getByRole("button", { name: /Ferry trip/ }))
      .toHaveAttribute("aria-current", "true");

    await user.click(reloaded);
    await user.keyboard("Our ");
    await eventually(() => expect(backend.notes.get("note-1")?.markdown).toBe("Our Ferry trip\n\nBook the ferry"));
    const updates = backend.commands("update_note");
    expect(updates[updates.length - 1]).toMatchObject({ input: { expectedRevision: 2 } });
  });

  it("keeps both versions on Save mine as a new Note", async () => {
    backend.seed("Trip plan\n\nBook the train", "Trip plan");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Book the train"));
    backend.editElsewhere("note-1", "Trip plan\n\nBook the ferry");
    await user.click(editor);
    await user.keyboard("My ");
    const banner = await screen.findByRole("alert", {}, { timeout: 3000 });

    await user.click(within(banner).getByRole("button", { name: "Save mine as a new Note" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(backend.notes.get("note-2")?.markdown).toBe("My Trip plan\n\nBook the train");
    expect(backend.notes.get("note-1")?.markdown).toBe("Trip plan\n\nBook the ferry");
    const items = within(screen.getByRole("list", { name: "Notes" })).getAllByRole("button");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("My Trip plan");
    expect(items[0]).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("My Trip plan");

    await user.click(screen.getByRole("textbox", { name: "Note text" }));
    await user.keyboard("Ok ");
    await eventually(() => expect(backend.notes.get("note-2")?.markdown).toBe("Ok My Trip plan\n\nBook the train"));
    expect(backend.notes.get("note-1")?.markdown).toBe("Trip plan\n\nBook the ferry");
  });

  it("saves typing that has not reached autosave before the app quits", async () => {
    backend.seed("Trip plan", "Trip plan");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Trip plan"));
    await user.click(editor);
    await user.keyboard("My ");

    // Quit before the autosave pause has passed.
    await quitRequested();
    expect(backend.notes.get("note-1")?.markdown).toBe("My Trip plan");
    const commands = invokeMock.mock.calls.map(([command]) => command);
    expect(commands.lastIndexOf("update_note")).toBeLessThan(commands.indexOf("quit_ready"));
    expect(invokeMock).toHaveBeenLastCalledWith("quit_ready", { proceed: true });
  });

  it("stays open on quit when the Note cannot be saved, and shows why", async () => {
    backend.seed("Trip plan", "Trip plan");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Trip plan"));
    backend.editElsewhere("note-1", "Ferry plan");
    await user.click(editor);
    await user.keyboard("My ");
    await user.click(screen.getByRole("button", { name: "All tasks" }));
    expect(screen.queryByRole("textbox", { name: "Note text" })).not.toBeInTheDocument();

    await quitRequested();
    expect(invokeMock).toHaveBeenLastCalledWith("quit_ready", { proceed: false });
    expect(await screen.findByRole("textbox", { name: "Note text" })).toHaveTextContent("My Trip plan");
    expect(screen.getByRole("alert")).toHaveTextContent("This Note was changed somewhere else");
    expect(backend.notes.get("note-1")?.markdown).toBe("Ferry plan");
  });

  it("quits right away when no Note has unsaved text", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "All notes" });
    await quitRequested();
    expect(invokeMock).toHaveBeenLastCalledWith("quit_ready", { proceed: true });
  });

  it("marks All notes, not a Task view, as the current place in the sidebar", async () => {
    const user = await openNotes();
    await screen.findByRole("textbox", { name: "Note text" });
    expect(screen.getByRole("button", { name: "All notes" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "All tasks" })).not.toHaveAttribute("aria-current");

    await user.click(screen.getByRole("button", { name: "All tasks" }));
    expect(screen.getByRole("button", { name: "All tasks" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "All notes" })).not.toHaveAttribute("aria-current");
    expect(screen.queryByRole("textbox", { name: "Note text" })).not.toBeInTheDocument();
  });

  it("shows every supported format as formatting and saves the rest unchanged after an edit", async () => {
    const markdown = [
      "# Release plan",
      "## Scope",
      "Ship **bold**, *italic*, ~~old~~ and `code` today.",
      "1. First\n2. Second",
      "> Quoted line",
      "```\nconst ready = true;\n```",
      "- [x] Done item\n- [ ] Open item",
      "See [the docs](https://example.com/docs).",
    ].join("\n\n");
    backend.seed(markdown, "Release plan");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("Release plan"));

    expect(within(editor).getByRole("heading", { level: 1, name: "Release plan" })).toBeInTheDocument();
    expect(within(editor).getByRole("heading", { level: 2, name: "Scope" })).toBeInTheDocument();
    expect(within(editor).getByText("bold").tagName).toBe("STRONG");
    expect(within(editor).getByText("italic").tagName).toBe("EM");
    expect(within(editor).getByText("code").closest("code")).not.toBeNull();
    expect(editor.querySelector("ol")).toHaveTextContent("FirstSecond");
    expect(editor.querySelector("blockquote")).toHaveTextContent("Quoted line");
    expect(within(editor).getByText("const ready = true;").closest("code")).not.toBeNull();
    expect(within(editor).getByRole("checkbox", { name: "Done item" })).toHaveAttribute("aria-checked", "true");
    expect(within(editor).getByRole("checkbox", { name: "Open item" })).toHaveAttribute("aria-checked", "false");
    expect(within(editor).getByRole("link", { name: "the docs" })).toHaveAttribute("href", "https://example.com/docs");

    await user.click(editor);
    await user.keyboard("X");
    await eventually(() => expect(backend.notes.get("note-1")?.markdown)
      .toBe(markdown.replace("# Release plan", "# XRelease plan")));
  });

  it("turns a typed - [ ] into the same checklist item that reopening shows", async () => {
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await user.click(editor);
    // user-event reads "[[" as one literal "[".
    await user.keyboard("Packing list{Enter}- [[ ] Passport{Enter}Charger");

    expect(within(editor).getByRole("checkbox", { name: "Passport" })).toHaveAttribute("aria-checked", "false");
    expect(within(editor).getByRole("checkbox", { name: "Charger" })).toBeInTheDocument();
    await eventually(() => expect([...backend.notes.values()][0]?.markdown)
      .toBe("Packing list\n\n- [ ] Passport\n- [ ] Charger"));
  });

  it("keeps earlier bullets, honours [x], and leaves escaped brackets alone on open", async () => {
    backend.seed("- \\[ \\] Literal brackets", "[ ] Literal brackets");
    const user = await openNotes();
    const opened = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(opened).toHaveTextContent("[ ] Literal brackets"));
    expect(within(opened).queryByRole("checkbox")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "New note" }));
    const editor = screen.getByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent(""));
    await user.click(editor);
    await user.keyboard("- Milk{Enter}[[x] Eggs");

    expect(within(editor).getByRole("listitem")).toHaveTextContent("Milk");
    expect(within(editor).getByRole("checkbox", { name: "Eggs" })).toHaveAttribute("aria-checked", "true");
    await eventually(() => expect(backend.notes.get("note-2")?.markdown).toBe("- Milk\n\n- [x] Eggs"));
    expect(backend.notes.get("note-1")?.markdown).toBe("- \\[ \\] Literal brackets");
  });

  it("keeps text that only looks like Markdown as text after an edit", async () => {
    const markdown = [
      "Notes",
      "\\# Not a heading",
      "1\\. Not a list",
      "- \\[ \\] Literal brackets",
      "\\[draft\\](later) is not a link",
    ].join("\n\n");
    backend.seed(markdown, "Notes");
    const user = await openNotes();
    const editor = await screen.findByRole("textbox", { name: "Note text" });
    await waitFor(() => expect(editor).toHaveTextContent("# Not a heading"));
    expect(within(editor).queryByRole("heading")).not.toBeInTheDocument();
    expect(within(editor).queryByRole("checkbox")).not.toBeInTheDocument();
    expect(within(editor).queryByRole("link")).not.toBeInTheDocument();
    expect(editor.querySelector("ol")).toBeNull();

    await user.click(editor);
    await user.keyboard("X");
    await eventually(() => expect(backend.notes.get("note-1")?.markdown)
      .toBe(markdown.replace("Notes", "XNotes")));
  });
});
