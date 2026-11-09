import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function task(overrides: Partial<Task>): Task {
  return {
    id: "task-1", title: "Untitled", description: "", status: "ready",
    priority: "medium", difficulty: null, progress: 0, nextStep: "", listId: null,
    dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100, ...overrides,
  };
}

type ListArgs = { limit: number; cursor: string | null; sort?: string; difficulty?: string };
type ChangeInput = { id: string; expectedRevision: number; difficulty?: Task["difficulty"] };

let saved: Task[];
let listCalls: ListArgs[];
let changes: ChangeInput[];

beforeEach(() => {
  window.localStorage.clear();
  saved = [
    task({ id: "a", title: "Synthetic essay", difficulty: "hard" }),
    task({ id: "b", title: "Synthetic errand" }),
  ];
  listCalls = [];
  changes = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists") return { items: [], nextCursor: null };
    if (command === "list_notes") return { items: [], nextCursor: null };
    if (command === "list_task_notes") return { items: [], nextCursor: null };
    if (command === "list_progress_entries") return { items: [], nextCursor: null };
    if (command === "list_tasks") {
      listCalls.push(args as ListArgs);
      return { items: [...saved], nextCursor: null };
    }
    if (command === "change_task") {
      const input = (args as { input: ChangeInput }).input;
      changes.push(input);
      saved = saved.map((item) => item.id !== input.id ? item : {
        ...item,
        difficulty: input.difficulty === undefined ? item.difficulty : input.difficulty,
        revision: item.revision + 1,
      });
      return saved.find((item) => item.id === input.id);
    }
    throw new Error("Unexpected command: " + command);
  });
});

afterEach(cleanup);

describe("Difficulty", () => {
  it("shows a Difficulty pill beside Priority only when one is set", async () => {
    render(<App />);
    const essay = await screen.findByRole("article", { name: "Synthetic essay" });
    const pill = within(essay).getByText("Hard");
    expect(pill).toHaveClass("difficulty-pill");
    // Read aloud as Difficulty, so it is not mistaken for a Priority.
    expect(pill).toHaveTextContent("Difficulty Hard");
    expect(pill.previousElementSibling).toHaveClass("priority-pill");

    const errand = screen.getByRole("article", { name: "Synthetic errand" });
    expect(errand.querySelector(".difficulty-pill")).toBeNull();
  });

  it("sets and clears Difficulty from the editor through the core's change", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic errand" }));
    const select = screen.getByRole("combobox", { name: "Difficulty" });
    expect(select).toHaveValue("");
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "No difficulty", "Very easy", "Easy", "Medium", "Hard", "Very hard",
    ]);

    await user.selectOptions(select, "very_hard");
    await waitFor(() => expect(changes).toEqual([
      { id: "b", expectedRevision: 1, difficulty: "very_hard" },
    ]));
    const errand = screen.getByRole("article", { name: "Synthetic errand" });
    await waitFor(() =>
      expect(errand.querySelector(".difficulty-pill")).toHaveTextContent("Difficulty Very hard"));

    await user.selectOptions(screen.getByRole("combobox", { name: "Difficulty" }), "");
    await waitFor(() => expect(changes[1]).toEqual({ id: "b", expectedRevision: 2, difficulty: null }));
    await waitFor(() => expect(errand.querySelector(".difficulty-pill")).toBeNull());
  });

  it("sorts hardest first and filters by Difficulty in the core", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic essay" });
    const sort = screen.getByRole("combobox", { name: "Sort tasks" });
    expect(within(sort).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "Highest priority", "Hardest first", "Newest first",
    ]);

    await user.selectOptions(sort, "difficulty");
    await waitFor(() => expect(listCalls[listCalls.length - 1]).toEqual({ limit: 50, cursor: null, sort: "difficulty" }));

    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by difficulty" }), "hard");
    await waitFor(() => expect(listCalls[listCalls.length - 1]).toEqual({
      limit: 50, cursor: null, sort: "difficulty", difficulty: "hard",
    }));
    expect(screen.getByRole("button", { name: "Clear search and filters" })).toBeInTheDocument();
  });
});
