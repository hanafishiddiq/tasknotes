import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * Batch 3, Ticket 05: the number beside each sidebar folder is that place's
 * unfinished Tasks as the core counts them (Q45), shown where Settings says
 * (Q48). The rule itself is the core's (`unfinished_counts`, tested in Rust);
 * this fake answers it from its own Tasks. Synthetic data only.
 */
let tasks: Task[];
const home: TaskList = { id: "list-home", name: "Synthetic home", revision: 1, removed: false, createdAt: 1, updatedAt: 1 } as TaskList;
const empty: TaskList = { id: "list-empty", name: "Synthetic empty", revision: 1, removed: false, createdAt: 1, updatedAt: 1 } as TaskList;

function task(n: number, fields: Partial<Task> = {}): Task {
  return {
    id: `task-${n}`, title: `Synthetic task ${n}`, description: "", status: "ready", priority: "medium",
    difficulty: null, wontDo: false, progress: 0, nextStep: "", listId: null, dueDate: null, tags: [],
    revision: 1, createdAt: 100 + n, updatedAt: 100 + n, ...fields,
  };
}

const countsOf = (all: Task[]) => {
  const unfinished = all.filter((item) => item.status !== "completed");
  const lists: Record<string, number> = {};
  for (const item of unfinished) if (item.listId) lists[item.listId] = (lists[item.listId] ?? 0) + 1;
  return { all: unfinished.length, inbox: unfinished.filter((item) => !item.listId).length, lists };
};

beforeEach(() => {
  tasks = [
    task(1), task(2, { status: "in_progress", progress: 30 }), task(3, { status: "completed", progress: 100 }),
    task(4, { listId: home.id }), task(5, { listId: home.id }), task(6, { listId: home.id }),
  ];
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: [home, empty], nextCursor: null };
      case "list_tasks": {
        const view = body.view as { kind: string; listId?: string } | undefined;
        const items = tasks.filter((item) => !view || view.kind === "all"
          || (view.kind === "inbox" ? !item.listId : item.listId === view.listId));
        return { items: items.map((item) => ({ ...item })), nextCursor: null };
      }
      case "unfinished_counts": return countsOf(tasks);
      case "create_task": {
        const made = task(100 + tasks.length, { title: body.input.title.trim() });
        tasks.push(made);
        return made;
      }
      case "list_progress_entries":
      case "list_task_notes":
      case "list_removed":
        return { items: [], nextCursor: null };
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(cleanup);

const folder = (name: string) => screen.getByRole("button", { name });
const shown = (name: string) => folder(name).querySelector(".folder-count [aria-hidden]")?.textContent ?? null;

async function openApp() {
  render(<App />);
  await screen.findByRole("article", { name: "Synthetic task 1" });
  await waitFor(() => expect(shown("All tasks")).not.toBeNull());
}

describe("Sidebar counts of unfinished Tasks (batch 3, Ticket 05)", () => {
  it("shows every folder's Ready and In Progress Tasks, and nothing where there are none", async () => {
    await openApp();
    expect(shown("All tasks")).toBe("5");
    expect(shown("Inbox")).toBe("2");
    expect(shown("Synthetic home")).toBe("3");
    expect(shown("Synthetic empty")).toBeNull();
    // A screen reader hears it after the folder's name.
    expect(folder("Inbox")).toHaveAccessibleDescription("2 unfinished");
    expect(folder("Synthetic empty")).not.toHaveAccessibleDescription();
  });

  it("shows them only on the open folder, or not at all, as Settings says, and keeps the choice", async () => {
    const user = userEvent.setup();
    await openApp();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const choices = screen.getByRole("radiogroup", { name: "Task counts in the sidebar" });
    expect(within(choices).getByRole("radio", { name: "Every folder" })).toBeChecked();

    await user.click(within(choices).getByRole("radio", { name: "Open folder only" }));
    // Settings is open, so no folder is: none shows a number.
    expect(shown("All tasks")).toBeNull();
    await user.click(folder("Synthetic home"));
    await screen.findByRole("article", { name: "Synthetic task 4" });
    expect(shown("Synthetic home")).toBe("3");
    expect(shown("All tasks")).toBeNull();
    expect(shown("Inbox")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("radio", { name: "None" }));
    expect(screen.getByRole("navigation", { name: "Workspace" }).querySelector(".folder-count")).toBeNull();
    expect(screen.getByRole("navigation", { name: "Lists" }).querySelector(".folder-count")).toBeNull();

    // The choice is this window's, and stays for the next launch.
    cleanup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("radio", { name: "None" })).toBeChecked();
  });

  it("reads the numbers again after a Task changes here", async () => {
    const user = userEvent.setup();
    await openApp();
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Synthetic new errand");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await screen.findByRole("article", { name: "Synthetic new errand" });
    await waitFor(() => expect(shown("All tasks")).toBe("6"));
    expect(shown("Inbox")).toBe("3");
  });
});
