import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { Task } from "./lib/task-api";
import type { Subtask } from "./lib/subtask-api";
import App from "./App";

/**
 * Subtasks (owner, batch 4, 2026-11-18): a simple checklist inside a Task,
 * long items welcome; the row shows how many are done ("2/5") and the
 * Task's progress stays the person's own. The core keeps the rules; this
 * fake only stores what it is sent.
 */
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const base: Task = {
  id: "trip", title: "Synthetic trip", description: "", status: "ready", priority: "medium", difficulty: null,
  wontDo: false, progress: 0, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1, createdAt: 100,
  updatedAt: 100, subtasksDone: 0, subtasksTotal: 0,
};
let task: Task;
let items: Subtask[];
let next: number;
let failNext: string | null;

const item = (text: string, done = false): Subtask => ({
  id: `s${next++}`, taskId: "trip", text, done, removed: false, revision: 1, createdAt: 1, updatedAt: 1,
});
const shown = () => items.filter((each) => !each.removed);
const counted = (): Task => ({
  ...task, subtasksTotal: shown().length, subtasksDone: shown().filter((each) => each.done).length,
});
const calls = (name: string) => invokeMock.mock.calls.filter(([command]) => command === name).map(([, args]) => args);

beforeEach(() => {
  window.localStorage.clear();
  next = 1;
  task = { ...base };
  items = [];
  failNext = null;
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    if (failNext && command.endsWith("subtask")) {
      const message = failNext;
      failNext = null;
      throw message;
    }
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: [counted()], nextCursor: null };
      case "list_subtasks": return shown().map((each) => ({ ...each }));
      case "add_subtask": {
        const added = item(body.input.text.trim());
        items.push(added);
        return { ...added };
      }
      case "update_subtask": {
        const { id, expectedRevision, text, done } = body.input;
        const found = items.find((each) => each.id === id)!;
        if (found.revision !== expectedRevision) throw "Revision conflict: the Task has changed; reload before saving";
        Object.assign(found, { text: text ?? found.text, done: done ?? found.done, revision: found.revision + 1 });
        return { ...found };
      }
      case "move_subtask": {
        const { id, index } = body.input;
        const found = items.find((each) => each.id === id)!;
        items = items.filter((each) => each !== found);
        items.splice(index, 0, found);
        found.revision += 1;
        return { ...found };
      }
      case "remove_subtask": {
        const found = items.find((each) => each.id === body.id)!;
        Object.assign(found, { removed: true, revision: found.revision + 1 });
        return { ...found };
      }
      case "restore_subtask": {
        const found = items.find((each) => each.id === body.id)!;
        if (found.revision !== body.expectedRevision) throw "Revision conflict: the Task has changed; reload before saving";
        Object.assign(found, { removed: false, revision: found.revision + 1 });
        return { ...found };
      }
      case "list_task_notes": case "list_progress_entries": return { items: [], nextCursor: null };
      case "list_removed": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      default: return null;
    }
  });
});
afterEach(cleanup);

async function openTrip(user: ReturnType<typeof userEvent.setup>) {
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Open Synthetic trip" }));
  return screen.findByRole("group", { name: "Subtasks" });
}
const row = () => screen.getByRole("article", { name: "Synthetic trip" });

