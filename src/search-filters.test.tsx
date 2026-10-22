import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskPage } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function task(overrides: Partial<Task>): Task {
  return {
    id: "task-1", title: "Untitled", description: "", status: "ready",
    priority: "medium", progress: 0, nextStep: "", listId: null, dueDate: null,
    tags: [], revision: 1, createdAt: 100, updatedAt: 100, ...overrides,
  };
}

type ListArgs = {
  limit: number; cursor: string | null; sort?: string;
  text?: string; status?: string; priority?: string; tag?: string;
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((callback) => { resolve = callback; });
  return { promise, resolve };
}

const budget = task({ id: "a", title: "Quarterly budget" });
const lease = task({ id: "b", title: "Renew lease", status: "in_progress", priority: "high", tags: ["Home"] });
let listCalls: ListArgs[];
let respond: (args: ListArgs) => Promise<TaskPage> | TaskPage;

function lastListCall() {
  return listCalls[listCalls.length - 1];
}

beforeEach(() => {
  window.localStorage.clear();
  listCalls = [];
  respond = (args) => ({
    items: [budget, lease].filter((item) =>
      !args.text || item.title.toLowerCase().includes(args.text.toLowerCase())),
    nextCursor: null,
  });
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_tasks") {
      listCalls.push(args as ListArgs);
      return respond(args as ListArgs);
    }
    if (command === "create_task") {
      const input = (args as { input: { title: string } }).input;
      return task({ id: "created", title: input.title, createdAt: 200 });
    }
    throw new Error("Unexpected command: " + command);
  });
});

afterEach(() => cleanup());

