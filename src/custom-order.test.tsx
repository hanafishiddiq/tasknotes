/**
 * Ticket 13: a person drags Tasks into a Custom Order within a work-status
 * group of one List (ADR 0010). A small in-memory core stands behind the
 * window; synthetic data only.
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { SortMode, Task, TaskList, TaskView } from "./lib/task-api";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const ROW_HEIGHT = 40;
const RANK = { very_high: 5, high: 4, medium: 3, low: 2, very_low: 1 } as const;
const GROUP = { in_progress: 0, ready: 1, completed: 2 } as const;

type Stored = Task & { position: number };
let tasks: Stored[];
let lists: TaskList[];
let sorts: Record<string, SortMode>;
let calls: { command: string; body: Record<string, any> }[];
let backend: NoteBackend;

const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

function list(name: string): TaskList {
  return { id: `list-${name}`, name, revision: 1, createdAt: 1, updatedAt: 1, removed: false };
}

function task(id: string, listId: string | null, position: number, extra: Partial<Task> = {}): Stored {
  return {
    id, title: id, description: "", status: "ready", priority: "medium", difficulty: null,
    wontDo: false, progress: 0, nextStep: "", listId, dueDate: null, tags: [], revision: 1,
    createdAt: 100 + position, updatedAt: 100, position, ...extra,
  };
}

const key = (view: TaskView) => view.kind === "list" ? `list:${view.listId}` : view.kind;
const strip = ({ position: _position, ...item }: Stored): Task => item;

function listed(body: Record<string, any>): Task[] {
  const view: TaskView = body.view ?? { kind: "all" };
  const sort: SortMode = body.sort ?? "priority";
  const shown = tasks.filter((item) => view.kind === "all"
    || (view.kind === "inbox" ? item.listId === null : item.listId === view.listId))
    .filter((item) => !body.text || item.title.includes(body.text))
    .filter((item) => !body.taskId || item.id === body.taskId);
  const second = (a: Stored, b: Stored) => sort === "custom" ? a.position - b.position
    : sort === "priority" ? RANK[b.priority] - RANK[a.priority] || b.createdAt - a.createdAt
    : b.createdAt - a.createdAt;
  return shown.sort((a, b) => GROUP[a.status] - GROUP[b.status] || second(a, b)).map(strip);
}

/** The core's move: the Task is placed among its group, seeded from the sort on screen. */
function reorder(input: Record<string, any>): Task {
  const moving = tasks.find((item) => item.id === input.id)!;
  if (moving.revision !== input.expectedRevision) throw "Revision conflict: the Task has changed; reload before saving";
  const view: TaskView = moving.listId ? { kind: "list", listId: moving.listId } : { kind: "inbox" };
  const order = listed({ view, sort: input.shownSort }).map((item) => item.id);
  const anchor = input.beforeId ?? input.afterId;
  const anchorTask = tasks.find((item) => item.id === anchor);
  if (anchor && anchorTask?.status !== moving.status) {
    throw "The Tasks around this one changed while it was moving; reload and try again";
  }
  const others = order.filter((id) => id !== moving.id);
  const at = input.beforeId ? others.indexOf(input.beforeId)
    : input.afterId ? others.indexOf(input.afterId) + 1 : 0;
  others.splice(at, 0, moving.id);
  others.forEach((id, index) => { tasks.find((item) => item.id === id)!.position = index; });
  moving.revision += 1;
  if (input.shownSort !== "custom") sorts[key(view)] = "custom";
  return strip(moving);
}

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  lists = [list("Work"), list("Home")];
  tasks = [
    task("One", "list-Work", 1, { priority: "low" }),
    task("Two", "list-Work", 2, { priority: "high" }),
    task("Three", "list-Work", 3),
    task("Doing", "list-Work", 0, { status: "in_progress", progress: 10 }),
    task("Home task", "list-Home", 0),
    task("Inbox task", null, 0),
  ];
  sorts = { "list:list-Work": "custom", "list:list-Home": "difficulty" };
  calls = [];
  backend = new NoteBackend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    calls.push({ command, body });
    switch (command) {
      case "list_lists": return { items: lists.map((item) => ({ ...item })), nextCursor: null };
      case "list_tasks": return { items: listed(body), nextCursor: null };
      case "view_sorts": return Object.entries(sorts).map(([view, sort]) => ({
        view: view.startsWith("list:") ? { kind: "list", listId: view.slice(5) } : { kind: view }, sort,
      }));
      case "set_view_sort": sorts[key(body.input.view)] = body.input.sort; return null;
      case "reorder_task": return reorder(body.input);
      case "change_task": {
        const changing = tasks.find((item) => item.id === body.input.id)!;
        Object.assign(changing, { priority: body.input.priority ?? changing.priority, revision: changing.revision + 1 });
        return strip(changing);
      }
      case "list_progress_entries":
      case "list_task_notes": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      default: return backend.handle(command, body);
    }
  });
  // No layout in the test DOM: every Task row is 40px tall, one under the other.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function box(this: HTMLElement) {
    const rows = [...(this.closest(".task-list")?.querySelectorAll("[data-drag-row]") ?? [])];
    const at = this.dataset.dragRow ? rows.indexOf(this) : -1;
    const top = at >= 0 ? at * ROW_HEIGHT : 0;
    const bottom = at >= 0 ? top + ROW_HEIGHT : 4000;
    return { top, bottom, left: 0, right: 600, width: 600, height: bottom - top, x: 0, y: top, toJSON: () => ({}) };
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const titles = () => screen.queryAllByRole("article").map((row) => row.getAttribute("aria-label"));
const sortSelect = () => screen.getByRole("combobox", { name: "Sort tasks" }) as HTMLSelectElement;
const openButton = (title: string) => screen.getByRole("button", { name: `Open ${title}` });
const lastListed = () => calls.filter((call) => call.command === "list_tasks").slice(-1)[0]?.body;
const reorders = () => calls.filter((call) => call.command === "reorder_task").map((call) => call.body.input);