describe("a Task's checklist", () => {
  it("adds items with Return, each at the end, and the row counts them", async () => {
    const user = userEvent.setup();
    const list = await openTrip(user);
    const field = within(list).getByRole("textbox", { name: "Add subtask" });
    await user.type(field, "Book the train{Enter}");
    await user.type(field, "Pack the bag{Enter}");
    expect(calls("add_subtask")).toEqual([
      { input: { taskId: "trip", text: "Book the train" } },
      { input: { taskId: "trip", text: "Pack the bag" } },
    ]);
    expect(within(list).getAllByRole("checkbox").map((box) => box.getAttribute("aria-label")))
      .toEqual(["Book the train", "Pack the bag"]);
    expect(field).toHaveValue("");
    await waitFor(() => expect(within(row()).getByLabelText("0 of 2 subtasks done")).toHaveTextContent("0/2"));
  });

  it("ticks an item done, and the Task's progress is not touched", async () => {
    items = [item("Book the train"), item("Pack the bag")];
    const user = userEvent.setup();
    const list = await openTrip(user);
    await user.click(await within(list).findByRole("checkbox", { name: "Book the train" }));
    expect(calls("update_subtask")).toEqual([{ input: { id: "s1", expectedRevision: 1, done: true } }]);
    await waitFor(() => expect(within(list).getByRole("checkbox", { name: "Book the train" })).toBeChecked());
    await waitFor(() => expect(within(row()).getByLabelText("1 of 2 subtasks done")).toBeInTheDocument());
    expect(calls("change_task")).toEqual([]);
  });

  it("keeps long items whole and saves an edit when leaving it", async () => {
    items = [item("Draft")];
    const long = "A long thought ".repeat(60).trim(); // 899 characters
    const user = userEvent.setup();
    const list = await openTrip(user);
    const text = await within(list).findByRole("textbox", { name: "Subtask text" });
    // The window cuts nothing short: the core alone holds the 10,000-character limit.
    expect(text).not.toHaveAttribute("maxLength");
    await user.clear(text);
    await user.click(text);
    await user.paste(long);
    await user.tab();
    await waitFor(() => expect(calls("update_subtask")).toEqual([{ input: { id: "s1", expectedRevision: 1, text: long } }]));
  });

  it("removes an item, with Undo", async () => {
    items = [item("Book the train"), item("Pack the bag")];
    const user = userEvent.setup();
    const list = await openTrip(user);
    await user.click(await within(list).findByRole("button", { name: "Remove “Book the train”" }));
    expect(calls("remove_subtask")).toEqual([{ id: "s1", expectedRevision: 1 }]);
    await waitFor(() => expect(within(list).queryByRole("checkbox", { name: "Book the train" })).toBeNull());
    await user.click(within(list).getByRole("button", { name: "Undo" }));
    expect(calls("restore_subtask")).toEqual([{ id: "s1", expectedRevision: 2 }]);
    expect(await within(list).findByRole("checkbox", { name: "Book the train" })).toBeInTheDocument();
  });

  it("moves an item with Option and the arrow keys", async () => {
    items = [item("A"), item("B"), item("C")];
    const user = userEvent.setup();
    const list = await openTrip(user);
    const texts = await within(list).findAllByRole("textbox", { name: "Subtask text" });
    await user.click(texts[0]);
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    expect(calls("move_subtask")).toEqual([{ input: { id: "s1", expectedRevision: 1, index: 1 } }]);
    await waitFor(() => expect(within(list).getAllByRole("checkbox").map((box) => box.getAttribute("aria-label")))
      .toEqual(["B", "A", "C"]));
    // The moved item keeps the cursor.
    expect(within(list).getAllByRole("textbox", { name: "Subtask text" })[1]).toHaveFocus();
  });

  it("runs an edit and the next action one after the other, each on the newest revision", async () => {
    items = [item("A"), item("B")];
    const user = userEvent.setup();
    const list = await openTrip(user);
    const [first] = await within(list).findAllByRole("textbox", { name: "Subtask text" });
    await user.type(first, " and more");
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await waitFor(() => expect(calls("move_subtask")).toEqual([{ input: { id: "s1", expectedRevision: 2, index: 1 } }]));
    expect(calls("update_subtask")).toEqual([{ input: { id: "s1", expectedRevision: 1, text: "A and more" } }]);
    // Edited, then ticked straight away: the tick names the revision the edit made.
    const [, moved] = within(list).getAllByRole("textbox", { name: "Subtask text" });
    await user.type(moved, "!");
    await user.click(within(list).getByRole("checkbox", { name: "A and more" }));
    await waitFor(() => expect(calls("update_subtask").slice(-1)[0]).toEqual({ input: { id: "s1", expectedRevision: 4, done: true } }));
    expect(within(list).queryByRole("alert")).toBeNull();
  });

  it("takes a long item with line breaks in the add field, Shift+Return for a new line", async () => {
    const user = userEvent.setup();
    const list = await openTrip(user);
    const field = within(list).getByRole("textbox", { name: "Add subtask" });
    await user.type(field, "First line{Shift>}{Enter}{/Shift}second line{Enter}");
    expect(calls("add_subtask")).toEqual([{ input: { taskId: "trip", text: "First line\nsecond line" } }]);
    expect(field).toHaveValue("");
  });

  it("says why a change was refused and shows the checklist as it is now", async () => {
    items = [item("Book the train")];
    const user = userEvent.setup();
    const list = await openTrip(user);
    failNext = "Revision conflict: the Task has changed; reload before saving";
    await user.click(await within(list).findByRole("checkbox", { name: "Book the train" }));
    expect(await within(list).findByRole("alert")).toHaveTextContent("Revision conflict");
    await waitFor(() => expect(calls("list_subtasks").length).toBeGreaterThan(1));
  });
});

describe("the count on a Task's row", () => {
  it("shows done of all, and nothing for a Task without a checklist", async () => {
    items = [item("A", true), item("B", true), item("C"), item("D"), item("E")];
    render(<App />);
    const counted = await screen.findByLabelText("2 of 5 subtasks done");
    expect(counted).toHaveTextContent("2/5");
    cleanup();
    items = [];
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic trip" });
    expect(screen.queryByLabelText(/subtasks done/)).toBeNull();
  });
});