describe("Task search and filters", () => {
  it("searches from the toolbar and keeps the plain query free of filter keys", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });
    expect(listCalls[0]).toEqual({ limit: 50, cursor: null });

    await user.type(screen.getByRole("searchbox", { name: "Search tasks" }), "budget");
    await waitFor(() => expect(lastListCall()).toEqual({ limit: 50, cursor: null, text: "budget" }));
    expect(await screen.findByRole("article", { name: "Quarterly budget" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Renew lease" })).not.toBeInTheDocument();
    expect(listCalls.filter((call) => call.text === "b")).toHaveLength(0);
  });

  it("combines status, priority and tag chips with the search text", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });

    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by status" }), "in_progress");
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "high");
    await user.type(screen.getByRole("textbox", { name: "Filter by tag" }), "Home");
    await user.type(screen.getByRole("searchbox", { name: "Search tasks" }), "lease");
    await waitFor(() => expect(lastListCall()).toEqual({
      limit: 50, cursor: null, text: "lease", status: "in_progress", priority: "high", tag: "Home",
    }));

    await user.click(screen.getByRole("button", { name: "Clear search and filters" }));
    await waitFor(() => expect(lastListCall()).toEqual({ limit: 50, cursor: null }));
    expect(screen.getByRole("searchbox", { name: "Search tasks" })).toHaveValue("");
    expect(screen.getByRole("combobox", { name: "Filter by status" })).toHaveValue("");
    expect(screen.getByRole("textbox", { name: "Filter by tag" })).toHaveValue("");
  });

  it("shows a no-match state instead of the empty-list welcome", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });

    await user.type(screen.getByRole("searchbox", { name: "Search tasks" }), "zebra");
    expect(await screen.findByRole("heading", { name: "No matching tasks" })).toBeInTheDocument();
    expect(screen.queryByText("Your list is clear")).not.toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Clear search and filters" })[0]);
    expect(await screen.findByRole("article", { name: "Renew lease" })).toBeInTheDocument();
  });

  it("ignores a slow reply for an older search", async () => {
    const user = userEvent.setup();
    const slow = deferred<TaskPage>();
    respond = (args) => args.text === "rene"
      ? slow.promise
      : { items: args.text ? [lease] : [budget, lease], nextCursor: null };
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });

    const search = screen.getByRole("searchbox", { name: "Search tasks" });
    await user.type(search, "rene");
    await waitFor(() => expect(lastListCall().text).toBe("rene"));
    await user.type(search, "w");
    await waitFor(() => expect(lastListCall().text).toBe("renew"));
    expect(await screen.findByRole("article", { name: "Renew lease" })).toBeInTheDocument();

    await act(async () => {
      slow.resolve({ items: [task({ id: "stale", title: "Stale result" })], nextCursor: null });
      await slow.promise;
    });
    expect(screen.queryByRole("article", { name: "Stale result" })).not.toBeInTheDocument();
    expect(screen.getByRole("article", { name: "Renew lease" })).toBeInTheDocument();
  });

  it("keeps the filters when paging", async () => {
    const user = userEvent.setup();
    respond = (args) => args.cursor === "page-2"
      ? { items: [lease], nextCursor: null }
      : { items: [budget], nextCursor: "page-2" };
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });

    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "high");
    await waitFor(() => expect(lastListCall()).toEqual({ limit: 50, cursor: null, priority: "high" }));
    await user.click(await screen.findByRole("button", { name: "Next page" }));
    await waitFor(() => expect(lastListCall()).toEqual({ limit: 50, cursor: "page-2", priority: "high" }));
  });

  it("keeps accepting typing after the first search results arrive", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });
    const search = screen.getByRole("searchbox", { name: "Search tasks" });
    await user.type(search, "ren");
    await waitFor(() => expect(lastListCall().text).toBe("ren"));
    await screen.findByRole("article", { name: "Renew lease" });

    await user.type(search, "ew");
    expect(search).toHaveValue("renew");
    expect(search).toHaveFocus();
    await waitFor(() => expect(lastListCall().text).toBe("renew"));
  });

  it("says when a new Task is hidden by the active search or filters", async () => {
    const user = userEvent.setup();
    respond = (args) => ({ items: args.priority ? [lease] : [budget, lease], nextCursor: null });
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "high");
    await waitFor(() => expect(screen.queryByRole("article", { name: "Quarterly budget" })).not.toBeInTheDocument());

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Buy oat milk{Enter}");
    const notice = await screen.findByRole("status", { name: "Task hidden by filters" });
    expect(notice).toHaveTextContent("Task added, but hidden by the current search or filters");

    await user.click(within(notice).getByRole("button", { name: "Clear search and filters" }));
    await waitFor(() => expect(lastListCall()).toEqual({ limit: 50, cursor: null }));
    expect(screen.queryByRole("status", { name: "Task hidden by filters" })).not.toBeInTheDocument();
  });

  it("stays quiet when the new Task is visible or no filter is active", async () => {
    const user = userEvent.setup();
    respond = () => ({ items: [task({ id: "created", title: "Buy oat milk" }), lease], nextCursor: null });
    render(<App />);
    await screen.findByRole("article", { name: "Renew lease" });
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by status" }), "ready");
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Buy oat milk{Enter}");
    expect(await screen.findByRole("article", { name: "Buy oat milk" })).toBeInTheDocument();
    expect(screen.queryByRole("status", { name: "Task hidden by filters" })).not.toBeInTheDocument();
  });

  it("drops the hidden-Task notice once the filters change", async () => {
    const user = userEvent.setup();
    respond = (args) => ({ items: args.priority ? [lease] : [budget, lease], nextCursor: null });
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "high");
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Buy oat milk{Enter}");
    await screen.findByRole("status", { name: "Task hidden by filters" });

    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "low");
    await waitFor(() => expect(screen.queryByRole("status", { name: "Task hidden by filters" })).not.toBeInTheDocument());
  });

  it("does not claim a new Task is hidden while more filtered pages exist", async () => {
    const user = userEvent.setup();
    respond = (args) => args.priority
      ? { items: [lease], nextCursor: "more" }
      : { items: [budget, lease], nextCursor: null };
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "high");
    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Buy oat milk{Enter}");
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Task title" })).toHaveValue(""));
    expect(screen.queryByRole("status", { name: "Task hidden by filters" })).not.toBeInTheDocument();
  });

  it("does not reuse a page cursor when the filters change during a save", async () => {
    const user = userEvent.setup();
    const pendingSave = deferred<Task>();
    respond = (args) => {
      // The core rejects a cursor that belongs to another query.
      if (args.cursor === "page-2" && args.status) throw "Invalid task page cursor";
      if (args.cursor === "page-2") return { items: [lease], nextCursor: null };
      return { items: [budget], nextCursor: args.status ? null : "page-2" };
    };
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
      if (command === "list_tasks") {
        listCalls.push(args as ListArgs);
        return respond(args as ListArgs);
      }
      if (command === "change_task") return pendingSave.promise;
      throw new Error("Unexpected command: " + command);
    });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Next page" }));
    await user.click(await screen.findByRole("button", { name: "Open Renew lease" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Priority" }), "very_high");
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by status" }), "in_progress");

    await act(async () => {
      pendingSave.resolve({ ...lease, priority: "very_high", revision: 2 });
      await pendingSave.promise;
    });
    await waitFor(() => expect(lastListCall()).toEqual({ limit: 50, cursor: null, status: "in_progress" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(listCalls.some((call) => call.cursor === "page-2" && call.status)).toBe(false);
  });

  it("explains a refused search instead of claiming the database failed", async () => {
    const user = userEvent.setup();
    respond = (args) => {
      if (args.text && args.text.split(" ").length > 16) throw "Search can use at most 16 words";
      return { items: [budget, lease], nextCursor: null };
    };
    render(<App />);
    await screen.findByRole("article", { name: "Quarterly budget" });

    await user.type(screen.getByRole("searchbox", { name: "Search tasks" }), "a b c d e f g h i j k l m n o p q");
    expect(await screen.findByRole("alert")).toHaveTextContent("Search can use at most 16 words");
    expect(screen.queryByRole("heading", { name: "Couldn't load your tasks" })).not.toBeInTheDocument();
    const section = screen.getByRole("region", { name: "Your tasks" });
    expect(within(section).getByRole("heading", { name: "Search couldn't run" })).toBeInTheDocument();

    await user.click(within(section).getByRole("button", { name: "Clear search and filters" }));
    expect(await screen.findByRole("article", { name: "Renew lease" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
