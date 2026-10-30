import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task } from "./lib/task-api";
import type { PendingBulkRequest } from "./lib/approval-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * A stand-in for the core behind the Tauri seam. What a request may do, and
 * whether it is stale, is decided and tested in the core; this only mimics
 * what the window observes. Synthetic data only.
 */
let tasks: Task[];
let pending: PendingBulkRequest[];

const base: Task = {
  id: "task-1", title: "Synthetic draft", description: "",
  status: "ready", priority: "medium", progress: 0, nextStep: "", revision: 1,
  createdAt: 100, updatedAt: 100, listId: null, dueDate: null, tags: [],
};

function request(id: string, overrides: Partial<PendingBulkRequest> = {}): PendingBulkRequest {
  return {
    request: {
      id, action: { action: "remove_tasks" }, count: 2, requestedBy: "mcp",
      createdAt: 1_700_000_000_000, expiresAt: 1_700_086_400_000, status: "pending", decidedAt: null,
    },
    stale: false, target: null, total: 2,
    items: [
      { kind: "task", id: "task-1", title: "Synthetic draft", excludedFromAi: false },
      { kind: "task", id: "task-2", title: "Synthetic review", excludedFromAi: false },
    ],
    ...overrides,
  };
}

beforeEach(() => {
  tasks = [{ ...base }, { ...base, id: "task-2", title: "Synthetic review" }];
  pending = [];
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: tasks.map((task) => ({ ...task })), nextCursor: null };
      case "count_pending_bulk_requests": return pending.length;
      case "list_pending_bulk_requests": return pending.map((item) => structuredClone(item));
      case "approve_bulk_request": {
        const found = pending.find((item) => item.request.id === body.id)!;
        if (found.stale) throw "Something in this request changed since it was made; nothing was changed. Ask again";
        pending = pending.filter((item) => item !== found);
        tasks = tasks.filter((task) => !found.items.some((line) => line.id === task.id));
        return { ...found.request, status: "applied" };
      }
      case "decline_bulk_request": {
        const found = pending.find((item) => item.request.id === body.id)!;
        pending = pending.filter((item) => item !== found);
        return { ...found.request, status: "declined" };
      }
      default: throw new Error("Unexpected native command " + command);
    }
  });
});
afterEach(() => cleanup());

const openApprovals = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(await screen.findByRole("button", { name: /^Waiting for approval/ }));
  return screen.findByRole("region", { name: "Waiting for approval" });
};

describe("Waiting for approval", () => {
  it("shows nothing in the sidebar while no AI request waits", async () => {
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic draft" });
    await waitFor(() => expect(call).toHaveBeenCalledWith("count_pending_bulk_requests"));
    expect(screen.queryByRole("button", { name: /Waiting for approval/ })).not.toBeInTheDocument();
  });

  it("lists a pending request with its preview and applies it only when approved", async () => {
    pending = [request("req-1")];
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole("button", { name: "Waiting for approval, 1" })).toBeInTheDocument();
    const view = await openApprovals(user);
    const item = await within(view).findByRole("listitem", { name: "Move 2 Tasks to Removed" });
    expect(within(item).getByText("Synthetic draft")).toBeInTheDocument();
    expect(within(item).getByText("Synthetic review")).toBeInTheDocument();
    expect(within(item).getByText(/Asked by an MCP client/)).toBeInTheDocument();
    expect(call).not.toHaveBeenCalledWith("approve_bulk_request", expect.anything());

    await user.click(within(item).getByRole("button", { name: "Approve: Move 2 Tasks to Removed" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("approve_bulk_request", { id: "req-1" }));
    expect(await within(view).findByText("Nothing waiting for approval")).toBeInTheDocument();
    // The Task view and the count catch up.
    await user.click(screen.getByRole("button", { name: "All tasks" }));
    await waitFor(() => expect(screen.queryByRole("article", { name: "Synthetic draft" })).not.toBeInTheDocument());
    await waitFor(() => expect(screen.queryByRole("button", { name: /Waiting for approval/ })).not.toBeInTheDocument());
  });

  it("declines without changing anything", async () => {
    pending = [request("req-2", { request: { ...request("req-2").request, action: { action: "set_task_status", status: "completed" } } })];
    const user = userEvent.setup();
    render(<App />);
    const view = await openApprovals(user);
    const item = await within(view).findByRole("listitem", { name: "Mark 2 Tasks Completed" });
    await user.click(within(item).getByRole("button", { name: "Decline: Mark 2 Tasks Completed" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("decline_bulk_request", { id: "req-2" }));
    expect(tasks).toHaveLength(2);
    expect(call).not.toHaveBeenCalledWith("approve_bulk_request", expect.anything());
  });

  it("explains a stale request and offers only Decline", async () => {
    pending = [request("req-3", { stale: true })];
    const user = userEvent.setup();
    render(<App />);
    const view = await openApprovals(user);
    const item = await within(view).findByRole("listitem", { name: "Move 2 Tasks to Removed" });
    expect(within(item).getByText(/Something changed since this was asked/)).toBeInTheDocument();
    expect(within(item).getByRole("button", { name: /^Approve/ })).toBeDisabled();
    expect(within(item).getByRole("button", { name: /^Decline/ })).toBeEnabled();
  });

  it("shows the person everything a folder removal would hide, private Notes marked", async () => {
    pending = [request("req-4", {
      request: { ...request("req-4").request, action: { action: "remove_folder" }, count: null },
      target: "Synthetic folder", total: 2,
      items: [
        { kind: "folder", id: "f-2", title: "Synthetic subfolder", excludedFromAi: false },
        { kind: "note", id: "n-1", title: "Synthetic private note", excludedFromAi: true },
      ],
    })];
    const user = userEvent.setup();
    render(<App />);
    const view = await openApprovals(user);
    const item = await within(view).findByRole("listitem", {
      name: "Move the folder “Synthetic folder” and 2 items in it to Removed",
    });
    expect(within(item).getByText("Synthetic private note")).toBeInTheDocument();
    expect(within(item).getByText("Excluded from AI")).toBeInTheDocument();
  });

  it("asks again when the window regains focus, so a new request appears", async () => {
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic draft" });
    pending = [request("req-5")];
    window.dispatchEvent(new Event("focus"));
    expect(await screen.findByRole("button", { name: "Waiting for approval, 1" })).toBeInTheDocument();
  });
});
