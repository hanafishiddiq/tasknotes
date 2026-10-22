import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
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
});
