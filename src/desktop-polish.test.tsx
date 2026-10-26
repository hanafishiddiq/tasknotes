import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * A stand-in for the core behind the Tauri seam. Only synthetic fixtures; the
 * rules themselves are the core's and are tested in Rust.
 */
let tasks: (Task & { removedAt: number | null })[];
let lists: TaskList[];
let clock: number;

function task(n: number, fields: Partial<Task> = {}): Task & { removedAt: number | null } {
  return {
    id: `task-${n}`, title: `Synthetic task ${n}`, description: "", status: "ready",
    priority: "medium", progress: 0, nextStep: "", revision: 1, createdAt: 100 + n,
    updatedAt: 100 + n, listId: null, dueDate: null, tags: [], removedAt: null, ...fields,
  };
}

/** Lets a test decide what the system appearance and Reduce Motion report. */
const media = { dark: true, reducedMotion: false, listeners: new Set<() => void>() };
function installMatchMedia() {
  window.matchMedia = ((query: string) => ({
    get matches() {
      if (query.includes("prefers-color-scheme: dark")) return media.dark;
      if (query.includes("prefers-reduced-motion: reduce")) return media.reducedMotion;
      return false;
    },
    media: query,
    addEventListener: (_: string, listener: () => void) => media.listeners.add(listener),
    removeEventListener: (_: string, listener: () => void) => media.listeners.delete(listener),
    addListener: () => undefined,
    removeListener: () => undefined,
    onchange: null,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

beforeEach(() => {
  tasks = [];
  lists = [];
  clock = 1_000;
  media.dark = true;
  media.reducedMotion = false;
  media.listeners.clear();
  installMatchMedia();
  window.localStorage.clear();
  delete document.documentElement.dataset.theme;
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((list) => ({ ...list })), nextCursor: null };
      case "list_tasks": {
        const hidden = new Set(lists.filter((list) => list.removed).map((list) => list.id));
        const items = tasks
          .filter((item) => item.removedAt === null && !(item.listId && hidden.has(item.listId)))
          .filter((item) => !body.taskId || item.id === body.taskId)
          .filter((item) => !body.listId || item.listId === body.listId)
          .filter((item) => !body.inbox || item.listId === null)
          .filter((item) => !body.text || item.title.toLowerCase().includes(String(body.text).toLowerCase()))
          .map(({ removedAt: _, ...item }) => item);
        return { items, nextCursor: null };
      }
      case "set_task_removed": {
        const found = tasks.find((item) => item.id === body.input.id)!;
        found.removedAt = body.input.removed ? ++clock : null;
        found.revision++;
        const { removedAt, ...shown } = found;
        return { ...shown, removed: removedAt !== null };
      }
      case "set_list_removed": {
        const list = lists.find((item) => item.id === body.input.id)!;
        list.removed = body.input.removed;
        list.revision++;
        return { ...list };
      }
      case "list_removed": return { items: [], nextCursor: null };
      case "list_progress_entries": return { items: [], nextCursor: null };
      case "list_task_notes": return { items: [], nextCursor: null };
      case "list_notes": return { items: [], nextCursor: null };
      case "list_note_folders": return { items: [], nextCursor: null };
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => cleanup());

describe("Settings and appearance", () => {
  it("opens Settings from the sidebar and follows the chosen appearance", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    expect(document.documentElement.dataset.theme).toBe("dark");

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
    const appearance = screen.getByRole("radiogroup", { name: "Appearance" });
    expect(within(appearance).getByRole("radio", { name: "Dark" })).toBeChecked();

    await user.click(within(appearance).getByRole("radio", { name: "Light" }));
    expect(document.documentElement.dataset.theme).toBe("light");

    // Match system follows what macOS reports, and changes with it.
    media.dark = false;
    await user.click(within(appearance).getByRole("radio", { name: "Match system" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    media.dark = true;
    act(() => media.listeners.forEach((listener) => listener()));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(window.localStorage.getItem("tasknotes.theme")).toBe("system");
  });
});

describe("Navigation and empty states", () => {
  it("opens All tasks and reaches every place without showing a made-up Task", async () => {
    lists = [{ id: "list-1", name: "Errands", revision: 1, removed: false, createdAt: 1, updatedAt: 1 } as TaskList];
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole("heading", { name: "All tasks" })).toBeVisible();
    expect(screen.getByRole("button", { name: "All tasks" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByText("Your list is clear")).toBeVisible();

    for (const [place, heading] of [["Inbox", "Inbox"], ["Errands", "Errands"]] as const) {
      await user.click(screen.getByRole("button", { name: place }));
      expect(await screen.findByRole("heading", { name: heading })).toBeVisible();
      expect(await screen.findByText("Your list is clear")).toBeVisible();
    }
    await user.click(screen.getByRole("button", { name: "Removed" }));
    expect(await screen.findByText("Nothing in Removed")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "All notes" }));
    expect(await screen.findByRole("heading", { name: "Notes" })).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();

    expect(screen.queryAllByRole("article")).toHaveLength(0);
    expect(call.mock.calls.some(([command]) => String(command).startsWith("create"))).toBe(false);
  });
});

/**
 * jsdom has no layout. Give the scroll area a 600px window and each list item
 * a height (an open Task is taller), so the virtualizer has something to measure.
 */
function withLayout() {
  const original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight")!;
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get(this: HTMLElement) {
      if (this.classList.contains("main-scroll")) return 600;
      if (this.dataset.index !== undefined) {
        if (this.tagName === "H3") return 30;
        return this.querySelector(".task-editor") ? 400 : 50;
      }
      return 0;
    },
  });
  return () => Object.defineProperty(HTMLElement.prototype, "offsetHeight", original);
}

function scrollTasksTo(top: number) {
  const area = document.querySelector<HTMLElement>("main:not([hidden]) .main-scroll")!;
  area.scrollTop = top;
  fireEvent.scroll(area);
  return area;
}

describe("Virtualized Task rows", () => {
  let restoreLayout: () => void;
  beforeEach(() => {
    restoreLayout = withLayout();
    tasks = Array.from({ length: 300 }, (_, i) => task(i + 1));
  });
  afterEach(() => restoreLayout());

  it("renders only the rows near the visible part of a sizeable page", async () => {
    render(<App />);
    expect(await screen.findByRole("article", { name: "Synthetic task 1" })).toBeInTheDocument();
    expect(screen.getAllByRole("article").length).toBeLessThan(40);
    expect(screen.queryByRole("article", { name: "Synthetic task 300" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Ready tasks" })).toBeInTheDocument();

    await act(async () => { scrollTasksTo(30 + 50 * 199); });
    expect(await screen.findByRole("article", { name: "Synthetic task 200" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Synthetic task 1" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("article").length).toBeLessThan(40);
  });

  it("keeps the scroll position when a Task opens and closes", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    const area = await act(async () => scrollTasksTo(30 + 50 * 40));
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 42" }));
    expect(screen.getByRole("region", { name: "Task details" })).toBeInTheDocument();
    expect(area.scrollTop).toBe(30 + 50 * 40);
    await user.click(screen.getByRole("button", { name: "Close Synthetic task 42" }));
    expect(area.scrollTop).toBe(30 + 50 * 40);
    expect(screen.getByRole("button", { name: "Open Synthetic task 42" })).toHaveFocus();
  });

  it("keeps an unsaved edit when the open Task scrolls far out of view", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 3" }));
    const title = screen.getByRole("textbox", { name: "Edit title" });
    await user.clear(title);
    await user.type(title, "Draft title kept");

    await act(async () => { scrollTasksTo(30 + 50 * 250); });
    expect(await screen.findByRole("article", { name: "Synthetic task 250" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Edit title" })).toHaveValue("Draft title kept");
    await act(async () => { scrollTasksTo(0); });
    expect(screen.getByRole("textbox", { name: "Edit title" })).toHaveValue("Draft title kept");
  });

  it("keeps an unsaved edit after opening another Task, and marks the row", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 3" }));
    await user.type(screen.getByRole("textbox", { name: "Next step" }), "Call the printer");
    await user.click(screen.getByRole("button", { name: "Open Synthetic task 4" }));

    const row = screen.getByRole("article", { name: "Synthetic task 3" });
    expect(within(row).getByText("Unsaved edits")).toBeInTheDocument();
    // Far out of view, the row holding unsaved edits stays mounted.
    await act(async () => { scrollTasksTo(30 + 50 * 250); });
    expect(screen.getByRole("article", { name: "Synthetic task 3" })).toBeInTheDocument();
    await act(async () => { scrollTasksTo(0); });

    await user.click(screen.getByRole("button", { name: "Open Synthetic task 3" }));
    expect(screen.getByRole("textbox", { name: "Next step" })).toHaveValue("Call the printer");
  });
});

describe("Keyboard navigation in the Task list", () => {
  let restoreLayout: () => void;
  beforeEach(() => {
    restoreLayout = withLayout();
    tasks = [
      task(1, { status: "in_progress", progress: 40 }),
      ...Array.from({ length: 200 }, (_, i) => task(i + 2)),
    ];
  });
  afterEach(() => restoreLayout());

  it("moves between Tasks with the arrow keys, across groups and off screen", async () => {
    const user = userEvent.setup();
    render(<App />);
    (await screen.findByRole("button", { name: "Open Synthetic task 1" })).focus();

    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("button", { name: "Open Synthetic task 2" })).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(screen.getByRole("button", { name: "Open Synthetic task 1" })).toHaveFocus();
    await user.keyboard("{End}");
    expect(await screen.findByRole("button", { name: "Open Synthetic task 201" })).toHaveFocus();
    await user.keyboard("{Home}");
    expect(await screen.findByRole("button", { name: "Open Synthetic task 1" })).toHaveFocus();
  });

  it("opens with Return and closes with Escape, handing focus back to the row", async () => {
    const user = userEvent.setup();
    render(<App />);
    (await screen.findByRole("button", { name: "Open Synthetic task 2" })).focus();
    await user.keyboard("{Enter}");
    const title = screen.getByRole("textbox", { name: "Edit title" });
    await user.click(title);
    // Arrow keys inside a field stay with the field.
    await user.keyboard("{ArrowDown}");
    expect(title).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Task details" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Synthetic task 2" })).toHaveFocus();
  });
});

describe("Undo", () => {
  it("undoes moving a List to Removed with Undo or Cmd+Z", async () => {
    lists = [{ id: "list-1", name: "Errands", revision: 1, removed: false, createdAt: 1, updatedAt: 1 } as TaskList];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    await user.click(screen.getByRole("button", { name: "Remove Errands" }));
    const notice = await screen.findByRole("status", { name: "Moved to Removed" });
    expect(notice).toHaveTextContent("“Errands” moved to Removed");
    expect(screen.queryByRole("button", { name: "Errands" })).not.toBeInTheDocument();

    await user.keyboard("{Meta>}z{/Meta}");
    expect(await screen.findByRole("button", { name: "Errands" })).toBeInTheDocument();
    expect(lists[0].removed).toBe(false);
    expect(call).toHaveBeenCalledWith("set_list_removed", {
      input: { id: "list-1", expectedRevision: 2, removed: false },
    });
    expect(screen.queryByRole("status", { name: "Moved to Removed" })).not.toBeInTheDocument();
  });

  it("leaves Cmd+Z to a text field that is being typed in", async () => {
    tasks = [task(1)];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    await user.click(screen.getByRole("button", { name: "Move to Removed" }));
    await screen.findByRole("status", { name: "Moved to Removed" });
    await user.click(screen.getByRole("textbox", { name: "Task title" }));
    await user.keyboard("{Meta>}z{/Meta}");
    expect(tasks[0].removedAt).not.toBeNull();

    await user.click(screen.getByRole("heading", { name: "All tasks" }));
    await user.keyboard("{Meta>}z{/Meta}");
    expect(await screen.findByRole("article", { name: "Synthetic task 1" })).toBeInTheDocument();
    expect(tasks[0].removedAt).toBeNull();
  });
});

describe("Command palette and shortcuts", () => {
  it("opens with Cmd+K, filters commands and runs the chosen one", async () => {
    lists = [{ id: "list-1", name: "Errands", revision: 1, removed: false, createdAt: 1, updatedAt: 1 } as TaskList];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const before = screen.getByRole("button", { name: "Inbox" });
    before.focus();

    await user.keyboard("{Meta>}k{/Meta}");
    const palette = screen.getByRole("dialog", { name: "Command palette" });
    const input = within(palette).getByRole("combobox", { name: "Command" });
    expect(input).toHaveFocus();
    await user.type(input, "err");
    const options = within(palette).getAllByRole("option");
    expect(options[0]).toHaveTextContent("Errands");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Enter}");

    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "Errands" })).toBeVisible();
  });

  it("moves through results with the arrow keys and closes with Escape", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const before = screen.getByRole("button", { name: "Inbox" });
    before.focus();
    await user.keyboard("{Meta>}k{/Meta}");
    const palette = screen.getByRole("dialog", { name: "Command palette" });
    const options = within(palette).getAllByRole("option");
    await user.keyboard("{ArrowDown}");
    expect(options[1]).toHaveAttribute("aria-selected", "true");
    expect(within(palette).getByRole("combobox", { name: "Command" }))
      .toHaveAttribute("aria-activedescendant", options[1].id);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(before).toHaveFocus();
  });

  it("searches Tasks and Notes for whatever was typed", async () => {
    tasks = [task(1, { title: "Quarterly report" }), task(2, { title: "Water plants" })];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Water plants" });
    await user.keyboard("{Meta>}k{/Meta}");
    await user.type(screen.getByRole("combobox", { name: "Command" }), "report");
    await user.click(screen.getByRole("option", { name: "Search for “report”" }));
    expect(screen.getByRole("searchbox", { name: "Search tasks and notes" })).toHaveValue("report");
    await waitFor(() => expect(call).toHaveBeenCalledWith("list_tasks", expect.objectContaining({ text: "report" })));
    expect(await screen.findByRole("article", { name: "Quarterly report" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Water plants" })).not.toBeInTheDocument();
  });

  it("offers Undo, appearance and Settings in the palette", async () => {
    tasks = [task(1)];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    await user.click(screen.getByRole("button", { name: "Move to Removed" }));
    await screen.findByRole("status", { name: "Moved to Removed" });

    await user.keyboard("{Meta>}k{/Meta}");
    await user.type(screen.getByRole("combobox", { name: "Command" }), "undo");
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("article", { name: "Synthetic task 1" })).toBeInTheDocument();

    await user.keyboard("{Meta>}k{/Meta}");
    await user.type(screen.getByRole("combobox", { name: "Command" }), "light");
    await user.keyboard("{Enter}");
    expect(document.documentElement.dataset.theme).toBe("light");

    await user.keyboard("{Meta>}k{/Meta}");
    expect(screen.queryByRole("option", { name: /Undo/ })).not.toBeInTheDocument();
    await user.type(screen.getByRole("combobox", { name: "Command" }), "settings");
    await user.keyboard("{Enter}");
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
  });

  it("focuses New task with Cmd+N, search with Cmd+F and opens Settings with Cmd+,", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    await user.keyboard("{Meta>}n{/Meta}");
    expect(screen.getByRole("textbox", { name: "Task title" })).toHaveFocus();
    await user.keyboard("{Meta>}f{/Meta}");
    expect(screen.getByRole("searchbox", { name: "Search tasks and notes" })).toHaveFocus();
    await user.keyboard("{Meta>},{/Meta}");
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
    await user.keyboard("{Meta>}n{/Meta}");
    expect(screen.getByRole("heading", { name: "All tasks" })).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Task title" })).toHaveFocus();
  });
});