async function showList(name: string) {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name }));
  await eventually(() => expect(screen.getByRole("heading", { level: 1, name })).toBeInTheDocument());
  await screen.findAllByRole("article");
  return user;
}

/** Presses a row's title, moves to `y` and lets go: a drag the way a mouse makes one. */
function drag(title: string, y: number) {
  const row = openButton(title);
  const from = [...document.querySelectorAll(".task-slot[data-drag-row]")]
    .findIndex((slot) => slot.contains(row)) * ROW_HEIGHT + ROW_HEIGHT / 2;
  fireEvent.mouseDown(row, { button: 0, clientX: 20, clientY: from });
  fireEvent.mouseMove(window, { clientX: 20, clientY: y });
  fireEvent.mouseUp(window);
  fireEvent.click(row);
}

/** The y that drops on the upper or lower half of the row at `index` on screen. */
const onRow = (index: number, half: "upper" | "lower") => index * ROW_HEIGHT + (half === "upper" ? 8 : 32);

describe("Custom Order", () => {
  it("moves a Task with Alt+Up and Alt+Down within its group, keeping focus and saying so", async () => {
    const user = await showList("Work");
    expect(sortSelect().value).toBe("custom");
    expect(titles()).toEqual(["Doing", "One", "Two", "Three"]);

    act(() => openButton("Two").focus());
    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    await eventually(() => expect(titles()).toEqual(["Doing", "Two", "One", "Three"]));
    expect(reorders()).toEqual([{ id: "Two", expectedRevision: 1, beforeId: "One", shownSort: "custom" }]);
    await eventually(() => expect(openButton("Two")).toHaveFocus());
    expect(await screen.findByText("Two moved before One")).toBeInTheDocument();

    // The next press names the new revision.
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await eventually(() => expect(titles()).toEqual(["Doing", "One", "Two", "Three"]));
    expect(reorders()[1]).toEqual({ id: "Two", expectedRevision: 2, afterId: "One", shownSort: "custom" });

    // The top of a group is as far as it goes: In Progress is another group.
    act(() => openButton("One").focus());
    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    expect(reorders()).toHaveLength(2);
    expect(openButton("Two")).toHaveAttribute("aria-keyshortcuts", "Alt+ArrowUp Alt+ArrowDown");
  });

  it("drags a Task within its group and refuses a drop in another group", async () => {
    await showList("Work");
    expect(titles()).toEqual(["Doing", "One", "Two", "Three"]);

    drag("Three", onRow(1, "upper"));
    await eventually(() => expect(titles()).toEqual(["Doing", "Three", "One", "Two"]));
    expect(reorders()).toEqual([{ id: "Three", expectedRevision: 1, beforeId: "One", shownSort: "custom" }]);

    drag("One", onRow(3, "lower"));
    await eventually(() => expect(titles()).toEqual(["Doing", "Three", "Two", "One"]));
    expect(reorders()[1]).toMatchObject({ id: "One", afterId: "Two" });

    // Onto the In Progress row: nothing moves and no status changes.
    drag("Two", onRow(0, "lower"));
    drag("Two", onRow(0, "upper"));
    expect(reorders()).toHaveLength(2);
    expect(calls.some((call) => call.command === "change_task")).toBe(false);
    expect(titles()).toEqual(["Doing", "Three", "Two", "One"]);
  });

  it("switches the List to Custom when a drag breaks its sort, starting from the order on screen", async () => {
    sorts["list:list-Work"] = "priority";
    await showList("Work");
    expect(sortSelect().value).toBe("priority");
    expect(titles()).toEqual(["Doing", "Two", "Three", "One"]);

    drag("One", onRow(1, "upper"));
    await eventually(() => expect(sortSelect().value).toBe("custom"));
    expect(reorders()).toEqual([{ id: "One", expectedRevision: 1, beforeId: "Two", shownSort: "priority" }]);
    expect(titles()).toEqual(["Doing", "One", "Two", "Three"]);
    await eventually(() => expect(lastListed()?.sort).toBe("custom"));
    expect(titles()).toEqual(["Doing", "One", "Two", "Three"]);
  });

  it("never switches to Custom when a priority changes through its control", async () => {
    sorts["list:list-Work"] = "priority";
    const user = await showList("Work");
    await user.click(openButton("One"));
    await user.selectOptions(await screen.findByRole("combobox", { name: "Priority" }), "very_high");
    await eventually(() => expect(titles()).toEqual(["Doing", "One", "Two", "Three"]));
    expect(sortSelect().value).toBe("priority");
    expect(reorders()).toEqual([]);
    expect(calls.some((call) => call.command === "set_view_sort")).toBe(false);
  });

  it("remembers each List's sort, also after a restart", async () => {
    const user = await showList("Work");
    await user.selectOptions(sortSelect(), "newest");
    expect(calls.find((call) => call.command === "set_view_sort")?.body.input)
      .toEqual({ view: { kind: "list", listId: "list-Work" }, sort: "newest" });
    await eventually(() => expect(titles()).toEqual(["Doing", "Three", "Two", "One"]));

    await user.click(screen.getByRole("button", { name: "Home" }));
    await eventually(() => expect(sortSelect().value).toBe("difficulty"));
    await user.click(screen.getByRole("button", { name: "Inbox" }));
    await eventually(() => expect(sortSelect().value).toBe("priority"));
    await user.click(screen.getByRole("button", { name: "Work" }));
    await eventually(() => expect(sortSelect().value).toBe("newest"));

    // Back to Custom: the order it had is still there.
    await user.selectOptions(sortSelect(), "custom");
    await eventually(() => expect(titles()).toEqual(["Doing", "One", "Two", "Three"]));

    cleanup();
    await showList("Work");
    expect(sortSelect().value).toBe("custom");
  });

  it("keeps drag and Custom off in All tasks, and drag off in a search or tag filter", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findAllByRole("article");
    expect([...sortSelect().options].map((option) => option.value)).not.toContain("custom");
    expect(document.querySelector(".task-slot[data-drag-row]")).toBeNull();
    act(() => openButton("Two").focus());
    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    expect(reorders()).toEqual([]);

    await user.click(screen.getByRole("button", { name: "Work" }));
    await eventually(() => expect(document.querySelector(".task-slot[data-drag-row]")).not.toBeNull());
    expect([...sortSelect().options].map((option) => option.value)).toContain("custom");

    await user.type(screen.getByRole("searchbox", { name: "Search tasks and notes" }), "T");
    await eventually(() => expect(titles()).toEqual(["Two", "Three"]));
    expect(document.querySelector(".task-slot[data-drag-row]")).toBeNull();
    act(() => openButton("Two").focus());
    await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    drag("Three", onRow(0, "upper"));
    expect(reorders()).toEqual([]);
    expect(within(screen.getByRole("article", { name: "Two" })).getByRole("button", { name: "Open Two" }))
      .not.toHaveAttribute("aria-keyshortcuts");

    // A tag filter alone turns it off too.
    await user.clear(screen.getByRole("searchbox", { name: "Search tasks and notes" }));
    await eventually(() => expect(document.querySelector(".task-slot[data-drag-row]")).not.toBeNull());
    await user.type(screen.getByRole("textbox", { name: "Filter by tag" }), "synthetic");
    await eventually(() => expect(lastListed()?.tag).toBe("synthetic"));
    await eventually(() => expect(document.querySelector(".task-slot[data-drag-row]")).toBeNull());
  });

  it("shows the core's order again when it refuses a move", async () => {
    await showList("Work");
    tasks.find((item) => item.id === "Three")!.revision = 5;
    drag("Three", onRow(1, "upper"));
    expect(await screen.findByText(/Revision conflict/)).toBeInTheDocument();
    await eventually(() => expect(titles()).toEqual(["Doing", "One", "Two", "Three"]));
  });
});
