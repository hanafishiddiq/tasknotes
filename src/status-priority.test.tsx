import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { Task } from "./lib/task-api";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const base: Task = {
  id: "task-1",
  title: "Ready plan",
  description: "",
  status: "ready",
  priority: "medium",
  progress: 0,
  nextStep: "",
  revision: 1,
  createdAt: 100,
  updatedAt: 100,
};
let saved: Task[];

beforeEach(() => {
  saved = [{ ...base }];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_tasks") return { items: [...saved], nextCursor: null };
    if (command === "change_task") {
      const change = (args as { input: {
        id: string; expectedRevision: number;
        status?: Task["status"]; priority?: Task["priority"]; progress?: number;
      } }).input;
      saved = saved.map((task) => {
        if (task.id !== change.id) return task;
        if (change.expectedRevision !== task.revision) throw Error("Revision conflict");
        const nextStatus = change.progress === 100 ? "completed"
          : change.progress !== undefined ? "in_progress"
          : change.status ?? task.status;
        return {
          ...task,
          status: nextStatus,
          priority: change.priority ?? task.priority,
          progress: change.status === "completed" ? 100 : change.progress ?? task.progress,
          revision: task.revision + 1,
        };
      });
      return saved.find((task) => task.id === change.id);
    }
    throw new Error("Unexpected invoke: " + command);
  });
});
afterEach(cleanup);

describe("Task status and priority", () => {
  it("renders grouped Tasks with distinct priority symbols and accessible labels", async () => {
    saved = [
      { ...base, id: "p", title: "Doing task", status: "in_progress", priority: "high", progress: 20 },
      { ...base, id: "r", title: "Ready task", priority: "very_low" },
      { ...base, id: "d", title: "Done task", status: "completed", priority: "very_high", progress: 100 },
    ];
    render(<App />);
    const sections = await screen.findAllByRole("region", { name: /^(In Progress|Ready|Completed) tasks$/ });
    expect(sections.map((section) => section.getAttribute("aria-label"))).toEqual([
      "In Progress tasks", "Ready tasks", "Completed tasks",
    ]);
    expect(within(screen.getByRole("article", { name: "Doing task" })).getByText("High")).toBeInTheDocument();
    expect(within(screen.getByRole("article", { name: "Ready task" })).getByText("Very low")).toBeInTheDocument();
    expect(within(screen.getByRole("article", { name: "Done task" })).getByText("Very high")).toBeInTheDocument();
  });

  it("allows user to promote a Task priority through the Rust command", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Ready plan" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Priority" }), "very_high");
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("change_task", {
      input: { id: "task-1", expectedRevision: 1, priority: "very_high" },
    }));
    expect(await within(screen.getByRole("article", { name: "Ready plan" })).findByText("Very high", { selector: ".priority-pill" })).toBeInTheDocument();
  });

  it("shows all five labeled priorities with distinct visual symbols", async () => {
    const priorities: Task["priority"][] = [
      "very_low", "low", "medium", "high", "very_high",
    ];
    saved = priorities.map((priority, index) => ({
      ...base, id: `id-${index}`, title: `Priority task ${index}`, priority,
    }));
    render(<App />);
    for (const [index, priority] of priorities.entries()) {
      const row = await screen.findByRole("article", { name: `Priority task ${index}` });
      const marker = row.querySelector(".priority-pill");
      expect(marker).toHaveClass(`priority-pill--${priority}`);
      expect(marker?.querySelector("svg")).not.toBeNull();
      expect(marker?.textContent).toBe(
        ["Very low", "Low", "Medium", "High", "Very high"][index],
      );
    }
  });

  it("sends progress updates to Rust and groups a completed Task at the bottom", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Ready plan" }));

    await user.clear(screen.getByRole("spinbutton", { name: "Progress percentage" }));
    await user.type(screen.getByRole("spinbutton", { name: "Progress percentage" }), "60");
    await user.click(screen.getByRole("button", { name: "Apply progress" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("change_task", {
      input: { id: "task-1", expectedRevision: 1, progress: 60 },
    }));
    expect(await screen.findByRole("region", { name: "In Progress tasks" })).toBeInTheDocument();
    await user.clear(screen.getByRole("spinbutton", { name: "Progress percentage" }));
    await user.type(screen.getByRole("spinbutton", { name: "Progress percentage" }), "100");
    await user.click(screen.getByRole("button", { name: "Apply progress" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("change_task", {
      input: { id: "task-1", expectedRevision: 2, progress: 100 },
    }));
    expect(await screen.findByRole("region", { name: "Completed tasks" })).toBeInTheDocument();
    expect(within(screen.getByRole("article", { name: "Ready plan" })).getByText("Completed", { selector: ".status-pill" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Ready" })).toBeDisabled();
  });

  it("allows sorting by newest while keeping status groups in fixed order", async () => {
    const user = userEvent.setup();
    saved = [
      { ...base, id: "c", title: "Complete", status: "completed", progress: 100 },
      { ...base, id: "r", title: "Ready again" },
      { ...base, id: "i", title: "Started", status: "in_progress", progress: 20 },
    ];
    render(<App />);
    await screen.findByText("Started");
    await user.selectOptions(screen.getByRole("combobox", { name: "Sort tasks" }), "newest");
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("list_tasks", {
      cursor: null, limit: 50, sort: "newest",
    }));
    const groups = screen.getAllByRole("region", { name: /^(In Progress|Ready|Completed) tasks$/ });
    expect(groups.map((node) => node.getAttribute("aria-label"))).toEqual([
      "In Progress tasks", "Ready tasks", "Completed tasks",
    ]);
  });

  it("does not lose text edits when switching priority and does not fake a failed write", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Ready plan" }));
    const input = screen.getByRole("textbox", { name: "Edit title" });
    await user.type(input, " updated");
    expect(screen.getByRole("combobox", { name: "Priority" })).toBeDisabled();
    expect(screen.getByText("Save title/description edits before changing status or priority.")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("change_task", expect.anything());
  });

  it("safely cancels an uncommitted progress draft before switching metadata", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Ready plan" }));
    const progress = screen.getByRole("spinbutton", { name: "Progress percentage" });
    await user.clear(progress);
    await user.type(progress, "25");
    expect(screen.getByRole("combobox", { name: "Priority" })).toBeDisabled();
    expect(screen.getByRole("combobox", { name: "Work status" })).toBeDisabled();
    expect(screen.getByRole("textbox", { name: "Edit title" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Discard progress edit" }));
    expect(progress).toHaveValue(0);
    expect(screen.getByRole("combobox", { name: "Priority" })).toBeEnabled();
    expect(invokeMock).not.toHaveBeenCalledWith("change_task", expect.anything());
  });

});
