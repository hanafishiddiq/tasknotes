import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { TaskList } from "./lib/task-api";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const ROW_HEIGHT = 28;
let lists: TaskList[];
let backend: NoteBackend;
let refuseReorder: string | null;

const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

function list(name: string): TaskList {
  return { id: `list-${name}`, name, revision: 1, createdAt: 1, updatedAt: 1, removed: false };
}

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  lists = ["Alpha", "Bravo", "Charlie"].map(list);
  backend = new NoteBackend();
  refuseReorder = null;
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((item) => ({ ...item })), nextCursor: null };
      case "list_tasks": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      case "create_list": {
        const made = list(body.input.name);
        lists = [...lists, made];
        return { ...made };
      }
      case "reorder_list": {
        if (refuseReorder) throw refuseReorder;
        const moving = lists.find((item) => item.id === body.input.id)!;
        const others = lists.filter((item) => item !== moving);
        const at = body.input.afterId === null ? 0 : others.findIndex((item) => item.id === body.input.afterId) + 1;
        others.splice(at, 0, { ...moving, revision: moving.revision + 1 });
        lists = others;
        return { ...others[at] };
      }
      default: return backend.handle(command, body);
    }
  });
  // No layout in the test DOM: every row is 28px tall, one under the other.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function box(this: HTMLElement) {
    const rows = [...(this.closest("nav")?.querySelectorAll("[data-drag-row]") ?? [])];
    const at = this.dataset.dragRow ? rows.indexOf(this) : -1;
    const top = at >= 0 ? at * ROW_HEIGHT : 0;
    const bottom = at >= 0 ? top + ROW_HEIGHT : 4000;
    return { top, bottom, left: 0, right: 200, width: 200, height: bottom - top, x: 0, y: top, toJSON: () => ({}) };
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** Presses a row and moves to `y` (from the top of its level), leaving the button down. */
function dragTo(row: HTMLElement, y: number, x = 20) {
  fireEvent.mouseDown(row, { button: 0, clientX: 20, clientY: y + 50 });
  fireEvent.mouseMove(window, { clientX: x, clientY: y });
}

function drop(row: HTMLElement) {
  fireEvent.mouseUp(window);
  fireEvent.click(row);
}

const listsNav = () => screen.getByRole("navigation", { name: "Lists" });
const listRow = (name: string) => within(listsNav()).getByRole("button", { name });
const listOrder = () => [...listsNav().querySelectorAll("[data-list-id] .folder-name")].map((row) => row.textContent);
const reorders = () => invokeMock.mock.calls.filter(([command]) => command === "reorder_list").map(([, args]) => (args as any).input);

