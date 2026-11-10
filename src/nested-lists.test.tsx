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
/** Sibling order is the order in this array; the tree is read parents first, as the core reads it. */
let lists: TaskList[];
let backend: NoteBackend;
let refuse: string | null;
/** Lists per page of `list_lists`; the core pages the tree in this order. */
let pageSize: number;

const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

function list(name: string, parentId: string | null = null, extra: Partial<TaskList> = {}): TaskList {
  return { id: `list-${name}`, parentId, name, revision: 1, createdAt: 1, updatedAt: 1, removed: false, ...extra };
}

function tree(): TaskList[] {
  const out: TaskList[] = [];
  const walk = (parentId: string | null) => {
    for (const item of lists.filter((l) => (l.parentId ?? null) === parentId)) {
      out.push({ ...item, hasChildren: lists.some((l) => l.parentId === item.id && !l.removed) });
      walk(item.id);
    }
  };
  walk(null);
  return out;
}

function bump(id: string, change: Partial<TaskList>): TaskList {
  const moving = lists.find((item) => item.id === id)!;
  const next = { ...moving, ...change, revision: moving.revision + 1 };
  lists = lists.filter((item) => item.id !== id);
  return next;
}

beforeAll(() => import("./components/NotesWorkspace"));

/** Where the sidebar remembers which Lists are open, as Note Folders do. */
const EXPANDED = "tasknotes.lists.expanded";

