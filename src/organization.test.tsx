import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);
let records: Task[];
let lists: TaskList[];
let revision: number;

const base: Task = {
  id: "task-1", title: "Prepare launch", description: "",
  status: "ready", priority: "medium", progress: 0, nextStep: "", revision: 1,
  createdAt: 100, updatedAt: 100,
  listId: null, dueDate: null, tags: [],
};

beforeEach(() => {
  records = [{ ...base }];
  lists = [];
  revision = 0;
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = args as Record<string, any> ?? {};
    switch (command) {
      case "list_lists": return { items: [...lists], nextCursor: null };
      case "create_list": {
        const item: TaskList = {
          id: "list-1", name: body.input.name, revision: 1,
          createdAt: ++revision, updatedAt: revision, removed: false,
        };
        lists.push(item);
        return item;
      }
      case "rename_list": {
        const list = lists.find((x) => x.id === body.input.id)!;
        list.name = body.input.name;
        list.revision++;
        return { ...list };
      }
      case "set_list_removed": {
        const list = lists.find((x) => x.id === body.input.id)!;
        list.removed = body.input.removed;
        list.revision++;
        return { ...list };
      }
      case "list_removed": return {
        items: lists.filter((list) => list.removed).map((list) => ({
          kind: "list", id: list.id, title: list.name, revision: list.revision,
          removedAt: list.updatedAt, inRemovedContainer: false,
        })),
        nextCursor: null,
      };
      case "list_tasks": {
        const v = body.view?.kind ?? "all";
        const active = lists.filter((list) => !list.removed).map((x) => x.id);
        return {
          items: records.filter((task) =>
            (task.listId === null || active.includes(task.listId!))
            && (v === "all" || (v === "inbox" && task.listId === null)
                || (v === "list" && task.listId === body.view.listId))),
          nextCursor: null,
        };
      }
      case "create_task": {
        const task = { ...base, id: "new-task", title: body.input.title, description: body.input.description,
          listId: body.listId ?? null };
        records.push(task);
        return { ...task };
      }
      case "move_task": {
        const task = records.find((r) => r.id === body.input.id)!;
        task.listId = body.input.listId;
        task.revision++;
        return { ...task };
      }
      case "update_task_details": {
        const task = records.find((r) => r.id === body.input.id)!;
        task.tags = body.input.tags;
        task.dueDate = body.input.dueDate;
        task.revision++;
        return { ...task };
      }
      default: throw new Error("Unexpected native command " + command);
    }
  });
});
afterEach(() => cleanup());

