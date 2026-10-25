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
const folderTree = () => screen.getByRole("navigation", { name: "Note folders" });

describe("Removed Notes and folders", () => {
  it("moves the open Note to Removed, opens the next one, and restores it from Removed", async () => {
    backend.seed("# Older note", "Older note");
    const packing = backend.seed("# Packing list", "Packing list");
    const user = await openNotes();
    await eventually(() => expect(within(noteList()).getByRole("button", { name: /Packing list/ }))
      .toHaveAttribute("aria-current", "true"));

    await user.click(screen.getByRole("button", { name: "Move note to Removed" }));
    await eventually(() => expect(backend.commands("set_note_removed")).toEqual([
      { input: { id: packing.id, expectedRevision: 1, removed: true } },
    ]));
    await eventually(() => expect(within(noteList()).queryByRole("button", { name: /Packing list/ })).not.toBeInTheDocument());
    await eventually(() => expect(within(noteList()).getByRole("button", { name: /Older note/ }))
      .toHaveAttribute("aria-current", "true"));
    expect(backend.notes.has(packing.id)).toBe(true);

    await user.click(screen.getByRole("button", { name: "Removed" }));
    const view = await screen.findByRole("region", { name: "Removed items" });
    await user.click(await within(view).findByRole("button", { name: "Restore Packing list" }));
    await eventually(() => expect(within(view).getByText("Nothing in Removed")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "All notes" }));
    await eventually(() => expect(within(noteList()).getByRole("button", { name: /Packing list/ })).toBeInTheDocument());
  });

  it("moves a folder with its Notes to Removed and leaves the folder's view", async () => {
    const work = backend.folder("Work");
    backend.seed("# Loose note", "Loose note");
    backend.seed("# Budget", "Budget", { folderId: work.id });
    const user = await openNotes();
    await user.click(await within(folderTree()).findByRole("button", { name: "Work" }));
    await eventually(() => expect(screen.getByRole("heading", { name: "Work" })).toBeInTheDocument());

    await user.click(within(folderTree()).getByRole("button", { name: "Remove Work" }));
    await eventually(() => expect(backend.commands("set_note_folder_removed")).toEqual([
      { input: { id: work.id, expectedRevision: 1, removed: true } },
    ]));
    await eventually(() => expect(within(folderTree()).queryByRole("button", { name: "Work" })).not.toBeInTheDocument());
    // The folder's view closes; its Notes are hidden with it.
    await eventually(() => expect(screen.getByRole("heading", { name: "Notes" })).toBeInTheDocument());
    await eventually(() => expect(within(noteList()).getByRole("button", { name: /Loose note/ })).toBeInTheDocument());
    expect(within(noteList()).queryByRole("button", { name: /Budget/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Removed" }));
    const view = await screen.findByRole("region", { name: "Removed items" });
    await user.click(await within(view).findByRole("button", { name: "Restore Work" }));
    await eventually(() => expect(within(folderTree()).getByRole("button", { name: "Work" })).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "All notes" }));
    await eventually(() => expect(within(noteList()).getByRole("button", { name: /Budget/ })).toBeInTheDocument());
  });
});
