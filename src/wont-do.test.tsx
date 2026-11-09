import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { statusChoiceArgs, statusChoiceOf, type ProgressEntry, type Task, type WorkStatus } from "./lib/task-api";
import { STATUS_CHOICES, statusChoiceLabel } from "./lib/task-visuals";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function task(overrides: Partial<Task>): Task {
  return {
    id: "task-1", title: "Untitled", description: "", status: "ready",
    priority: "medium", difficulty: null, progress: 0, wontDo: false, nextStep: "", listId: null,
    dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100, ...overrides,
  };
}

type ListArgs = { limit: number; cursor: string | null; status?: string; wontDo?: boolean };
type ChangeInput = { id: string; expectedRevision: number; status?: WorkStatus; wontDo?: boolean };

let saved: Task[];
let listCalls: ListArgs[];
let changes: ChangeInput[];
let history: ProgressEntry[];

/** A stand-in for the core's rules, enough for the window's side of them. */
function applyChange(item: Task, input: ChangeInput): Task {
  if (input.wontDo) {
    return { ...item, status: "completed", wontDo: true, wontDoFrom: item.status === "ready" ? "ready" : "in_progress" };
  }
  if (input.status === "completed") return { ...item, status: "completed", progress: 100, wontDo: false };
  if (input.status) return { ...item, status: input.status, wontDo: false };
  return item;
}

beforeEach(() => {
  window.localStorage.clear();
  saved = [
    task({ id: "a", title: "Synthetic draft", status: "in_progress", progress: 40, createdAt: 300 }),
    task({ id: "b", title: "Synthetic idea", createdAt: 200 }),
    task({
      id: "c", title: "Synthetic set aside", status: "completed", progress: 30, wontDo: true,
      wontDoFrom: "in_progress", createdAt: 100,
    }),
    task({ id: "d", title: "Synthetic finished", status: "completed", progress: 100, createdAt: 50 }),
  ];
  listCalls = [];
  changes = [];
  history = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_notes") return { items: [], nextCursor: null };
    if (command === "list_task_notes") return { items: [], nextCursor: null };
    if (command === "list_progress_entries") return { items: history, nextCursor: null };
    if (command === "list_tasks") {
      listCalls.push(args as ListArgs);
      return { items: [...saved], nextCursor: null };
    }
    if (command === "change_task") {
      const input = (args as { input: ChangeInput }).input;
      changes.push(input);
      saved = saved.map((item) => item.id !== input.id ? item : {
        ...applyChange(item, input), revision: item.revision + 1,
      });
      return saved.find((item) => item.id === input.id);
    }
    throw new Error("Unexpected command: " + command);
  });
});

afterEach(cleanup);