describe("reordering Lists", () => {
  it("drags a List between two others, saves it and does not open it", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "Charlie" });
    const charlie = listRow("Charlie");

    dragTo(charlie, ROW_HEIGHT * 1 + 2); // the top edge of Bravo
    expect(charlie.closest("[data-drag-row]")).toHaveAttribute("data-dragging");
    expect(listRow("Bravo").closest("[data-drag-row]")).toHaveAttribute("data-drop-edge", "before");
    drop(charlie);

    expect(listOrder()).toEqual(["Alpha", "Charlie", "Bravo"]);
    await eventually(() => expect(reorders()).toEqual([{ id: "list-Charlie", expectedRevision: 1, afterId: "list-Alpha" }]));
    expect(document.body).not.toHaveClass("row-dragging");
    expect(listRow("Charlie")).not.toHaveAttribute("aria-current");
    expect(listRow("Alpha").closest("[data-drag-row]")).not.toHaveAttribute("data-drop-edge");

    // Reopening reads what the core kept.
    cleanup();
    render(<App />);
    await screen.findByRole("button", { name: "Charlie" });
    expect(listOrder()).toEqual(["Alpha", "Charlie", "Bravo"]);
  });

  it("drags to the very top and below the last row", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "Charlie" });
    const bravo = listRow("Bravo");
    dragTo(bravo, 2);
    drop(bravo);
    await eventually(() => expect(reorders().slice(-1)[0]).toMatchObject({ id: "list-Bravo", afterId: null }));
    expect(listOrder()).toEqual(["Bravo", "Alpha", "Charlie"]);

    const again = listRow("Bravo");
    dragTo(again, ROW_HEIGHT * 3 + 40);
    drop(again);
    await eventually(() => expect(reorders().slice(-1)[0]).toMatchObject({ id: "list-Bravo", afterId: "list-Charlie" }));
    expect(listOrder()).toEqual(["Alpha", "Charlie", "Bravo"]);
  });

  it("treats a press without travel as a click, and leaves the order alone", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Bravo" });
    await user.click(listRow("Bravo"));
    expect(listRow("Bravo")).toHaveAttribute("aria-current", "page");
    expect(reorders()).toEqual([]);
  });

  it("cancels on Escape, on leaving the sidebar, and on landing where it already is", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "Bravo" });
    const bravo = listRow("Bravo");

    dragTo(bravo, 2);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(document.body).not.toHaveClass("row-dragging");
    fireEvent.mouseMove(window, { clientX: 20, clientY: 40 });
    // Releasing on the pressed row after Escape does not open it either.
    drop(bravo);
    expect(bravo).not.toHaveAttribute("aria-current");

    dragTo(bravo, 2, 900);
    expect(listsNav().querySelector("[data-drop-edge]")).toBeNull();
    fireEvent.mouseUp(window);

    dragTo(bravo, ROW_HEIGHT + 3); // over itself
    fireEvent.mouseUp(window);
    dragTo(bravo, ROW_HEIGHT * 2 + 3); // the top edge of the next one: where it is
    fireEvent.mouseUp(window);

    expect(reorders()).toEqual([]);
    expect(listOrder()).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("moves with Alt and the arrow keys, keeps focus and says what happened", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Alpha" });
    listRow("Alpha").focus();

    await user.keyboard("{Alt>}{ArrowUp}{/Alt}"); // already first
    expect(reorders()).toEqual([]);

    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await eventually(() => expect(reorders()).toEqual([{ id: "list-Alpha", expectedRevision: 1, afterId: "list-Bravo" }]));
    await eventually(() => expect(listOrder()).toEqual(["Bravo", "Alpha", "Charlie"]));
    expect(listRow("Alpha")).toHaveFocus();
    expect(screen.getByText("Alpha moved after Bravo")).toBeInTheDocument();

    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    await eventually(() => expect(reorders().slice(-1)[0]).toEqual({ id: "list-Alpha", expectedRevision: 2, afterId: null }));
    await eventually(() => expect(listOrder()).toEqual(["Alpha", "Bravo", "Charlie"]));
    expect(listRow("Alpha")).toHaveFocus();
    expect(screen.getByText("Alpha moved to the top")).toBeInTheDocument();
  });

  it("takes one move at a time, so a quick second press never conflicts with the first", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Alpha" });
    let release = () => {};
    const handle = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "reorder_list") await new Promise<void>((resolve) => { release = resolve; });
      return handle(command, args);
    });
    listRow("Alpha").focus();

    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    expect(reorders()).toHaveLength(1);
    release();
    await eventually(() => expect(screen.getByText("Alpha moved after Bravo")).toBeInTheDocument());

    // Once saved, the next press carries the new revision.
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await eventually(() => expect(reorders()).toHaveLength(2));
    expect(reorders()[1]).toEqual({ id: "list-Alpha", expectedRevision: 2, afterId: "list-Charlie" });
    release();
    await eventually(() => expect(screen.getByText("Alpha moved after Charlie")).toBeInTheDocument());
    expect(screen.queryByText(/changed elsewhere/)).not.toBeInTheDocument();
  });

  it("does not announce a move the core refused, and leaves focus alone while busy", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Alpha" });
    refuseReorder = "This List changed elsewhere; reload before saving";
    listRow("Alpha").focus();
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await eventually(() => expect(screen.getByText(refuseReorder!)).toBeInTheDocument());
    expect(screen.queryByText(/moved after/)).not.toBeInTheDocument();
    await eventually(() => expect(listOrder()).toEqual(["Alpha", "Bravo", "Charlie"]));

    // Refused keeps nothing pending: a later change of the Lists does not pull focus back.
    const field = screen.getByRole("textbox", { name: "New list name" });
    field.focus();
    await user.type(field, "E{Enter}");
    await eventually(() => expect(within(listsNav()).getByRole("button", { name: "E" })).toBeInTheDocument());
    expect(listRow("Alpha")).not.toHaveFocus();
  });

  it("keeps a new List in view when it lands past the first page", async () => {
    const user = userEvent.setup();
    const handle = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation(async (command, args) => {
      // The core has more pages: its first page holds only what is already there.
      if (command === "list_lists") return { items: ["Alpha", "Bravo"].map(list), nextCursor: "more" };
      if (command === "create_list") return { ...list("Far below"), name: (args as any).input.name };
      return handle(command, args);
    });
    render(<App />);
    await screen.findByRole("button", { name: "Bravo" });
    await user.type(screen.getByRole("textbox", { name: "New list name" }), "Far below{Enter}");
    await eventually(() => expect(listOrder()).toEqual(["Alpha", "Bravo", "Far below"]));
    expect(screen.getByRole("heading", { name: "Far below" })).toBeInTheDocument();
  });

  it("shows the core's refusal and what is true now when someone else changed the List", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "Charlie" });
    refuseReorder = "This List changed elsewhere; reload before saving";
    const charlie = listRow("Charlie");
    dragTo(charlie, 2);
    drop(charlie);
    expect(listOrder()).toEqual(["Charlie", "Alpha", "Bravo"]);

    await eventually(() => expect(screen.getByText(refuseReorder!)).toBeInTheDocument());
    await eventually(() => expect(listOrder()).toEqual(["Alpha", "Bravo", "Charlie"]));
  });
});

