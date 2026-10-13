import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { ProgressEntry, Task } from "./lib/task-api";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const firstTask: Task = {
  id: "task-a",
  title: "Review release",
  description: "Document test results",
  status: "in_progress",
  priority: "high",
  progress: 25,
  nextStep: "Review initial QA",
  listId: null,
  dueDate: null,
  tags: [],
  revision: 2,
  createdAt: 123400000,
  updatedAt: 123400000,
};
let task: Task;
let entries: ProgressEntry[];

beforeEach(() => {
  task = { ...firstTask };
  entries = [
    {
      sequence: 1,
      id: "entry-1",
      taskId: "task-a",
      previousStatus: "ready",
      status: "in_progress",
      previousProgress: 0,
      progress: 25,
      note: "Implemented the first part",
      nextStep: "Review initial QA",
      createdAt: 123400000,
    },
  ];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_tasks") return { items: [task], nextCursor: null };
    if (command === "list_progress_entries") {
      return { items: [...entries], nextCursor: null };
    }
    if (command === "record_progress") {
      const input = (
        args as {
          input: {
            id: string;
            expectedRevision: number;
            progress?: number;
            nextStep?: string;
            note?: string;
          };
        }
      ).input;
      if (input.expectedRevision !== task.revision)
        throw Error("Revision conflict");
      const updated: Task = {
        ...task,
        progress: input.progress ?? task.progress,
        status: input.progress === 100 ? "completed" : task.status,
        nextStep: input.nextStep ?? task.nextStep,
        revision: task.revision + 1,
      };
      if (
        (input.note ?? "").trim() ||
        input.progress !== undefined ||
        input.nextStep !== undefined
      ) {
        entries.unshift({
          sequence: 2,
          id: "entry-2",
          taskId: task.id,
          previousStatus: task.status,
          status: updated.status,
          previousProgress: task.progress,
          progress: updated.progress,
          note: (input.note ?? "").trim(),
          nextStep: updated.nextStep,
          createdAt: 123500000,
        });
      }
      task = updated;
      return task;
    }
    throw Error("Unexpected command " + command);
  });
});
afterEach(cleanup);

