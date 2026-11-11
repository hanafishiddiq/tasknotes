/**
 * Batch 3, Ticket 03 (owner decisions 2026-11-11, Q20–Q24): right-clicking a
 * Task row opens a small menu in the app's own look. Each choice is an existing
 * control's change, so the core's rules apply as they do there. A small
 * stand-in for the core sits behind the Tauri seam. Synthetic data only.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, WorkStatus } from "./lib/task-api";
import { FakeAudioContext } from "./test/fake-audio";
import { rules } from "./test/stylesheet";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

type ChangeInput = {
  id: string; expectedRevision: number; status?: WorkStatus; progress?: number; wontDo?: boolean; priority?: Task["priority"];
};

let saved: Task[];
let changes: ChangeInput[];
let removedIds: string[];

function task(id: string, fields: Partial<Task> = {}): Task {
  return {
    id, title: `Synthetic ${id}`, description: "", status: "ready", priority: "medium", difficulty: null,
    progress: 0, wontDo: false, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1,
    createdAt: 100, updatedAt: 100, ...fields,
  };
}

const GROUP = { in_progress: 0, ready: 1, completed: 2 } as const;

/** Just enough of the core's transitions for what the menu sends. */
function applyChange(item: Task, input: ChangeInput): Task {
  const next = { ...item, revision: item.revision + 1 };
  const from = item.status === "completed" ? undefined : item.status;
  if (input.priority) return { ...next, priority: input.priority };
  if (input.wontDo === true) return { ...next, status: "completed", wontDo: true, wontDoFrom: from };
  if (input.status === "completed") return { ...next, status: "completed", progress: 100, wontDo: false, completedFrom: from };
  if (input.status) return { ...next, status: input.status, wontDo: false };
  throw "Unexpected change: " + JSON.stringify(input);
}