describe("reordering Note Folders", () => {
  const tree = () => screen.getByRole("list", { name: "Folders" });
  const folderRow = (name: string) => within(tree()).getByRole("button", { name });
  const shown = () => [...tree().querySelectorAll("[data-folder-id]")].map((row) => row.textContent);
  const placed = () => backend.commands("place_note_folder").map((call) => (call as any).input);
  const moved = () => backend.commands("move_note_folder").map((call) => (call as any).input);

  async function openWith(names: string[]) {
    const folders = names.map((name) => backend.folder(name));
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    await screen.findByRole("button", { name: names[0] });
    return { user, folders };
  }

  /** The fake's rows were added after the first load; reading them again shows the disclosure. */
  async function reopenWithWorkOpen(user: ReturnType<typeof userEvent.setup>) {
    cleanup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    await user.click(await within(await screen.findByRole("list", { name: "Folders" })).findByRole("button", { name: "Expand Work" }));
  }

  it("drags a folder between its siblings", async () => {
    const { folders } = await openWith(["Work", "Home", "Trips"]);
    const trips = folderRow("Trips");
    dragTo(trips, ROW_HEIGHT + 2);
    expect(folderRow("Home").closest("[data-drag-row]")).toHaveAttribute("data-drop-edge", "before");
    drop(trips);
    await eventually(() => expect(shown()).toEqual(["Work", "Trips", "Home"]));
    expect(placed()).toEqual([{ id: folders[2].id, expectedRevision: 1, parentId: null, afterId: folders[0].id }]);
  });

  it("drops a folder inside another, which opens to show it at the bottom", async () => {
    const { folders } = await openWith(["Work", "Home"]);
    backend.folder("Earlier", folders[0]);
    const home = folderRow("Home");
    dragTo(home, ROW_HEIGHT / 2);
    expect(backend.commands("place_note_folder")).toEqual([]);
    expect(folderRow("Work").closest("[data-drag-row]")).toHaveAttribute("data-drop-edge", "into");
    drop(home);
    await eventually(() => expect(shown()).toEqual(["Work", "Earlier", "Home"]));
    expect(moved()).toEqual([{ id: folders[1].id, expectedRevision: 1, parentId: folders[0].id }]);
    expect(folderRow("Work").closest("li")!.querySelector("ul")).toBeInTheDocument();
  });

  it("drags a subfolder out to the top level in one step, and below the tree to the end", async () => {
    const { folders, user } = await openWith(["Work", "Home"]);
    const inner = backend.folder("Inner", folders[0]);
    backend.folder("Second", folders[0]);
    await reopenWithWorkOpen(user);
    await eventually(() => expect(shown()).toEqual(["Work", "Inner", "Second", "Home"]));

    // Top edge of Home, which is the fourth row and on the top level.
    const second = folderRow("Second");
    dragTo(second, ROW_HEIGHT * 3 + 2);
    drop(second);
    await eventually(() => expect(placed().slice(-1)[0]).toMatchObject({ parentId: null, afterId: folders[0].id }));
    // Work stays open above it; Second is on the top level now.
    await eventually(() => expect(shown()).toEqual(["Work", "Inner", "Second", "Home"]));

    const innerRow = folderRow("Inner");
    dragTo(innerRow, ROW_HEIGHT * 10);
    expect(tree()).toHaveAttribute("data-drop-end");
    drop(innerRow);
    await eventually(() => expect(placed().slice(-1)[0]).toMatchObject({ id: inner.id, parentId: null, afterId: folders[1].id }));
  });

  it("lands first among an open folder's subfolders from just below its row", async () => {
    const { folders, user } = await openWith(["Work", "Home"]);
    backend.folder("Inner", folders[0]);
    await reopenWithWorkOpen(user);
    await eventually(() => expect(shown()).toEqual(["Work", "Inner", "Home"]));

    const home = folderRow("Home");
    dragTo(home, ROW_HEIGHT - 2);
    drop(home);
    await eventually(() => expect(placed().slice(-1)[0]).toMatchObject({ id: folders[1].id, parentId: folders[0].id, afterId: null }));
  });

  it("never offers a folder's own subfolders as a place to land", async () => {
    const { folders, user } = await openWith(["Work", "Home"]);
    backend.folder("Inner", folders[0]);
    await reopenWithWorkOpen(user);
    await eventually(() => expect(shown()).toEqual(["Work", "Inner", "Home"]));

    const work = folderRow("Work");
    dragTo(work, ROW_HEIGHT + ROW_HEIGHT / 2); // the middle of Inner
    expect(tree().querySelector("[data-drop-edge]")).toBeNull();
    drop(work);
    expect(placed()).toEqual([]);
    expect(moved()).toEqual([]);
  });

  it("moves with Alt and the arrows: up, down, into the folder above and back out", async () => {
    const { folders, user } = await openWith(["Work", "Home", "Trips"]);
    folderRow("Home").focus();

    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await eventually(() => expect(shown()).toEqual(["Work", "Trips", "Home"]));
    expect(folderRow("Home")).toHaveFocus();
    expect(screen.getByText("Home moved after Trips")).toBeInTheDocument();

    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    await eventually(() => expect(shown()).toEqual(["Work", "Home", "Trips"]));
    expect(folderRow("Home")).toHaveFocus();

    await user.keyboard("{Alt>}{ArrowRight}{/Alt}");
    await eventually(() => expect(shown()).toEqual(["Work", "Home", "Trips"]));
    await eventually(() => expect(moved()).toEqual([{ id: folders[1].id, expectedRevision: 3, parentId: folders[0].id }]));
    await eventually(() => expect(folderRow("Home")).toHaveFocus());
    expect(screen.getByText("Home moved into Work")).toBeInTheDocument();

    await user.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    await eventually(() => expect(placed().slice(-1)[0]).toMatchObject({ id: folders[1].id, parentId: null, afterId: folders[0].id }));
    await eventually(() => expect(folderRow("Home")).toHaveFocus());
  });

  it("keeps loaded pages after a move, and a folder that lands past them in view", async () => {
    const names = Array.from({ length: 55 }, (_, n) => `Folder ${n}`);
    const { user } = await openWith(names);
    expect(shown()).not.toContain("Folder 54");
    await user.click(within(screen.getByRole("navigation", { name: "Note folders" })).getByRole("button", { name: "Load more folders" }));
    await eventually(() => expect(shown()).toContain("Folder 54"));

    const last = folderRow("Folder 54");
    dragTo(last, 2);
    drop(last);
    await eventually(() => expect(shown()[0]).toBe("Folder 54"));
    expect(shown()).toHaveLength(55);

    cleanup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    await screen.findByRole("button", { name: "Folder 54" });
    expect(shown()).not.toContain("Folder 53");
    await user.type(screen.getByRole("textbox", { name: "New folder name" }), "Newest{Enter}");
    await eventually(() => expect(shown()[shown().length - 1]).toBe("Newest"));
  });

  it("shows the core's refusal, then the tree as it now is, so the next try can work", async () => {
    const { folders } = await openWith(["Work", "Home"]);
    backend.refuse("place_note_folder", "This folder changed elsewhere; reload before saving");
    const home = folderRow("Home");
    dragTo(home, 2);
    drop(home);
    await eventually(() => expect(screen.getByRole("alert")).toHaveTextContent("This folder changed elsewhere"));
    expect(shown()).toEqual(["Work", "Home"]);
    expect(folders).toHaveLength(2);
  });
});