describe("Won't Do", () => {
  it("marks a Won't Do Task in the Completed group with a struck title and a label", async () => {
    render(<App />);
    const aside = await screen.findByRole("article", { name: "Synthetic set aside" });
    expect(aside).toHaveClass("task-card--completed", "task-card--wont-do");
    // The progress it kept shows on the collapsed row.
    expect(within(aside).getByText("Won't do · 30%")).toHaveClass("wont-do-label");
    expect(aside.querySelector(".task-check--wont-do svg.lucide-x")).not.toBeNull();
    // Not the finished tick.
    expect(aside.querySelector("svg.lucide-check")).toBeNull();

    const finished = screen.getByRole("article", { name: "Synthetic finished" });
    expect(finished).not.toHaveClass("task-card--wont-do");
    expect(within(finished).queryByText(/Won't do/)).toBeNull();
  });

  it("offers Won't do in the status control and sends it to the core", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic draft" }));
    const select = screen.getByRole("combobox", { name: "Work status" });
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "In Progress", "Ready", "Completed", "Won't do",
    ]);
    await user.selectOptions(select, "wont_do");
    await waitFor(() => expect(changes).toEqual([{ id: "a", expectedRevision: 1, wontDo: true }]));
    const draft = screen.getByRole("article", { name: "Synthetic draft" });
    await waitFor(() => expect(draft).toHaveClass("task-card--wont-do"));
    expect(screen.getByRole("combobox", { name: "Work status" })).toHaveValue("wont_do");
  });

  it("reopens a Won't Do Task through the status control", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic set aside" }));
    const select = screen.getByRole("combobox", { name: "Work status" });
    expect(select).toHaveValue("wont_do");
    // It has progress, so only In Progress reopens it; Ready stays closed.
    expect(within(select).getByRole("option", { name: "Ready" })).toBeDisabled();
    await user.selectOptions(select, "in_progress");
    await waitFor(() => expect(changes).toEqual([{ id: "c", expectedRevision: 1, status: "in_progress" }]));
    const aside = screen.getByRole("article", { name: "Synthetic set aside" });
    await waitFor(() => expect(aside).not.toHaveClass("task-card--wont-do"));
  });

  it("offers Ready only to a Won't Do Task that never started, and says why a finished one can't be set aside", async () => {
    saved = [
      task({ id: "z", title: "Synthetic unstarted", status: "completed", progress: 0, wontDo: true, wontDoFrom: "ready" }),
      task({ id: "i", title: "Synthetic idle", status: "completed", progress: 0, wontDo: true, wontDoFrom: "in_progress" }),
      task({ id: "d", title: "Synthetic finished", status: "completed", progress: 100 }),
    ];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic unstarted" }));
    const select = screen.getByRole("combobox", { name: "Work status" });
    expect(within(select).getByRole("option", { name: "Ready" })).toBeEnabled();

    // Started, then set aside at 0%: it stays started work (ADR 0006).
    await user.click(screen.getByRole("button", { name: "Open Synthetic idle" }));
    const idle = screen.getByRole("combobox", { name: "Work status" });
    expect(within(idle).getByRole("option", { name: "Ready" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Open Synthetic finished" }));
    const finished = screen.getByRole("combobox", { name: "Work status" });
    const option = within(finished).getByRole("option", { name: "Won't do" });
    expect(option).toBeDisabled();
    const why = "Reopen it first: a finished Task can't be set aside";
    expect(option).toHaveAttribute("title", why);
    expect(option).toHaveAttribute("aria-description", why);
    // A Task that can be set aside gets no such note.
    await user.click(screen.getByRole("button", { name: "Open Synthetic unstarted" }));
    const open = within(screen.getByRole("combobox", { name: "Work status" })).getByRole("option", { name: "Won't do" });
    expect(open).not.toHaveAttribute("title");
  });

  it("filters by Won't Do in the core", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic set aside" });
    const filter = screen.getByRole("combobox", { name: "Filter by status" });
    expect(within(filter).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "Any status", "In Progress", "Ready", "Completed", "Won't do",
    ]);
    await user.selectOptions(filter, "wont_do");
    await waitFor(() => expect(listCalls[listCalls.length - 1]).toEqual({
      limit: 50, cursor: null, wontDo: true,
    }));
    expect(screen.getByRole("button", { name: "Clear search and filters" })).toBeInTheDocument();
  });

  it("names Won't do in the progress history", async () => {
    history = [{
      sequence: 2, id: "p2", taskId: "c", previousStatus: "in_progress", status: "completed",
      previousProgress: 30, progress: 30, previousWontDo: false, wontDo: true,
      note: "", nextStep: "", createdAt: 200,
    }];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic set aside" }));
    const entry = await screen.findByText("Won't do", { selector: ".history-entry-summary span" });
    expect(entry).toBeInTheDocument();
  });
});

describe("The status choice mapping", () => {
  it("maps Won't do to the core's flag and every other choice to its status, both ways", () => {
    expect(STATUS_CHOICES).toEqual(["in_progress", "ready", "completed", "wont_do"]);
    expect(STATUS_CHOICES.map(statusChoiceLabel)).toEqual(["In Progress", "Ready", "Completed", "Won't do"]);
    expect(statusChoiceArgs("wont_do")).toEqual({ wontDo: true });
    expect(statusChoiceArgs("completed")).toEqual({ status: "completed" });
    expect(statusChoiceOf({ status: "completed", wontDo: true })).toBe("wont_do");
    expect(statusChoiceOf({ status: "completed", wontDo: false })).toBe("completed");
    expect(statusChoiceOf({ status: "ready", wontDo: false })).toBe("ready");
  });
});
