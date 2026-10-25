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
      { input: { markdown: "Budget review", folderId: work.id } },
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

describe("pinned Notes and moving a Note", () => {
  const titlesIn = (list: HTMLElement) => within(list).getAllByRole("button")
    .map((button) => button.querySelector(".note-list-title")?.textContent);

  it("pins the open Note above the rest and unpins it, without making its autosave conflict", async () => {
    const older = backend.seed("Packing list", "Packing list");
    backend.seed("Recipes", "Recipes");
    const user = await openNotes();
    const list = screen.getByRole("list", { name: "Notes" });
    await eventually(() => expect(titlesIn(list)).toEqual(["Recipes", "Packing list"]));

    await user.click(within(list).getByText("Packing list", { exact: true }));
    await eventually(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Packing list"));
    await user.click(screen.getByRole("button", { name: "Pin note" }));
    await eventually(() => expect(titlesIn(list)).toEqual(["Packing list", "Recipes"]));
    expect(backend.commands("set_note_pinned")).toEqual([{ input: { id: older.id, pinned: true } }]);
    expect(within(list).getByText("Pinned")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Unpin note" })).toHaveAttribute("aria-pressed", "true");

    // Pinning is not an edit: the editor still saves over the revision it opened.
    await user.click(screen.getByRole("textbox", { name: "Note text" }));
    await user.keyboard("Bali ");
    await eventually(() => expect(backend.commands("update_note")).toEqual([
      { input: { id: older.id, markdown: "Bali Packing list", expectedRevision: 1 } },
    ]));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await eventually(() => expect(titlesIn(list)).toEqual(["Bali Packing list", "Recipes"]));

    await user.click(screen.getByRole("button", { name: "Unpin note" }));
    await eventually(() => expect(within(list).queryByText("Pinned")).not.toBeInTheDocument());
    expect(backend.commands("set_note_pinned")[1]).toEqual({ input: { id: older.id, pinned: false } });
  });

  it("an edited Note stays below the pinned ones", async () => {
    backend.seed("Pinned plan", "Pinned plan", { pinned: true });
    backend.seed("Scratch", "Scratch");
    const user = await openNotes();
    const list = screen.getByRole("list", { name: "Notes" });
    await eventually(() => expect(titlesIn(list)).toEqual(["Pinned plan", "Scratch"]));
    await user.click(within(list).getByText("Scratch", { exact: true }));
    await eventually(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Scratch"));
    await user.click(screen.getByRole("textbox", { name: "Note text" }));
    await user.keyboard("Old ");
    await eventually(() => expect(titlesIn(list)).toEqual(["Pinned plan", "Old Scratch"]));
  });

  it("moves the open Note to another folder", async () => {
    const work = backend.folder("Work");
    const home = backend.folder("Home");
    const receipts = backend.seed("Receipts", "Receipts", { folderId: work.id });
    const user = await openNotes();
    await user.click(await within(folderTree()).findByRole("button", { name: "Work" }));
    const list = screen.getByRole("list", { name: "Notes" });
    await eventually(() => expect(titlesIn(list)).toEqual(["Receipts"]));
    await eventually(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Receipts"));

    const picker = screen.getByRole("combobox", { name: "Folder" });
    expect(picker).toHaveValue(work.id);
    await user.selectOptions(picker, "Home");
    await eventually(() => expect(backend.commands("move_note")).toEqual([
      { input: { id: receipts.id, folderId: home.id } },
    ]));
    // Work no longer holds it; the Note stays open where it now lives.
    await eventually(() => expect(within(list).queryAllByRole("button")).toHaveLength(0));
    expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Receipts");
    expect(screen.getByRole("combobox", { name: "Folder" })).toHaveValue(home.id);

    await user.selectOptions(screen.getByRole("combobox", { name: "Folder" }), "No folder");
    await eventually(() => expect(backend.commands("move_note")[1]).toEqual({
      input: { id: receipts.id, folderId: null },
    }));
  });
});