beforeEach(() => {
  window.localStorage.clear();
  FakeAudioContext.reset();
  vi.stubGlobal("AudioContext", FakeAudioContext);
  saved = [
    task("draft", { status: "in_progress", progress: 40 }),
    task("idea"),
    task("finished", { status: "completed", progress: 100, completedFrom: "in_progress" }),
  ];
  changes = [];
  removedIds = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists":
      case "list_notes":
      case "list_task_notes":
      case "list_progress_entries":
      case "list_removed":
        return { items: [], nextCursor: null };
      case "list_tasks":
        return {
          items: saved.filter((item) => !removedIds.includes(item.id))
            .sort((a, b) => GROUP[a.status] - GROUP[b.status]),
          nextCursor: null,
        };
      case "change_task": {
        const input = body.input as ChangeInput;
        changes.push(input);
        const changed = applyChange(saved.find((item) => item.id === input.id)!, input);
        saved = saved.map((item) => item.id === input.id ? changed : item);
        return changed;
      }
      case "set_task_removed": {
        const found = saved.find((item) => item.id === body.input.id)!;
        if (body.input.removed) removedIds.push(found.id);
        found.revision += 1;
        return { ...found, removed: body.input.removed };
      }
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const row = (title: string) => screen.getByRole("article", { name: title });
const menu = () => screen.getByRole("menu", { name: /^Actions for/ });
/** A choice in the menu, whichever kind of menu item it is. */
const item = (name: string) =>
  within(menu()).queryByRole("menuitem", { name }) ?? within(menu()).getByRole("menuitemradio", { name });

async function rightClick(title: string) {
  // The row's open button, which reads Close on the open Task.
  const target = within(await screen.findByRole("article", { name: title }))
    .getByRole("button", { name: new RegExp(`^(Open|Close) ${title}$`) });
  return fireEvent.contextMenu(target, { clientX: 120, clientY: 80 });
}

describe("opening the Task menu", () => {
  it("opens on a right-click on the row, in place of the system menu, with the agreed items", async () => {
    render(<App />);
    const notCancelled = await rightClick("Synthetic idea");
    // fireEvent returns false when the handler prevented the default (the system menu).
    expect(notCancelled).toBe(false);

    expect(menu()).toHaveAccessibleName("Actions for “Synthetic idea”");
    const names = [...menu().querySelectorAll("[role^='menuitem']")].map((element) => element.textContent?.trim());
    expect(names).toEqual(["Open", "Ready", "In Progress", "Completed", "Won't do", "Priority", "Move to Removed"]);
    expect(within(menu()).getByRole("menuitemradio", { name: "Ready" })).toHaveAttribute("aria-checked", "true");
    expect(within(menu()).getByRole("menuitemradio", { name: "Completed" })).toHaveAttribute("aria-checked", "false");
    expect(item("Priority")).toHaveAttribute("aria-haspopup", "menu");
    // The first choice has focus, so the keyboard can go on from there.
    expect(item("Open")).toHaveFocus();
  });

  it("leaves the system menu inside an open Task's fields", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic idea" }));
    const details = screen.getByRole("region", { name: "Task details" });
    const field = await within(details).findByRole("textbox", { name: "Edit title" });
    expect(fireEvent.contextMenu(field)).toBe(true);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("opens from the keyboard with Shift+F10 or the menu key, and Escape puts focus back", async () => {
    const user = userEvent.setup();
    render(<App />);
    const open = await screen.findByRole("button", { name: "Open Synthetic idea" });
    open.focus();
    await user.keyboard("{Shift>}{F10}{/Shift}");
    expect(item("Open")).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(open).toHaveFocus();

    fireEvent.keyDown(open, { key: "ContextMenu" });
    expect(menu()).toBeInTheDocument();
  });
});

describe("what the menu offers", () => {
  it("dims Ready once work has started, and Won't do on a finished Task, as the status control does", async () => {
    render(<App />);
    await rightClick("Synthetic draft");
    expect(within(menu()).getByRole("menuitemradio", { name: "Ready" })).toHaveAttribute("aria-disabled", "true");
    expect(within(menu()).getByRole("menuitemradio", { name: "In Progress" })).toHaveAttribute("aria-checked", "true");
    fireEvent.keyDown(menu(), { key: "Escape" });

    await rightClick("Synthetic finished");
    const wontDo = within(menu()).getByRole("menuitemradio", { name: "Won't do" });
    expect(wontDo).toHaveAttribute("aria-disabled", "true");
    expect(wontDo).toHaveAccessibleDescription("Reopen it first: a finished Task can't be set aside");
    expect(item("Open")).not.toHaveAttribute("aria-disabled", "true");
  });

  it("says Close on the open Task, and closes it", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic idea" }));
    await rightClick("Synthetic idea");
    await user.click(item("Close"));
    expect(screen.queryByRole("region", { name: "Task details" })).toBeNull();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("opens the Task from Open", async () => {
    const user = userEvent.setup();
    render(<App />);
    await rightClick("Synthetic idea");
    await user.click(item("Open"));
    expect(within(row("Synthetic idea")).getByRole("region", { name: "Task details" })).toBeInTheDocument();
  });

  it("dims every change while the Task has unsaved edits, but still opens it", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic idea" }));
    await user.type(await within(row("Synthetic idea")).findByRole("textbox", { name: "Edit title" }), " edited");
    // The row keeps its saved title until the edit is saved.
    await rightClick("Synthetic idea");
    for (const name of ["Ready", "In Progress", "Completed", "Won't do"]) {
      expect(within(menu()).getByRole("menuitemradio", { name })).toHaveAttribute("aria-disabled", "true");
    }
    expect(item("Priority")).toHaveAttribute("aria-disabled", "true");
    expect(item("Move to Removed")).toHaveAttribute("aria-disabled", "true");
    expect(item("Close")).not.toHaveAttribute("aria-disabled", "true");
    await user.click(item("Move to Removed"));
    expect(removedIds).toEqual([]);
  });
});

describe("choosing from the menu", () => {
  it("completes a Task as the round check does: cue and Undo bar", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic draft" });
    const context = FakeAudioContext.latest();
    const before = context.oscillators.length;

    await rightClick("Synthetic draft");
    await user.click(within(menu()).getByRole("menuitemradio", { name: "Completed" }));

    await waitFor(() => expect(changes).toEqual([{ id: "draft", expectedRevision: 1, status: "completed" }]));
    expect(await screen.findByRole("status", { name: "Task completed" })).toBeInTheDocument();
    await waitFor(() => expect(context.oscillators.length).toBeGreaterThan(before));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("sets Won't do and other statuses through the status control's change", async () => {
    const user = userEvent.setup();
    render(<App />);
    await rightClick("Synthetic idea");
    await user.click(within(menu()).getByRole("menuitemradio", { name: "Won't do" }));
    await waitFor(() => expect(changes).toEqual([{ id: "idea", expectedRevision: 1, wontDo: true }]));
  });

  it("changes the priority from its submenu, by pointer or keyboard", async () => {
    const user = userEvent.setup();
    render(<App />);
    await rightClick("Synthetic idea");
    item("Priority").focus();
    await user.keyboard("{ArrowRight}");
    const levels = screen.getByRole("menu", { name: "Priority" });
    expect(within(levels).getByRole("menuitemradio", { name: "Medium" })).toHaveAttribute("aria-checked", "true");
    expect(within(levels).getByRole("menuitemradio", { name: "Medium" })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.queryByRole("menu", { name: "Priority" })).toBeNull();
    expect(item("Priority")).toHaveFocus();

    await user.hover(item("Priority"));
    await user.click(within(screen.getByRole("menu", { name: "Priority" })).getByRole("menuitemradio", { name: "High" }));
    await waitFor(() => expect(changes).toEqual([{ id: "idea", expectedRevision: 1, priority: "high" }]));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("moves a Task to Removed as the red button does, with Undo", async () => {
    const user = userEvent.setup();
    render(<App />);
    await rightClick("Synthetic idea");
    await user.click(item("Move to Removed"));
    await waitFor(() => expect(removedIds).toEqual(["idea"]));
    expect(await screen.findByRole("status", { name: "Moved to Removed" })).toBeInTheDocument();
  });
});

describe("moving through the menu and closing it", () => {
  it("moves with the arrows over the enabled items, jumps with Home and End, and chooses with Return", async () => {
    const user = userEvent.setup();
    render(<App />);
    await rightClick("Synthetic draft");
    // Ready is dimmed on a started Task: the arrow skips it.
    await user.keyboard("{ArrowDown}");
    expect(within(menu()).getByRole("menuitemradio", { name: "In Progress" })).toHaveFocus();
    await user.keyboard("{End}");
    expect(item("Move to Removed")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(item("Open")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(item("Move to Removed")).toHaveFocus();
    await user.keyboard("{Home}{Enter}");
    expect(within(row("Synthetic draft")).getByRole("region", { name: "Task details" })).toBeInTheDocument();
  });

  it("closes on a click elsewhere, Tab, scrolling, resizing or the window losing focus", async () => {
    const user = userEvent.setup();
    render(<App />);
    await rightClick("Synthetic idea");
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("menu")).toBeNull();

    await rightClick("Synthetic idea");
    await user.keyboard("{Tab}");
    expect(screen.queryByRole("menu")).toBeNull();

    for (const event of [new Event("scroll"), new Event("resize"), new Event("blur")]) {
      await rightClick("Synthetic idea");
      fireEvent(event.type === "scroll" ? document : window, event);
      expect(screen.queryByRole("menu"), event.type).toBeNull();
    }
  });

  it("stays inside the window near its edges", async () => {
    render(<App />);
    const target = within(await screen.findByRole("article", { name: "Synthetic idea" })).getByRole("button", { name: "Open Synthetic idea" });
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", { configurable: true, get: () => 220 });
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", { configurable: true, get: () => 240 });
    try {
      fireEvent.contextMenu(target, { clientX: window.innerWidth - 10, clientY: window.innerHeight - 10 });
      const sheet = menu();
      expect(parseFloat(sheet.style.left) + 220).toBeLessThanOrEqual(window.innerWidth);
      expect(parseFloat(sheet.style.top) + 240).toBeLessThanOrEqual(window.innerHeight);
    } finally {
      delete (HTMLElement.prototype as { offsetWidth?: number }).offsetWidth;
      delete (HTMLElement.prototype as { offsetHeight?: number }).offsetHeight;
    }
  });
});

describe("the Task menu's stylesheet", () => {
  const bodyOf = (selector: string) => rules.filter((rule) =>
    rule.selector.split(",").map((part) => part.replace(/\s+/g, " ").trim()).includes(selector)).map((rule) => rule.body).join("\n");

  it("is a flat sidebar-gray sheet with a hairline, and Move to Removed is Alert Red", () => {
    const sheet = bodyOf(".task-menu");
    expect(sheet).toMatch(/background: var\(--sidebar\)/);
    expect(sheet).toMatch(/border: 1px solid var\(--sep\)/);
    expect(sheet).toMatch(/border-radius: 8px/);
    expect(sheet).not.toMatch(/box-shadow/);
    expect(bodyOf(".task-menu-item--remove")).toMatch(/color: var\(--red\)/);
  });
});
