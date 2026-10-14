import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { Task, TaskPage } from "./lib/task-api";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const nativeInvoke = vi.mocked(invoke);

const existing: Task = {
  id: "initial", title: "Existing Task", description: "",
  status: "ready", priority: "medium", progress: 0, nextStep: "",
  listId: null, dueDate: null, tags: [], revision: 1,
  createdAt: 100, updatedAt: 100,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((callback) => { resolve = callback; });
  return { promise, resolve };
}

afterEach(() => {
  cleanup();
  nativeInvoke.mockReset();
  vi.useRealTimers();
});

describe("independent reviewer regression evidence", () => {
  it("R1: a late Next-page reply cannot hide a successfully created Task", async () => {
    const user = userEvent.setup();
    const laterPage = deferred<TaskPage>();
    const saved: Task[] = [{ ...existing }];
    let pagingStarted = false;
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command === "list_tasks") {
        const request = args as { cursor: string | null };
        if (request.cursor === "next") {
          pagingStarted = true;
          return laterPage.promise;
        }
        return { items: [...saved], nextCursor: saved.length === 1 ? "next" : null };
      }
      if (command === "create_task") {
        const input = (args as { input: { title: string } }).input;
        const task = { ...existing, id: "created", title: input.title, createdAt: 200 };
        saved.unshift(task);
        return task;
      }
      throw new Error("Unexpected native call: " + command);
    });
    render(<App />);
    expect(await screen.findByRole("article", { name: "Existing Task" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(pagingStarted).toBe(true));

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "New Task");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    expect(await screen.findByRole("article", { name: "New Task" })).toBeInTheDocument();

    await act(async () => {
      laterPage.resolve({
        items: [{ ...existing, id: "stale-page", title: "Old Page Task" }],
        nextCursor: null,
      });
      await laterPage.promise;
    });
    expect(screen.getByRole("article", { name: "New Task" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Old Page Task" })).not.toBeInTheDocument();
    expect(saved.some((task) => task.title === "New Task")).toBe(true);
  });
  it("R2: a Task created while the sort changes uses the current sort order", async () => {
    const user = userEvent.setup();
    const high = { ...existing, id: "high", title: "Priority Task", priority: "very_high" as const, createdAt: 100 };
    const recent = { ...existing, id: "recent", title: "Recent Task", priority: "low" as const, createdAt: 300 };
    const newTask = { ...existing, id: "created", title: "New Capture", priority: "medium" as const, createdAt: 400 };
    const saved: Task[] = [high, recent];
    const pendingCreate = deferred<Task>();
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command === "list_tasks") {
        const sort = (args as { sort?: string }).sort ?? "priority";
        const items = [...saved].sort(sort === "newest"
          ? (a, b) => b.createdAt - a.createdAt
          : (a, b) => {
              const ranks: Record<string, number> = { very_high: 5, high: 4, medium: 3, low: 2, very_low: 1 };
              return ranks[b.priority] - ranks[a.priority];
            });
        return { items, nextCursor: null };
      }
      if (command === "create_task") return pendingCreate.promise;
      throw new Error("Unexpected native command: " + command);
    });
    render(<App />);
    await screen.findByRole("article", { name: "Priority Task" });
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "New Capture");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith("create_task", {
      input: { title: "New Capture", description: "" },
    }));

    await user.selectOptions(screen.getByRole("combobox", { name: "Sort tasks" }), "newest");
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith("list_tasks", {
      limit: 50, cursor: null, sort: "newest",
    }));
    saved.push(newTask);
    await act(async () => {
      pendingCreate.resolve(newTask);
      await pendingCreate.promise;
    });
    expect(await screen.findByRole("article", { name: "New Capture" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Sort tasks" })).toHaveValue("newest");
    expect(screen.getAllByRole("article").map((row) => row.getAttribute("aria-label")))
      .toEqual(["New Capture", "Recent Task", "Priority Task"]);
  });


  it("R2b: inverse completion of old sort fetch and new create refresh keeps current rows", async () => {
    const user = userEvent.setup();
    const old = { ...existing, id: "old", title: "Older Task", createdAt: 100 };
    const newest = { ...existing, id: "recent", title: "Recent Task", createdAt: 300 };
    const created = { ...existing, id: "new", title: "New Task", createdAt: 400 };
    const saved: Task[] = [old, newest];
    const delayedCreate = deferred<Task>();
    const staleSort = deferred<TaskPage>();
    let newestCalls = 0;
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command === "list_tasks") {
        const requested = (args as { sort?: string }).sort ?? "priority";
        if (requested === "newest") {
          newestCalls++;
          if (newestCalls === 1) return staleSort.promise;
          return { items: [...saved].sort((a,b) => b.createdAt-a.createdAt), nextCursor: null };
        }
        return { items: [...saved], nextCursor: null };
      }
      if (command === "create_task") return delayedCreate.promise;
      throw new Error("Unexpected native command: " + command);
    });
    render(<App />);
    await screen.findByRole("article", { name: "Older Task" });
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "New Task");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Sort tasks" }), "newest");
    await waitFor(() => expect(newestCalls).toBe(1));

    saved.push(created);
    await act(async () => {
      delayedCreate.resolve(created);
      await delayedCreate.promise;
    });
    // A completed newer query should end the loading state, even while the
    // older sort request has not yet returned.
    expect(await screen.findByRole("article", { name: "New Task" })).toBeInTheDocument();
    expect(screen.getAllByRole("article").map((row) => row.getAttribute("aria-label")))
      .toEqual(["New Task", "Recent Task", "Older Task"]);

    await act(async () => {
      staleSort.resolve({ items: [newest, old], nextCursor: null });
      await staleSort.promise;
    });
    expect(screen.getByRole("article", { name: "New Task" })).toBeInTheDocument();
    expect(screen.getAllByRole("article").map((row) => row.getAttribute("aria-label")))
      .toEqual(["New Task", "Recent Task", "Older Task"]);
  });

  it("R1b: a late page cannot overwrite a saved Task move between Lists", async () => {
    const user = userEvent.setup();
    const pendingPage = deferred<TaskPage>();
    const list = {
      id: "list-1", name: "Project", revision: 1,
      createdAt: 1, updatedAt: 1, removed: false,
    };
    let current = { ...existing };
    let pending = false;
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [list], nextCursor: null };
      if (command === "list_tasks") {
        if ((args as { cursor: string | null }).cursor === "next") {
          pending = true;
          return pendingPage.promise;
        }
        return { items: [{ ...current }], nextCursor: "next" };
      }
      if (command === "move_task") {
        current = { ...current, listId: "list-1", revision: current.revision + 1 };
        return { ...current };
      }
      if (command === "list_progress_entries") return { items: [], nextCursor: null };
      throw new Error("Unexpected native command: " + command);
    });
    render(<App />);
    await screen.findByRole("article", { name: "Existing Task" });
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(pending).toBe(true));

    await user.click(screen.getByRole("button", { name: "Open Existing Task" }));
    const details = screen.getByRole("region", { name: "Task details" });
    await user.selectOptions(
      details.querySelector('select[aria-label="Task list"]') as HTMLSelectElement,
      "list-1",
    );
    await waitFor(() => expect(current.listId).toBe("list-1"));
    expect(screen.getByRole("article", { name: "Existing Task" })).toBeInTheDocument();

    await act(async () => {
      pendingPage.resolve({
        items: [{ ...existing, id: "old", title: "Stale Page" }],
        nextCursor: null,
      });
      await pendingPage.promise;
    });
    expect(screen.getByRole("article", { name: "Existing Task" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Stale Page" })).not.toBeInTheDocument();
  });

  it("R3b: returning to the app after a suspended clock refreshes local deadlines", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 14, 23, 59, 58));
    const todo = {
      ...existing, id: "resumed-task", title: "Resume deadline",
      dueDate: "2026-10-14",
    };
    const completed = {
      ...existing, id: "resumed-done", title: "Completed deadline",
      dueDate: "2026-10-14", status: "completed" as const, progress: 100,
    };
    nativeInvoke.mockImplementation(async (command) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command === "list_tasks") return { items: [todo, completed], nextCursor: null };
      throw new Error("Unexpected native command: " + command);
    });
    render(<App />);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByRole("article", { name: "Resume deadline" }))
      .toHaveTextContent("Due today");

    // Clock changed while the macOS app was suspended, without firing timers.
    vi.setSystemTime(new Date(2026, 9, 15, 12, 0, 0));
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(screen.getByRole("article", { name: "Resume deadline" }))
      .toHaveTextContent("Overdue");
    expect(screen.getByRole("article", { name: "Completed deadline" }))
      .not.toHaveTextContent("Overdue");
  });

  it("R3: due-today updates to Overdue at local midnight without another UI action", async () => {
    const localBeforeMidnight = new Date(2026, 9, 14, 23, 59, 58);
    vi.useFakeTimers();
    vi.setSystemTime(localBeforeMidnight);
    const dueToday: Task = {
      ...existing, id: "due-today", title: "Due at midnight",
      dueDate: "2026-10-14",
    };
    const alreadyCompleted: Task = {
      ...existing, id: "completed", title: "Already completed",
      status: "completed", progress: 100, dueDate: "2026-10-14",
    };
    nativeInvoke.mockImplementation(async (command) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command === "list_tasks") {
        return { items: [dueToday, alreadyCompleted], nextCursor: null };
      }
      throw new Error("Unexpected native command: " + command);
    });
    render(<App />);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByRole("article", { name: "Due at midnight" }))
      .toHaveTextContent("Due today");
    expect(screen.getByRole("article", { name: "Already completed" }))
      .not.toHaveTextContent("Overdue");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(screen.getByRole("article", { name: "Due at midnight" }))
      .toHaveTextContent("Overdue");
    expect(screen.getByRole("article", { name: "Already completed" }))
      .not.toHaveTextContent("Overdue");
  });

});
