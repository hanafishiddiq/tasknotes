import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

let backend: NoteBackend;

const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

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

async function openNotes() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "All notes" }));
  await screen.findByRole("list", { name: "Notes" });
  return user;
}

const noteList = () => screen.getByRole("list", { name: "Notes" });
const titles = () => within(noteList()).queryAllByRole("button")
  .map((button) => button.querySelector(".note-list-title")?.textContent);

describe("searching Notes", () => {
  it("finds titles and content in every folder, shows where the words match, and clears back", async () => {
    const work = backend.folder("Work");
    backend.seed("Quarterly plan\n\nCall the landlord about the lease", "Quarterly plan", { folderId: work.id });
    backend.seed("Groceries\n\nOat milk", "Groceries");
    const user = await openNotes();
    await user.click(await screen.findByRole("button", { name: "Work" }));
    await eventually(() => expect(titles()).toEqual(["Quarterly plan"]));

    // Searching from inside a folder still looks in every folder.
    await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "oat");
    await eventually(() => expect(titles()).toEqual(["Groceries"]));
    expect(backend.commands("list_notes").slice(-1)[0]).toMatchObject({ text: "oat", folder: { kind: "all" } });

    await user.clear(screen.getByRole("searchbox", { name: "Search notes" }));
    await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "landlord");
    await eventually(() => expect(titles()).toEqual(["Quarterly plan"]));
    const hit = within(noteList()).getByRole("button");
    expect(within(hit).getByText("landlord", { selector: "mark" })).toBeInTheDocument();
    // Opening a hit shows the whole Note.
    await user.click(hit);
    await eventually(() => expect(screen.getByRole("textbox", { name: "Note text" }))
      .toHaveTextContent("Call the landlord about the lease"));

    await user.clear(screen.getByRole("searchbox", { name: "Search notes" }));
    await eventually(() => expect(backend.commands("list_notes").slice(-1)[0]).toMatchObject({
      text: "", folder: { kind: "folder", folderId: work.id },
    }));
    await eventually(() => expect(titles()).toEqual(["Quarterly plan"]));
    expect(within(noteList()).queryByText("landlord", { selector: "mark" })).not.toBeInTheDocument();
  });

  it("says when nothing matches and lists the Notes again once the search is cleared", async () => {
    backend.seed("Groceries", "Groceries");
    const user = await openNotes();
    await eventually(() => expect(titles()).toEqual(["Groceries"]));
    await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "zebra");
    await eventually(() => expect(screen.getByText("No matching notes")).toBeInTheDocument());
    expect(titles()).toEqual([]);
    await user.clear(screen.getByRole("searchbox", { name: "Search notes" }));
    await eventually(() => expect(titles()).toEqual(["Groceries"]));
    expect(screen.queryByText("No matching notes")).not.toBeInTheDocument();
  });

  it("moves between Notes in the list with the arrow keys", async () => {
    backend.seed("Third", "Third");
    backend.seed("Second", "Second");
    backend.seed("First", "First");
    const user = await openNotes();
    await eventually(() => expect(titles()).toEqual(["First", "Second", "Third"]));
    const [first, second, third] = within(noteList()).getAllByRole("button");
    first.focus();
    await user.keyboard("{ArrowDown}");
    expect(second).toHaveFocus();
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(third).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(second).toHaveFocus();
    await user.keyboard("{Enter}");
    await eventually(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Second"));
  });
});
