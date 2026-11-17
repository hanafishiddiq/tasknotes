/**
 * Hide Completed (batch 4, owner decisions 2026-11-17): a view hides its
 * Completed Tasks (Won't Do included) by default; a row under the list says
 * "Show Completed (N)" and turns into "Hide Completed". Each view remembers its
 * choice. A search or the Completed (or Won't Do) status filter always shows
 * them. The core leaves them out of its pages and counts them with the view's
 * filters (crates/tasknotes-core/tests/hide_completed.rs). Synthetic data only.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

let saved: Task[];
const lists: TaskList[] = [
  { id: "l1", name: "Synthetic one", revision: 1, createdAt: 1, updatedAt: 1, removed: false },
  { id: "l2", name: "Synthetic two", revision: 1, createdAt: 1, updatedAt: 1, removed: false },
];
const GROUP = { in_progress: 0, ready: 1, completed: 2 } as const;

function task(id: string, fields: Partial<Task> = {}): Task {
  return {
    id, title: `Synthetic ${id}`, description: "", status: "ready", priority: "medium", difficulty: null,
    progress: 0, wontDo: false, nextStep: "", listId: "l1", dueDate: null, tags: [], revision: 1,
    createdAt: 100, updatedAt: 100, ...fields,
  };
}

/** What the core's query keeps: view, status, words, and Hide Completed. */
function matching(body: Record<string, any>): Task[] {
  return saved
    .filter((item) => body.view?.kind !== "list" || item.listId === body.view.listId)
    .filter((item) => !body.status || item.status === body.status)
    .filter((item) => body.wontDo === undefined || item.wontDo === body.wontDo)
    .filter((item) => !body.text || item.title.toLowerCase().includes(String(body.text).toLowerCase()))
    .filter((item) => !body.hideCompleted || item.status !== "completed");
}

beforeEach(() => {
  window.localStorage.clear();
  saved = [
    task("started", { status: "in_progress", progress: 40 }),
    task("waiting"),
    task("done", { status: "completed", progress: 100, completedFrom: "ready" }),
    task("shelved", { status: "completed", progress: 10, wontDo: true, completedFrom: "ready" }),
    task("other done", { status: "completed", progress: 100, listId: "l2", completedFrom: "ready" }),
  ];
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists, nextCursor: null };
      case "list_tasks":
        return { items: matching(body).sort((a, b) => GROUP[a.status] - GROUP[b.status]), nextCursor: null };
      case "count_tasks": return matching(body).length;
      case "change_task": {
        const at = saved.findIndex((item) => item.id === body.input.id);
        saved[at] = { ...saved[at], status: "completed", progress: 100, completedFrom: saved[at].status as "ready" | "in_progress", revision: saved[at].revision + 1 };
        return saved[at];
      }
      case "list_notes":
      case "list_task_notes":
      case "list_progress_entries":
        return { items: [], nextCursor: null };
      default: return null;
    }
  });
});
afterEach(() => cleanup());

const rows = () => screen.queryAllByRole("article").map((row) => row.getAttribute("aria-label"));
const lastListArgs = () => {
  const found = call.mock.calls.filter(([command]) => command === "list_tasks");
  return found[found.length - 1][1] as Record<string, any>;
};

async function openList(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(await screen.findByRole("button", { name }));
  await screen.findByRole("heading", { level: 1, name });
}

describe("Hide Completed", () => {
  it("hides a view's Completed Tasks by default and says how many", async () => {
    const user = userEvent.setup();
    render(<App />);
    await openList(user, "Synthetic one");
    await waitFor(() => expect(rows()).toEqual(["Synthetic started", "Synthetic waiting"]));
    expect(screen.queryByRole("heading", { name: /^Completed/ })).toBeNull();
    // Won't Do counts too: it is a kind of Completed.
    expect(await screen.findByRole("button", { name: "Show Completed (2)" })).toBeInTheDocument();
    expect(lastListArgs()).toMatchObject({ hideCompleted: true });
  });

  it("shows them on request, offers Hide Completed, and remembers the choice for that view only", async () => {
    const user = userEvent.setup();
    render(<App />);
    await openList(user, "Synthetic one");
    await user.click(await screen.findByRole("button", { name: "Show Completed (2)" }));
    await waitFor(() => expect(rows()).toContain("Synthetic done"));
    expect(rows()).toContain("Synthetic shelved");
    expect(screen.getByRole("button", { name: "Hide Completed" })).toBeInTheDocument();
    expect(lastListArgs().hideCompleted).toBeUndefined();

    // Another List keeps its own choice: hidden.
    await openList(user, "Synthetic two");
    expect(await screen.findByRole("button", { name: "Show Completed (1)" })).toBeInTheDocument();
    expect(rows()).toEqual([]);

    // After a restart the first List still shows them.
    cleanup();
    render(<App />);
    await openList(userEvent.setup(), "Synthetic one");
    await waitFor(() => expect(rows()).toContain("Synthetic done"));
    await user.click(screen.getByRole("button", { name: "Hide Completed" }));
    await waitFor(() => expect(rows()).not.toContain("Synthetic done"));
  });

  it("always shows them to a search and to the Completed status filter", async () => {
    const user = userEvent.setup();
    render(<App />);
    await openList(user, "Synthetic one");
    await screen.findByRole("button", { name: "Show Completed (2)" });

    await user.type(screen.getByRole("searchbox", { name: "Search tasks and notes" }), "done");
    await waitFor(() => expect(rows()).toEqual(["Synthetic done"]));
    expect(screen.queryByRole("button", { name: /Completed \(/ })).toBeNull();
    await user.clear(screen.getByRole("searchbox", { name: "Search tasks and notes" }));
    await waitFor(() => expect(rows()).not.toContain("Synthetic done"));

    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by status" }), "completed");
    await waitFor(() => expect(rows()).toEqual(["Synthetic done", "Synthetic shelved"]));
  });

  it("takes a Task just completed out of the list and counts it", async () => {
    const user = userEvent.setup();
    render(<App />);
    await openList(user, "Synthetic one");
    await screen.findByRole("button", { name: "Show Completed (2)" });
    await user.click(within(screen.getByRole("article", { name: "Synthetic waiting" }))
      .getByRole("button", { name: "Complete “Synthetic waiting”" }));
    await waitFor(() => expect(rows()).toEqual(["Synthetic started"]), { timeout: 4000 });
    expect(await screen.findByRole("button", { name: "Show Completed (3)" })).toBeInTheDocument();
  });

  it("still offers them when every Task in the view is Completed", async () => {
    const user = userEvent.setup();
    render(<App />);
    await openList(user, "Synthetic two");
    expect(await screen.findByRole("button", { name: "Show Completed (1)" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show Completed (1)" }));
    await waitFor(() => expect(rows()).toEqual(["Synthetic other done"]));
  });
});