beforeEach(() => {
  window.localStorage.clear();
  // Lists start collapsed; most tests begin with the tree open.
  window.localStorage.setItem(EXPANDED, JSON.stringify(["list-Work", "list-Reports"]));
  lists = [list("Work"), list("Reports", "list-Work"), list("Weekly", "list-Reports"), list("Home"), list("Garden")];
  backend = new NoteBackend();
  refuse = null;
  pageSize = 100;
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    const input = body.input;
    switch (command) {
      case "list_lists": {
        const at = Number(body.cursor ?? 0);
        const all = tree();
        const end = at + pageSize;
        return { items: all.slice(at, end), nextCursor: end < all.length ? String(end) : null };
      }
      case "list_tasks": return { items: [], nextCursor: null };
      case "create_sub_list": {
        if (refuse) throw refuse;
        const made = list(input.name, input.parentId);
        lists.push(made);
        return { ...made };
      }
      case "quit_ready": return null;
      case "reorder_list":
      case "place_list": {
        if (refuse) throw refuse;
        const parentId = command === "place_list" ? input.parentId : lists.find((l) => l.id === input.id)!.parentId ?? null;
        const next = bump(input.id, { parentId });
        const at = input.afterId === null ? 0 : lists.findIndex((item) => item.id === input.afterId) + 1;
        lists.splice(at, 0, next);
        return { ...next };
      }
      case "move_list": {
        if (refuse) throw refuse;
        const next = bump(input.id, { parentId: input.parentId });
        lists.push(next);
        return { ...next };
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

const listsNav = () => screen.getByRole("navigation", { name: "Lists" });
const listRow = (name: string) => within(listsNav()).getByRole("button", { name });
/** Each shown List as its name, indented by two spaces per level. */
const shownTree = () => [...listsNav().querySelectorAll<HTMLElement>("[data-drag-row]")].map((row) => {
  const depth = Number.parseInt(row.style.paddingLeft || "0", 10) / 14;
  return "  ".repeat(depth) + row.querySelector(".folder-name")!.textContent;
});
const calls = (command: string) => invokeMock.mock.calls.filter(([name]) => name === command).map(([, args]) => (args as any).input);

/** Presses a row and moves to `y` from the top of the Lists, leaving the button down. */
function dragTo(row: HTMLElement, y: number) {
  fireEvent.mouseDown(row, { button: 0, clientX: 20, clientY: y + 50 });
  fireEvent.mouseMove(window, { clientX: 20, clientY: y });
}

function drop(row: HTMLElement) {
  fireEvent.mouseUp(window);
  fireEvent.click(row);
}

async function shown() {
  render(<App />);
  await screen.findByRole("button", { name: "Weekly" });
}

describe("Lists inside Lists", () => {
  it("start collapsed like Note Folders, and remember what the person opened after a restart", async () => {
    window.localStorage.clear();
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Garden" });
    expect(shownTree()).toEqual(["Work", "Home", "Garden"]);
    expect(within(listsNav()).getByRole("button", { name: "Expand Work" })).toHaveAttribute("aria-expanded", "false");

    await user.click(within(listsNav()).getByRole("button", { name: "Expand Work" }));
    await user.click(within(listsNav()).getByRole("button", { name: "Expand Reports" }));
    expect(shownTree()).toEqual(["Work", "  Reports", "    Weekly", "Home", "Garden"]);
    await user.click(within(listsNav()).getByRole("button", { name: "Collapse Reports" }));
    expect(shownTree()).toEqual(["Work", "  Reports", "Home", "Garden"]);
    // Stored as Note Folders store theirs: the ids of the open ones.
    expect(JSON.parse(window.localStorage.getItem(EXPANDED)!)).toEqual(["list-Work"]);

    cleanup();
    render(<App />);
    await screen.findByRole("button", { name: "Reports" });
    expect(shownTree()).toEqual(["Work", "  Reports", "Home", "Garden"]);
  });

  it("shows the disclosure of a List whose Lists are on the next page, and opening it loads them", async () => {
    window.localStorage.clear();
    lists = [list("Home"), list("Work"), list("Reports", "list-Work")];
    pageSize = 2;
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Work" });
    expect(shownTree()).toEqual(["Home", "Work"]);

    await user.click(within(listsNav()).getByRole("button", { name: "Expand Work" }));
    await eventually(() => expect(shownTree()).toEqual(["Home", "Work", "  Reports"]));
    expect(invokeMock.mock.calls.filter(([name]) => name === "list_lists").map(([, args]) => (args as any).cursor))
      .toEqual([null, "2"]);
  });

  it("creates a List inside another from its row, by pointer or keyboard, at the bottom there", async () => {
    const user = userEvent.setup();
    await shown();
    await user.click(within(listsNav()).getByRole("button", { name: "New List inside Work" }));
    await user.type(within(listsNav()).getByRole("textbox", { name: "New List name in Work" }), "Drafts{Enter}");
    await eventually(() => expect(calls("create_sub_list")).toEqual([{ name: "Drafts", parentId: "list-Work" }]));
    await eventually(() => expect(shownTree()).toEqual(["Work", "  Reports", "    Weekly", "  Drafts", "Home", "Garden"]));
    // The new List is the one showing, as after making a top-level List.
    expect(listRow("Drafts")).toHaveAttribute("aria-current", "page");

    // A closed List opens to show what was made inside it.
    const garden = within(listsNav()).getByRole("button", { name: "New List inside Garden" });
    garden.focus();
    await user.keyboard("{Enter}");
    const field = within(listsNav()).getByRole("textbox", { name: "New List name in Garden" });
    expect(field).toHaveFocus();
    await user.keyboard("Escape test{Escape}");
    expect(within(listsNav()).queryByRole("textbox", { name: "New List name in Garden" })).toBeNull();
    expect(garden).toHaveFocus();
    await user.keyboard("{Enter}");
    await user.keyboard("Beds{Enter}");
    await eventually(() => expect(shownTree()).toEqual(
      ["Work", "  Reports", "    Weekly", "  Drafts", "Home", "Garden", "  Beds"],
    ));
  });

  it("offers no List inside a List five levels deep, and shows the core's refusal", async () => {
    lists = [list("L1"), list("L2", "list-L1"), list("L3", "list-L2"), list("L4", "list-L3"), list("L5", "list-L4")];
    window.localStorage.setItem(EXPANDED, JSON.stringify(["list-L1", "list-L2", "list-L3", "list-L4"]));
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "L5" });
    expect(within(listsNav()).queryByRole("button", { name: "New List inside L5" })).toBeNull();

    refuse = "A List with this name already exists here";
    await user.click(within(listsNav()).getByRole("button", { name: "New List inside L4" }));
    await user.type(within(listsNav()).getByRole("textbox", { name: "New List name in L4" }), "l5{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("A List with this name already exists here");
    expect(within(listsNav()).getByRole("textbox", { name: "New List name in L4" })).toHaveValue("l5");
  });


  it("drops a List into another and out to the top level again", async () => {
    await shown();
    const garden = listRow("Garden");
    dragTo(garden, ROW_HEIGHT * 3 + 14); // the middle of Home
    expect(listRow("Home").closest("[data-drag-row]")).toHaveAttribute("data-drop-edge", "into");
    drop(garden);
    await eventually(() => expect(calls("move_list")).toEqual([{ id: "list-Garden", expectedRevision: 1, parentId: "list-Home" }]));
    await eventually(() => expect(shownTree()).toEqual(["Work", "  Reports", "    Weekly", "Home", "  Garden"]));
    expect(screen.getByText("Garden moved into Home")).toBeInTheDocument();

    // Onto the top edge of Work: first at the top level, in one write.
    const again = listRow("Garden");
    dragTo(again, 2);
    drop(again);
    await eventually(() => expect(calls("place_list")).toEqual([
      { id: "list-Garden", expectedRevision: 2, parentId: null, afterId: null },
    ]));
    await eventually(() => expect(shownTree()).toEqual(["Garden", "Work", "  Reports", "    Weekly", "Home"]));
  });

  it("carries the Lists inside along and never drops a List inside itself", async () => {
    await shown();
    const work = listRow("Work");
    dragTo(work, ROW_HEIGHT * 2 + 14); // the middle of Weekly, inside Work
    expect(listsNav().querySelector("[data-drop-edge]")).toBeNull();
    fireEvent.mouseUp(window);

    dragTo(work, ROW_HEIGHT * 4 + 27); // the bottom edge of Garden
    drop(work);
    await eventually(() => expect(calls("reorder_list")).toEqual([{ id: "list-Work", expectedRevision: 1, afterId: "list-Garden" }]));
    expect(shownTree()).toEqual(["Home", "Garden", "Work", "  Reports", "    Weekly"]);
    expect(calls("move_list")).toEqual([]);
  });

  it("moves with Alt and the arrow keys into the List above and out again", async () => {
    const user = userEvent.setup();
    await shown();
    listRow("Garden").focus();

    await user.keyboard("{Alt>}{ArrowRight}{/Alt}");
    await eventually(() => expect(calls("move_list")).toEqual([{ id: "list-Garden", expectedRevision: 1, parentId: "list-Home" }]));
    await eventually(() => expect(shownTree()).toEqual(["Work", "  Reports", "    Weekly", "Home", "  Garden"]));
    expect(listRow("Garden")).toHaveFocus();

    await user.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    await eventually(() => expect(calls("place_list")).toEqual([
      { id: "list-Garden", expectedRevision: 2, parentId: null, afterId: "list-Home" },
    ]));
    await eventually(() => expect(shownTree()).toEqual(["Work", "  Reports", "    Weekly", "Home", "Garden"]));
    expect(listRow("Garden")).toHaveFocus();
    expect(screen.getByText("Garden moved after Home")).toBeInTheDocument();

    // Weekly has no sibling above it to go into, and Up keeps it among its siblings.
    listRow("Weekly").focus();
    await user.keyboard("{Alt>}{ArrowRight}{/Alt}{Alt>}{ArrowUp}{/Alt}");
    expect(calls("move_list")).toHaveLength(1);
    expect(calls("reorder_list")).toEqual([]);
  });

  it("shows the core's refusal when a List would sit too deep, and reads the tree again", async () => {
    await shown();
    refuse = "Lists can be nested at most 5 levels deep";
    const garden = listRow("Garden");
    dragTo(garden, ROW_HEIGHT * 2 + 14); // the middle of Weekly
    drop(garden);
    await screen.findByText(refuse);
    await eventually(() => expect(shownTree()).toEqual(["Work", "  Reports", "    Weekly", "Home", "Garden"]));
  });

  it("hides the Lists inside a List in Removed, and keeps their icons and row actions otherwise", async () => {
    lists = [
      list("Work", null, { removed: true }), list("Reports", "list-Work"),
      list("Home"), list("Garden", "list-Home", { icon: "flower", color: "green" }),
    ];
    window.localStorage.setItem(EXPANDED, JSON.stringify(["list-Work", "list-Home"]));
    render(<App />);
    await screen.findByRole("button", { name: "Garden" });
    expect(shownTree()).toEqual(["Home", "  Garden"]);
    const row = listRow("Garden").closest<HTMLElement>("[data-drag-row]")!;
    expect(row.querySelector("svg")).not.toBeNull();
    for (const action of ["Icon and colour for Garden", "New List inside Garden", "Rename Garden", "Remove Garden"]) {
      expect(within(row).getByRole("button", { name: action })).toBeInTheDocument();
    }
  });
});
