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

const folderTree = () => screen.getByRole("navigation", { name: "Note folders" });

describe("nested Note Folders", () => {
  it("creates folders inside folders and remembers which are open after a restart", async () => {
    const user = await openNotes();
    await user.type(screen.getByRole("textbox", { name: "New folder name" }), "Work{Enter}");
    await eventually(() => expect(within(folderTree()).getByRole("button", { name: "Work" })).toBeInTheDocument());
    expect(backend.commands("create_note_folder")).toEqual([{ input: { name: "Work", parentId: null } }]);

    await user.click(within(folderTree()).getByRole("button", { name: "New folder in Work" }));
    await user.type(screen.getByRole("textbox", { name: "New folder name in Work" }), "Meetings{Enter}");
    // The parent opens to show its new subfolder.
    await eventually(() => expect(within(folderTree()).getByRole("button", { name: "Meetings" })).toBeInTheDocument());
    const work = [...backend.folders.values()].find((folder) => folder.name === "Work")!;
    expect(backend.commands("create_note_folder")[1]).toEqual({ input: { name: "Meetings", parentId: work.id } });
    expect(within(folderTree()).getByRole("button", { name: "Collapse Work" }))
      .toHaveAttribute("aria-expanded", "true");

    // Restart: the open folder is open again without a click.
    cleanup();
    await openNotes();
    await eventually(() => expect(within(folderTree()).getByRole("button", { name: "Meetings" })).toBeInTheDocument());

    const user2 = userEvent.setup();
    await user2.click(within(folderTree()).getByRole("button", { name: "Collapse Work" }));
    expect(within(folderTree()).queryByRole("button", { name: "Meetings" })).not.toBeInTheDocument();
    cleanup();
    await openNotes();
    await eventually(() => expect(within(folderTree()).getByRole("button", { name: "Expand Work" })).toBeInTheDocument());
    expect(within(folderTree()).queryByRole("button", { name: "Meetings" })).not.toBeInTheDocument();
  });

  it("opens a folder with only its own Notes and creates a new Note inside it", async () => {
    const work = backend.folder("Work");
    const meetings = backend.folder("Meetings", work);
    backend.seed("Loose idea", "Loose idea");
    backend.seed("Quarterly plan", "Quarterly plan", { folderId: work.id });
    backend.seed("Standup notes", "Standup notes", { folderId: meetings.id });
    const user = await openNotes();

    await user.click(await within(folderTree()).findByRole("button", { name: "Work" }));
    await eventually(() => expect(screen.getByRole("heading", { level: 1, name: "Work" })).toBeInTheDocument());
    const list = screen.getByRole("list", { name: "Notes" });
    await eventually(() => expect(within(list).getAllByRole("button").map((b) => b.textContent))
      .toEqual([expect.stringContaining("Quarterly plan")]));
    expect(within(folderTree()).getByRole("button", { name: "Work" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "All notes" })).not.toHaveAttribute("aria-current");
    expect(backend.commands("list_notes").slice(-1)[0]).toMatchObject({ folder: { kind: "folder", folderId: work.id } });

    await user.click(screen.getByRole("button", { name: "New note" }));
    await user.click(screen.getByRole("textbox", { name: "Note text" }));
    await user.keyboard("Budget review");
    await eventually(() => expect(backend.commands("create_note")).toEqual([
      { input: { markdown: "Budget review" }, folderId: work.id },
    ]));

    // All notes lists every Note again.
    await user.click(screen.getByRole("button", { name: "All notes" }));
    await eventually(() => expect(within(screen.getByRole("list", { name: "Notes" })).getAllByRole("button"))
      .toHaveLength(4));
  });

  it("renames and moves a folder, and shows the core's reason when a move is refused", async () => {
    const work = backend.folder("Work");
    backend.folder("Home");
    backend.folder("Projects", work);
    const user = await openNotes();
    await within(folderTree()).findByRole("button", { name: "Work" });

    await user.click(within(folderTree()).getByRole("button", { name: "Rename Work" }));
    const rename = screen.getByRole("textbox", { name: "Rename folder" });
    await user.clear(rename);
    await user.type(rename, "Office{Enter}");
    await eventually(() => expect(within(folderTree()).getByRole("button", { name: "Office" })).toBeInTheDocument());

    // Expand so the subfolder is a known destination, then try an impossible move.
    await user.click(within(folderTree()).getByRole("button", { name: "Expand Office" }));
    await within(folderTree()).findByRole("button", { name: "Projects" });
    backend.refuse("move_note_folder", "A folder can't be moved into itself or one of its subfolders");
    await user.click(within(folderTree()).getByRole("button", { name: "Move Office" }));
    // Destinations read as paths; the core, not the desktop, judges them.
    await user.selectOptions(screen.getByRole("combobox", { name: "Move Office to" }), "Office / Projects");
    await eventually(() => expect(screen.getByRole("alert")).toHaveTextContent(
      "A folder can't be moved into itself or one of its subfolders",
    ));
    expect([...backend.folders.values()].find((f) => f.name === "Office")?.parentId).toBeNull();

    await user.click(within(folderTree()).getByRole("button", { name: "Move Projects" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Move Projects to" }), "Top level");
    await eventually(() => expect(backend.commands("move_note_folder").slice(-1)[0]).toMatchObject({
      input: { parentId: null },
    }));
    await eventually(() => expect(within(folderTree())
      .getAllByRole("button", { name: /^(Home|Office|Projects)$/ }).map((button) => button.textContent))
      .toEqual(["Home", "Office", "Projects"]));
    // Office has nothing left to expand.
    expect(within(folderTree()).queryByRole("button", { name: /Office$/, expanded: true })).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