describe("Inbox, Lists, and Task metadata", () => {
  it("creates a named List then captures a Task there without duplicating All Tasks", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "All tasks" });

    await user.type(screen.getByRole("textbox", { name: "New list name" }), "Project");
    await user.click(screen.getByRole("button", { name: "Add list" }));
    await user.click(await screen.findByRole("button", { name: "Project" }));

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Ship preview");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("create_task", {
      input: { title: "Ship preview", description: "" }, listId: "list-1",
    }));
    expect(await screen.findByRole("article", { name: "Ship preview" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Inbox" }));
    expect(await screen.findByRole("article", { name: "Prepare launch" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Ship preview" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "All tasks" }));
    expect(await screen.findAllByRole("article")).toHaveLength(2);
  });

  it("moves an Inbox Task and saves tag/deadline while displaying overdue", async () => {
    const user = userEvent.setup();
    lists = [{ id: "list-1", name: "Project", revision: 1, createdAt: 1, updatedAt: 1, removed: false }];
    render(<App />);
    await screen.findByRole("article", { name: "Prepare launch" });
    await user.click(screen.getByRole("button", { name: "Open Prepare launch" }));
    const editor = screen.getByRole("region", { name: "Task details" });
    await user.selectOptions(within(editor).getByRole("combobox", { name: "Task list" }), "list-1");
    await waitFor(() => expect(call).toHaveBeenCalledWith("move_task", {
      input: { id: "task-1", expectedRevision: 1, listId: "list-1" },
    }));

    await user.type(within(editor).getByRole("textbox", { name: "Tags" }), "Work, Urgent");
    await user.type(within(editor).getByLabelText("Deadline"), "2020-01-01");
    await user.click(within(editor).getByRole("button", { name: "Save tags and deadline" }));

    await waitFor(() => expect(call).toHaveBeenCalledWith("update_task_details", {
      input: { id: "task-1", expectedRevision: 2, tags: ["Work", "Urgent"], dueDate: "2020-01-01" },
    }));
    expect(await within(screen.getByRole("article", { name: "Prepare launch" })).findByText("Overdue")).toBeInTheDocument();
  });

  it("removes a List reversibly without deleting its Task, then restores the Task", async () => {
    const user = userEvent.setup();
    lists = [{ id: "list-1", name: "Project", revision: 1, createdAt: 1, updatedAt: 1, removed: false }];
    records = [{ ...base, listId: "list-1" }];
    render(<App />);
    await screen.findByRole("article", { name: "Prepare launch" });
    await user.click(screen.getByRole("button", { name: "Remove Project" }));
    await waitFor(() => {
      expect(screen.queryByRole("article", { name: "Prepare launch" })).not.toBeInTheDocument();
    });
    expect(records).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Removed" }));
    await user.click(await screen.findByRole("button", { name: "Restore Project" }));
    await user.click(screen.getByRole("button", { name: "All tasks" }));
    expect(await screen.findByRole("article", { name: "Prepare launch" })).toBeInTheDocument();
    expect(records).toHaveLength(1);
  });

  it("protects unsaved title edits from simultaneous deadline updates", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Prepare launch" });
    await user.click(screen.getByRole("button", { name: "Open Prepare launch" }));
    const editor = screen.getByRole("region", { name: "Task details" });
    await user.type(within(editor).getByRole("textbox", { name: "Edit title" }), " extra");
    expect(within(editor).getByRole("textbox", { name: "Tags" })).toBeDisabled();
    expect(within(editor).getByLabelText("Deadline")).toBeDisabled();
    expect(within(editor).getByRole("combobox", { name: "Task list" })).toBeDisabled();
    expect(call).not.toHaveBeenCalledWith("update_task_details", expect.anything());
  });

  it("prevents changing Inbox/List scope while a previous page is in flight", async () => {
    const user = userEvent.setup();
    let resolvePage: ((page: { items: Task[]; nextCursor: string | null }) => void) | undefined;
    call.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command !== "list_tasks") throw new Error("Unexpected native command " + command);
      const { cursor, view } = (args ?? {}) as {
        cursor?: string | null; view?: { kind: string };
      };
      if (cursor === "next") {
        return new Promise((resolve) => { resolvePage = resolve; });
      }
      if (view?.kind === "inbox") return { items: [{ ...base }], nextCursor: null };
      return { items: [{ ...base }], nextCursor: "next" };
    });
    render(<App />);
    expect(await screen.findByRole("article", { name: "Prepare launch" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(resolvePage).toBeDefined());
    // A later List response must not leak into the next scope.
    expect(screen.getByRole("button", { name: "Inbox" })).toBeDisabled();
    resolvePage!({ items: [{ ...base, id: "old-page", title: "Other view Task" }], nextCursor: null });
    await waitFor(() => expect(screen.getByRole("button", { name: "Inbox" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Inbox" }));
    expect(await screen.findByRole("article", { name: "Prepare launch" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Other view Task" })).not.toBeInTheDocument();
  });

  it("keeps the List name draft when the native create is rejected", async () => {
    const user = userEvent.setup();
    const previous = call.getMockImplementation()!;
    call.mockImplementation(async (command, args) => {
      if (command === "create_list") throw new Error("A List with this name already exists");
      return previous(command, args);
    });
    render(<App />);
    await screen.findByRole("article", { name: "Prepare launch" });
    const input = screen.getByRole("textbox", { name: "New list name" });
    await user.type(input, "Already taken");
    await user.click(screen.getByRole("button", { name: "Add list" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("A List with this name already exists");
    expect(input).toHaveValue("Already taken");
  });

  it("preserves the List rename draft when the core rejects a stale revision", async () => {
    const user = userEvent.setup();
    lists = [{ id: "list-1", name: "Project", revision: 1, createdAt: 1, updatedAt: 1, removed: false }];
    const previous = call.getMockImplementation()!;
    call.mockImplementation(async (command, args) => {
      if (command === "rename_list") throw new Error("Revision conflict");
      return previous(command, args);
    });
    render(<App />);
    await screen.findByRole("button", { name: "Project" });
    await user.click(screen.getByRole("button", { name: "Rename Project" }));
    const draft = screen.getByRole("textbox", { name: "Rename list" });
    await user.clear(draft);
    await user.type(draft, "Updated Project");
    await user.click(screen.getByRole("button", { name: "Save list name" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Revision conflict");
    expect(screen.getByRole("textbox", { name: "Rename list" })).toHaveValue("Updated Project");
  });

  it("renders at most 50 Tasks per List page with reversible Next and Previous navigation", async () => {
    const user = userEvent.setup();
    const data = Array.from({ length: 120 }, (_, i) => ({
      ...base, id: "page-" + i, title: "Item " + i,
    }));
    call.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command !== "list_tasks") throw new Error("Unexpected native command " + command);
      const request = (args ?? {}) as { cursor: string | null };
      if (request.cursor === null) return { items: data.slice(0, 50), nextCursor: "cursor-50" };
      if (request.cursor === "cursor-50") return { items: data.slice(50, 100), nextCursor: "cursor-100" };
      if (request.cursor === "cursor-100") return { items: data.slice(100, 120), nextCursor: null };
      throw new Error("Invalid test cursor");
    });
    render(<App />);
    expect(await screen.findByRole("article", { name: "Item 0" })).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(50);

    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(await screen.findByRole("article", { name: "Item 50" })).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(50);
    expect(screen.queryByRole("article", { name: "Item 0" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(await screen.findByRole("article", { name: "Item 100" })).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(20);

    await user.click(screen.getByRole("button", { name: "Previous page" }));
    expect(await screen.findByRole("article", { name: "Item 50" })).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(50);

    await user.click(screen.getByRole("button", { name: "Previous page" }));
    expect(await screen.findByRole("article", { name: "Item 0" })).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(50);
  });

  it("shows an upcoming local date without marking it overdue", async () => {
    records = [{ ...base, dueDate: "2099-01-02" }];
    render(<App />);
    const row = await screen.findByRole("article", { name: "Prepare launch" });
    expect(within(row).getByText("Due Jan 2")).toBeInTheDocument();
    expect(within(row).queryByText("Overdue")).not.toBeInTheDocument();
  });

});
