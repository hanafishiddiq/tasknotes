import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function task(overrides: Partial<Task>): Task {
  return {
    id: "task-1",
    title: "Untitled",
    description: "",
    status: "ready",
    priority: "medium",
    progress: 0,
    nextStep: "",
    listId: null,
    dueDate: null,
    tags: [],
    revision: 1,
    createdAt: 100,
    updatedAt: 100,
    ...overrides,
  };
}

let saved: Task[];

beforeEach(() => {
  saved = [];
  window.localStorage.clear();
  delete document.documentElement.dataset.theme;
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command) => {
    if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_tasks") return { items: [...saved], nextCursor: null };
    if (command === "list_progress_entries") return { items: [], nextCursor: null };
    throw new Error("Unexpected command: " + command);
  });
});

afterEach(() => cleanup());

describe("Notes-style desktop shell", () => {
  it("opens in the dark theme and remembers a switch to light", async () => {
    const user = userEvent.setup();
    const first = render(<App />);
    await screen.findByText("Your list is clear");
    expect(document.documentElement.dataset.theme).toBe("dark");

    await user.click(screen.getByRole("button", { name: "Switch to light theme" }));
    expect(document.documentElement.dataset.theme).toBe("light");

    first.unmount();
    delete document.documentElement.dataset.theme;
    render(<App />);
    await screen.findByText("Your list is clear");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(screen.getByRole("button", { name: "Switch to dark theme" })).toBeInTheDocument();
  });

  it("keeps quick capture to one field until a title is typed", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");

    expect(screen.queryByRole("textbox", { name: "Description (optional)" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add task" })).not.toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Call the plumber");
    expect(screen.getByRole("textbox", { name: "Description (optional)" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add task" })).toBeEnabled();
  });

  it("shows progress and the Next Step as the row's second line", async () => {
    saved = [task({
      id: "t-1", title: "Draft migration", status: "in_progress", progress: 40,
      nextStep: "Write rollback test", description: "Longer context",
    })];
    render(<App />);

    const row = await screen.findByRole("article", { name: "Draft migration" });
    expect(within(row).getByLabelText("Progress 40%")).toHaveTextContent("40%");
    expect(within(row).getByText("Next: Write rollback test")).toBeInTheDocument();
    expect(within(row).queryByText("Longer context")).not.toBeInTheDocument();
  });

  it("saves a wrapped detail title with Enter instead of adding a line break", async () => {
    const user = userEvent.setup();
    saved = [task({ id: "t-3", title: "Long title" })];
    const updates: unknown[] = [];
    const base = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "update_task") {
        const input = (args as { input: { title: string } }).input;
        updates.push(input);
        saved = [{ ...saved[0], title: input.title, revision: 2 }];
        return saved[0];
      }
      return base(command, args);
    });
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Open Long title" }));
    const title = screen.getByRole("textbox", { name: "Edit title" });
    expect(title.tagName).toBe("TEXTAREA");
    await user.type(title, " v2{Enter}");

    expect(updates).toEqual([expect.objectContaining({ title: "Long title v2" })]);
  });

  it("falls back to the description when there is no Next Step", async () => {
    saved = [task({ id: "t-2", title: "Buy bulbs", description: "Warm white, E27" })];
    render(<App />);

    const row = await screen.findByRole("article", { name: "Buy bulbs" });
    expect(within(row).getByText("Warm white, E27")).toBeInTheDocument();
  });
});

describe("native QA findings (2026-10-22)", () => {
  const home = {
    id: "list-home", name: "Home Errands", revision: 1,
    createdAt: 1, updatedAt: 1, removed: false,
  };

  function folderCount(button: HTMLElement) {
    return button.querySelector(".folder-count")?.textContent ?? null;
  }

  it("shows the visible count beside the active view only", async () => {
    const user = userEvent.setup();
    const inboxTask = task({ id: "inbox-1", title: "Inbox task" });
    const homeTasks = [
      task({ id: "home-1", title: "Buy groceries", listId: home.id }),
      task({ id: "home-2", title: "Call the dentist", listId: home.id }),
    ];
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [home], nextCursor: null };
      if (command === "list_tasks") {
        const view = (args as { view?: { kind: string } }).view;
        if (view?.kind === "list") return { items: homeTasks, nextCursor: null };
        if (view?.kind === "inbox") return { items: [inboxTask], nextCursor: null };
        return { items: [inboxTask, ...homeTasks], nextCursor: "more" };
      }
      throw new Error("Unexpected command: " + command);
    });
    render(<App />);
    await screen.findByRole("article", { name: "Inbox task" });
    const allTasks = screen.getByRole("button", { name: "All tasks" });
    const inbox = screen.getByRole("button", { name: "Inbox" });
    const list = screen.getByRole("button", { name: "Home Errands" });
    expect(folderCount(allTasks)).toBe("3+");
    expect(folderCount(list)).toBeNull();

    await user.click(list);
    await screen.findByRole("article", { name: "Call the dentist" });
    expect(folderCount(list)).toBe("2");
    expect(folderCount(allTasks)).toBeNull();
    expect(folderCount(inbox)).toBeNull();
  });

  it("shows +N for Tags beyond the two shown on a row", async () => {
    saved = [task({ title: "Tagged task", tags: ["Café", "Urgent", "Work", "Home"] })];
    render(<App />);
    const row = await screen.findByRole("article", { name: "Tagged task" });
    expect(within(row).getByText("#Café")).toBeInTheDocument();
    expect(within(row).getByText("#Urgent")).toBeInTheDocument();
    expect(within(row).queryByText("#Work")).not.toBeInTheDocument();
    const more = within(row).getByText("+2");
    expect(more).toHaveAccessibleName("2 more tags: Work, Home");
  });

  it("discards an unsaved Tags and Deadline draft, unlocking other fields", async () => {
    const user = userEvent.setup();
    saved = [task({ title: "Draft homepage copy", tags: ["Work"], dueDate: "2026-10-21" })];
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Draft homepage copy" }));
    const tags = screen.getByRole("textbox", { name: "Tags" });
    await user.clear(tags);
    await user.type(tags, "WORK");
    expect(screen.getByRole("textbox", { name: "Progress note" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Discard tag and deadline changes" }));
    expect(screen.getByRole("textbox", { name: "Tags" })).toHaveValue("Work");
    expect(screen.getByLabelText("Deadline")).toHaveValue("2026-10-21");
    expect(screen.queryByRole("button", { name: "Save tags and deadline" })).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Progress note" })).toBeEnabled();
    expect(invokeMock).not.toHaveBeenCalledWith("update_task_details", expect.anything());
  });

  it("does not label a Task in an unloaded List as Inbox in its details", async () => {
    const user = userEvent.setup();
    saved = [task({ title: "Unloaded list task", listId: "list-unloaded" })];
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Unloaded list task" }));
    const details = screen.getByRole("region", { name: "Task details" });
    expect(within(details).queryByText(/· Inbox$/)).not.toBeInTheDocument();
    expect(within(details).getByText(/· List not loaded$/)).toBeInTheDocument();
  });
});
