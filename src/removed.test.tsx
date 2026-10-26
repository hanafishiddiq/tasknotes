import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";
import type { RemovedItem } from "./lib/removed-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * A stand-in for the core behind the Tauri seam, holding Removed as a flag
 * beside each record. The rules (what a container hides, what may be
 * restored) are the core's and are tested there; this only mimics what the
 * desktop observes.
 */
let tasks: (Task & { removedAt: number | null })[];
let lists: TaskList[];
let removedOthers: RemovedItem[];
let clock: number;

const base: Task = {
  id: "task-1", title: "Prepare launch", description: "",
  status: "in_progress", priority: "medium", progress: 30, nextStep: "", revision: 1,
  createdAt: 100, updatedAt: 100, listId: null, dueDate: null, tags: [],
};

function listRemoved(): RemovedItem[] {
  const removedLists = lists.filter((list) => list.removed);
  return [
    ...tasks.filter((task) => task.removedAt !== null).map((task): RemovedItem => ({
      kind: "task", id: task.id, title: task.title, revision: task.revision, removedAt: task.removedAt!,
      inRemovedContainer: removedLists.some((list) => list.id === task.listId),
    })),
    ...removedLists.map((list): RemovedItem => ({
      kind: "list", id: list.id, title: list.name, revision: list.revision, removedAt: list.updatedAt,
      inRemovedContainer: false,
    })),
    ...removedOthers,
  ].sort((a, b) => b.removedAt - a.removedAt);
}

beforeEach(() => {
  tasks = [{ ...base, removedAt: null }];
  lists = [];
  removedOthers = [];
  clock = 1_000;
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((list) => ({ ...list })), nextCursor: null };
      case "list_tasks": {
        const hidden = new Set(lists.filter((list) => list.removed).map((list) => list.id));
        const items = tasks
          .filter((task) => task.removedAt === null && !(task.listId && hidden.has(task.listId)))
          .filter((task) => !body.taskId || task.id === body.taskId)
          .map(({ removedAt: _, ...task }) => task);
        return { items, nextCursor: null };
      }
      case "set_task_removed": {
        const task = tasks.find((item) => item.id === body.input.id)!;
        if (task.revision !== body.input.expectedRevision) {
          throw "Revision conflict: the Task has changed; reload before saving";
        }
        task.removedAt = body.input.removed ? ++clock : null;
        task.revision++;
        const { removedAt, ...shown } = task;
        return { ...shown, removed: removedAt !== null };
      }
      case "set_list_removed": {
        const list = lists.find((item) => item.id === body.input.id)!;
        list.removed = body.input.removed;
        list.revision++;
        list.updatedAt = ++clock;
        return { ...list };
      }
      case "list_removed": {
        const words: string[] = body.text.toLowerCase().split(/\W+/).filter(Boolean);
        const all = listRemoved().filter((item) => words.every((word) => item.title.toLowerCase().includes(word)));
        const start = body.cursor ? Number(body.cursor) : 0;
        const end = start + body.limit;
        return { items: all.slice(start, end), nextCursor: end < all.length ? String(end) : null };
      }
      default: throw new Error("Unexpected native command " + command);
    }
  });
});
afterEach(() => cleanup());

const openRemoved = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(screen.getByRole("button", { name: "Removed" }));
  return screen.findByRole("region", { name: "Removed items" });
};

