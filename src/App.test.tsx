import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

type SavedTask = {
  id: string;
  title: string;
  description: string;
  status: "ready";
  priority: "medium";
  progress: number;
  revision: number;
  createdAt: number;
  updatedAt: number;
};

let saved: SavedTask[];

beforeEach(() => {
  saved = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const data = args as Record<string, unknown> | undefined;
    if (command === "list_tasks") {
      return { items: [...saved], nextCursor: null };
    }
    if (command === "create_task") {
      const input = data?.input as { title: string; description: string };
      const task: SavedTask = {
        id: "task-001",
        title: input.title.trim(),
        description: input.description,
        status: "ready",
        priority: "medium",
        progress: 0,
        revision: 1,
        createdAt: 123,
        updatedAt: 123,
      };
      saved = [task, ...saved];
      return task;
    }
    if (command === "update_task") {
      const input = data?.input as {
        id: string; title: string; description: string; expectedRevision: number
      };
      saved = saved.map((task) =>
        task.id === input.id
          ? { ...task, title: input.title, description: input.description, revision: task.revision + 1 }
          : task,
      );
      return saved.find((task) => task.id === input.id);
    }
    throw new Error("Unexpected command: " + command);
  });
});

afterEach(() => cleanup());

describe("Tasknotes desktop", () => {
  it("captures a Task with its description and keeps it ready", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByRole("heading", { name: "All tasks" })).toBeInTheDocument();
    expect(screen.getByText("Your list is clear")).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Write report");
    await user.type(screen.getByRole("textbox", { name: "Description (optional)" }), "Gather sources first");
    await user.click(screen.getByRole("button", { name: "Add task" }));

    const row = await screen.findByRole("article", { name: "Write report" });
    expect(within(row).getByText("Ready")).toBeInTheDocument();
    expect(within(row).getByText("Medium")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("create_task", {
      input: { title: "Write report", description: "Gather sources first" },
    });
    expect(invokeMock).toHaveBeenCalledWith("list_tasks", { limit: 50, cursor: null });
  });

  it("expands, edits, saves a Task and shows its persisted details", async () => {
    saved = [{
      id: "task-001", title: "Old name", description: "First notes",
      status: "ready", priority: "medium", progress: 0, revision: 3,
      createdAt: 123, updatedAt: 123,
    }];
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText("Old name")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open Old name" }));
    const editor = screen.getByRole("region", { name: "Task details" });
    await user.clear(within(editor).getByRole("textbox", { name: "Edit title" }));
    await user.type(within(editor).getByRole("textbox", { name: "Edit title" }), "New name");
    await user.clear(within(editor).getByRole("textbox", { name: "Edit description" }));
    await user.type(within(editor).getByRole("textbox", { name: "Edit description" }), "Final details");
    await user.click(within(editor).getByRole("button", { name: "Save changes" }));

    await waitFor(() => expect(saved[0].title).toBe("New name"));
    expect(invokeMock).toHaveBeenCalledWith("update_task", {
      input: { id: "task-001", title: "New name", description: "Final details", expectedRevision: 3 },
    });
    expect(screen.getByText("New name")).toBeInTheDocument();
    expect(screen.getByText("Final details")).toBeInTheDocument();
  });

  it("keeps the first page bounded and loads more only when requested", async () => {
    const earlier = {
      id: "first", title: "First task", description: "", status: "ready" as const,
      priority: "medium" as const, progress: 0, revision: 1, createdAt: 123, updatedAt: 123,
    };
    const later = { ...earlier, id: "second", title: "Second task" };
    invokeMock.mockImplementation(async (command, args) => {
      if (command !== "list_tasks") throw new Error("Unexpected command");
      const cursor = (args as { cursor: string | null })?.cursor;
      return cursor === null
        ? { items: [earlier], nextCursor: "cursor-2" }
        : { items: [later], nextCursor: null };
    });

    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText("First task")).toBeInTheDocument();
    expect(screen.queryByText("Second task")).not.toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_tasks", { limit: 50, cursor: null });
    await user.click(screen.getByRole("button", { name: "Load more tasks" }));
    expect(await screen.findByText("Second task")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_tasks", { limit: 50, cursor: "cursor-2" });
    expect(screen.queryByRole("button", { name: "Load more tasks" })).not.toBeInTheDocument();
  });

  it("never calls a failed first load an empty list and lets the user retry", async () => {
    let calls = 0;
    invokeMock.mockImplementation(async (command) => {
      if (command !== "list_tasks") throw new Error("Unexpected command");
      calls += 1;
      if (calls === 1) throw new Error("Storage unavailable");
      return { items: [], nextCursor: null };
    });

    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Storage unavailable");
    expect(screen.queryByText("Your list is clear")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Your list is clear")).toBeInTheDocument();
    expect(calls).toBe(2);
  });

  it("keeps errors visible without pretending failed writes succeeded", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_tasks") return { items: [], nextCursor: null };
      throw new Error("Storage unavailable");
    });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Important");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Storage unavailable");
    expect(screen.queryByRole("article", { name: "Important" })).not.toBeInTheDocument();
    expect(screen.getByText("Your list is clear")).toBeInTheDocument();
    expect(screen.queryByText("Couldn't load your tasks")).not.toBeInTheDocument();
  });

  it("keeps a successful create when sorted refresh fails and allows reloading", async () => {
    let listReads = 0;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "list_tasks") {
        listReads++;
        if (listReads === 2) throw new Error("Temporary read error");
        return { items: [...saved], nextCursor: null };
      }
      if (command === "create_task") {
        const input = (args as { input: { title: string; description: string } }).input;
        const created: SavedTask = {
          id: "once", title: input.title, description: input.description,
          status: "ready", priority: "medium", progress: 0, revision: 1,
          createdAt: 123, updatedAt: 123,
        };
        saved.push(created);
        return created;
      }
      throw new Error("Unexpected command: " + command);
    });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "One time only");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Task created");
    expect(screen.getByRole("textbox", { name: "Task title" })).toHaveValue("");
    expect(saved).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Refresh list" }));
    expect(await screen.findByRole("article", { name: "One time only" })).toBeInTheDocument();
    expect(saved).toHaveLength(1);
  });

});