describe("Next Step and Progress Entries", () => {
  it("shows saved Next Step and recent dated Progress Entries on expanded Tasks", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Open Review release" }),
    );
    expect(screen.getByRole("textbox", { name: "Next step" })).toHaveValue(
      "Review initial QA",
    );
    const history = await screen.findByRole("region", {
      name: "Progress history",
    });
    expect(
      within(history).getByText("Implemented the first part"),
    ).toBeInTheDocument();
    expect(within(history).getByText(/0%.*25%/)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_progress_entries", {
      taskId: "task-a",
      limit: 10,
      before: null,
    });
  });

  it("saves percentage, dated note and Next Step as one revision-safe update and reloads them", async () => {
    const user = userEvent.setup();
    const ui = render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Open Review release" }),
    );
    await user.clear(
      screen.getByRole("spinbutton", { name: "Progress percentage" }),
    );
    await user.type(
      screen.getByRole("spinbutton", { name: "Progress percentage" }),
      "60",
    );
    await user.clear(screen.getByRole("textbox", { name: "Next step" }));
    await user.type(
      screen.getByRole("textbox", { name: "Next step" }),
      "Run regression tests",
    );
    await user.type(
      screen.getByRole("textbox", { name: "Progress note" }),
      "Refined the user interface",
    );
    await user.click(
      screen.getByRole("button", { name: "Save progress update" }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("record_progress", {
        input: {
          id: "task-a",
          expectedRevision: 2,
          progress: 60,
          nextStep: "Run regression tests",
          note: "Refined the user interface",
        },
      }),
    );
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Next step" })).toHaveValue(
        "Run regression tests",
      ),
    );
    const history = await screen.findByRole("region", {
      name: "Progress history",
    });
    expect(
      await within(history).findByText("Refined the user interface"),
    ).toBeInTheDocument();
    expect(entries).toHaveLength(2);

    ui.unmount();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Open Review release" }),
    );
    expect(screen.getByRole("textbox", { name: "Next step" })).toHaveValue(
      "Run regression tests",
    );
    expect(
      await screen.findByText("Refined the user interface"),
    ).toBeInTheDocument();
  });

  it("records a Next Step-only update without inventing a percentage change", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Open Review release" }),
    );
    const step = screen.getByRole("textbox", { name: "Next step" });
    await user.clear(step);
    await user.type(step, "Run nightly build");
    await user.click(
      screen.getByRole("button", { name: "Save progress update" }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("record_progress", {
        input: {
          id: "task-a",
          expectedRevision: 2,
          nextStep: "Run nightly build",
        },
      }),
    );
    expect(task.progress).toBe(25);
    expect(task.status).toBe("in_progress");
    expect(entries).toHaveLength(2);
  });

  it("loads old progress records in bounded pages, only when expanded", async () => {
    entries = Array.from({ length: 11 }, (_, index) => ({
      sequence: 11 - index,
      id: `entry-${11 - index}`,
      taskId: task.id,
      previousStatus: "in_progress" as const,
      status: "in_progress" as const,
      previousProgress: index,
      progress: index + 1,
      note: `History item ${index + 1}`,
      nextStep: "Review",
      createdAt: 123400000 + index,
    }));
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_tasks") return { items: [task], nextCursor: null };
      if (command === "list_progress_entries") {
        const before = (args as { before: number | null }).before;
        return before === null
          ? { items: entries.slice(0, 10), nextCursor: 2 }
          : { items: entries.slice(10), nextCursor: null };
      }
      throw Error("Unexpected command " + command);
    });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Open Review release" });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "list_progress_entries",
      expect.anything(),
    );

    await user.click(
      screen.getByRole("button", { name: "Open Review release" }),
    );
    const history = await screen.findByRole("region", {
      name: "Progress history",
    });
    expect(
      await within(history).findByText("History item 1"),
    ).toBeInTheDocument();
    expect(
      within(history).queryByText("History item 11"),
    ).not.toBeInTheDocument();
    await user.click(
      within(history).getByRole("button", { name: "Load older progress" }),
    );
    expect(
      await within(history).findByText("History item 11"),
    ).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_progress_entries", {
      taskId: "task-a",
      limit: 10,
      before: 2,
    });
    expect(
      within(history).queryByRole("button", { name: "Load older progress" }),
    ).not.toBeInTheDocument();
  });

  it("retains unsaved notes and Next Step on a rejected revision", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_tasks") return { items: [task], nextCursor: null };
      if (command === "list_progress_entries")
        return { items: [], nextCursor: null };
      if (command === "record_progress") throw Error("Revision conflict");
      throw Error("Unexpected command " + command);
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Open Review release" }),
    );
    await user.type(
      screen.getByRole("textbox", { name: "Progress note" }),
      "Keep my work",
    );
    await user.click(
      screen.getByRole("button", { name: "Save progress update" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Revision conflict",
    );
    expect(screen.getByRole("textbox", { name: "Progress note" })).toHaveValue(
      "Keep my work",
    );
    expect(entries).toHaveLength(1);
    expect(screen.getByRole("textbox", { name: "Next step" })).toHaveValue(
      "Review initial QA",
    );
  });

  it("shows a retry action if local history cannot be loaded", async () => {
    let reads = 0;
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_tasks") return { items: [task], nextCursor: null };
      if (command === "list_progress_entries") {
        reads += 1;
        if (reads === 1) throw Error("History temporarily unavailable");
        return { items: [...entries], nextCursor: null };
      }
      throw Error("Unexpected command " + command);
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Open Review release" }),
    );
    const history = await screen.findByRole("region", {
      name: "Progress history",
    });
    expect(await within(history).findByRole("alert")).toHaveTextContent(
      "History temporarily unavailable",
    );
    await user.click(
      within(history).getByRole("button", { name: "Retry history" }),
    );
    expect(
      await within(history).findByText("Implemented the first part"),
    ).toBeInTheDocument();
    expect(reads).toBe(2);
  });
});