describe("Removed and restore", () => {
  it("moves a Task to Removed and brings it back from the Removed view unchanged", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Prepare launch" });
    await user.click(screen.getByRole("button", { name: "Open Prepare launch" }));
    const details = screen.getByRole("region", { name: "Task details" });
    await user.click(within(details).getByRole("button", { name: "Move to Removed" }));

    await waitFor(() => expect(call).toHaveBeenCalledWith("set_task_removed", {
      input: { id: "task-1", expectedRevision: 1, removed: true },
    }));
    await waitFor(() => expect(screen.queryByRole("article", { name: "Prepare launch" })).not.toBeInTheDocument());
    // The row is still there: Removed is never a deletion.
    expect(tasks).toHaveLength(1);

    const view = await openRemoved(user);
    expect(await within(view).findByText("Prepare launch")).toBeInTheDocument();
    expect(within(view).getByText("Task")).toBeInTheDocument();
    // Nothing in Removed can be deleted for good, and nothing says it will be.
    expect(screen.queryByRole("button", { name: /delete|erase|empty/i })).not.toBeInTheDocument();
    await user.click(within(view).getByRole("button", { name: "Restore Prepare launch" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("set_task_removed", {
      input: { id: "task-1", expectedRevision: 2, removed: false },
    }));
    expect(await within(view).findByText("Nothing in Removed")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "All tasks" }));
    const row = await screen.findByRole("article", { name: "Prepare launch" });
    expect(within(row).getByText("30%")).toBeInTheDocument();
  });

  it("offers Undo right after a Task is moved to Removed", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Prepare launch" });
    await user.click(screen.getByRole("button", { name: "Open Prepare launch" }));
    await user.click(screen.getByRole("button", { name: "Move to Removed" }));
    const notice = await screen.findByRole("status", { name: "Moved to Removed" });
    await user.click(within(notice).getByRole("button", { name: "Undo" }));
    expect(await screen.findByRole("article", { name: "Prepare launch" })).toBeInTheDocument();
    expect(tasks[0].removedAt).toBeNull();
  });

  it("restores a List from the Removed view and says what must come back first", async () => {
    const user = userEvent.setup();
    lists = [{ id: "list-1", name: "Trip", revision: 3, createdAt: 1, updatedAt: 900, removed: true }];
    tasks = [
      { ...base, id: "task-2", title: "Pack bags", listId: "list-1", removedAt: null },
      { ...base, id: "task-3", title: "Cancel hotel", listId: "list-1", removedAt: 800, revision: 4 },
    ];
    render(<App />);
    const view = await openRemoved(user);

    const blocked = (await within(view).findByText("Cancel hotel")).closest("li")!;
    expect(within(blocked as HTMLElement).getByText("Restore its List first")).toBeInTheDocument();
    expect(within(blocked as HTMLElement).queryByRole("button", { name: /Restore/ })).not.toBeInTheDocument();
    // A Task only hidden by its Removed List is not listed on its own.
    expect(within(view).queryByText("Pack bags")).not.toBeInTheDocument();

    await user.click(within(view).getByRole("button", { name: "Restore Trip" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("set_list_removed", {
      input: { id: "list-1", expectedRevision: 3, removed: false },
    }));
    // The List is back in the sidebar, and the Task removed on its own can come back now.
    expect(await screen.findByRole("button", { name: "Trip" })).toBeInTheDocument();
    expect(await within(view).findByRole("button", { name: "Restore Cancel hotel" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Trip" }));
    expect(await screen.findByRole("article", { name: "Pack bags" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Cancel hotel" })).not.toBeInTheDocument();
  });

  it("shows the core's refusal when a restore cannot go ahead", async () => {
    const user = userEvent.setup();
    tasks = [{ ...base, removedAt: 900, revision: 2 }];
    render(<App />);
    const view = await openRemoved(user);
    await within(view).findByText("Prepare launch");
    call.mockImplementationOnce(async () => {
      throw "Revision conflict: the Task has changed; reload before saving";
    });
    await user.click(within(view).getByRole("button", { name: "Restore Prepare launch" }));
    expect(await within(view).findByRole("alert")).toHaveTextContent("Revision conflict");
    expect(within(view).getByText("Prepare launch")).toBeInTheDocument();
  });

  it("pages through Removed without loading everything at once", async () => {
    const user = userEvent.setup();
    tasks = Array.from({ length: 60 }, (_, n) => ({
      ...base, id: `task-${n}`, title: `Old task ${n}`, removedAt: 100 + n,
    }));
    render(<App />);
    const view = await openRemoved(user);
    await within(view).findByText("Old task 59");
    expect(within(view).getAllByRole("listitem")).toHaveLength(50);
    expect(call).toHaveBeenCalledWith("list_removed", { text: "", limit: 50, cursor: null });
    await user.click(within(view).getByRole("button", { name: "Load more" }));
    await waitFor(() => expect(within(view).getAllByRole("listitem")).toHaveLength(60));
  });

  it("searches Removed, which ordinary search leaves out", async () => {
    const user = userEvent.setup();
    tasks = [
      { ...base, id: "task-2", title: "Book hotel", removedAt: 800 },
      { ...base, id: "task-3", title: "Renew passport", removedAt: 900 },
    ];
    render(<App />);
    const view = await openRemoved(user);
    await within(view).findByText("Renew passport");

    await user.type(within(view).getByRole("searchbox", { name: "Search Removed" }), "hotel");
    await waitFor(() => expect(within(view).queryByText("Renew passport")).not.toBeInTheDocument());
    expect(within(view).getByText("Book hotel")).toBeInTheDocument();
    expect(call).toHaveBeenCalledWith("list_removed", { text: "hotel", limit: 50, cursor: null });

    await user.type(within(view).getByRole("searchbox", { name: "Search Removed" }), " rome");
    expect(await within(view).findByText("No matching removed items")).toBeInTheDocument();
    expect(within(view).queryByText("Nothing in Removed")).not.toBeInTheDocument();
  });
});
