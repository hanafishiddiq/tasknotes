/**
 * Ticket 16: a Task's round check is a button. It completes a Ready or In
 * Progress Task, reopens a Completed one and clears Won't Do, always through
 * the core's own status change. A small stand-in for the core sits behind the
 * Tauri seam; its rules are the core's and are tested in Rust. Synthetic data only.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, WorkStatus } from "./lib/task-api";
import { COMPLETION_UNDO_MS, useCompletionUndo } from "./lib/use-completion-undo";
import { FakeAudioContext } from "./test/fake-audio";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

type Stored = Task & { previousIncomplete: number };
type ChangeInput = {
  id: string; expectedRevision: number; status?: WorkStatus; progress?: number; wontDo?: boolean; priority?: Task["priority"];
};

let saved: Stored[];
let changes: ChangeInput[];

function task(id: string, fields: Partial<Task> = {}): Stored {
  const made: Task = {
    id, title: `Synthetic ${id}`, description: "", status: "ready", priority: "medium", difficulty: null,
    progress: 0, wontDo: false, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1,
    createdAt: 100, updatedAt: 100, ...fields,
  };
  return { ...made, previousIncomplete: made.progress < 100 ? made.progress : 0 };
}

const strip = ({ previousIncomplete: _kept, ...item }: Stored): Task => ({ ...item });
const GROUP = { in_progress: 0, ready: 1, completed: 2 } as const;

/** The core's transitions, as far as a window can see them (ADR 0006, ADR 0008). */
function applyChange(item: Stored, input: ChangeInput): Stored {
  if (input.expectedRevision !== item.revision) throw "Revision conflict: the Task has changed; reload before saving";
  const next = { ...item, revision: item.revision + 1 };
  if (input.priority) return { ...next, priority: input.priority };
  if (input.wontDo === false) {
    return { ...next, status: item.wontDoFrom ?? item.status, wontDo: false, wontDoFrom: undefined };
  }
  if (input.status === "completed") return { ...next, status: "completed", progress: 100, wontDo: false, wontDoFrom: undefined };
  if (input.status === "in_progress") {
    const progress = input.progress ?? item.previousIncomplete;
    return { ...next, status: "in_progress", progress, previousIncomplete: progress, wontDo: false, wontDoFrom: undefined };
  }
  if (input.status === "ready") {
    if (item.status !== "ready") throw "Ready is only available for work that has not started";
    return next;
  }
  throw "Unexpected change: " + JSON.stringify(input);
}

beforeEach(() => {
  window.localStorage.clear();
  FakeAudioContext.reset();
  vi.stubGlobal("AudioContext", FakeAudioContext);
  saved = [
    task("draft", { status: "in_progress", progress: 40 }),
    task("idea"),
    task("finished", { status: "completed", progress: 100 }),
    task("shelved", { status: "completed", progress: 30, wontDo: true, wontDoFrom: "in_progress" }),
  ];
  // Reopening the finished one brings back the progress it had before.
  saved[2].previousIncomplete = 70;
  changes = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists":
      case "list_notes":
      case "list_task_notes":
      case "list_progress_entries":
        return { items: [], nextCursor: null };
      case "list_tasks":
        return { items: [...saved].sort((a, b) => GROUP[a.status] - GROUP[b.status]).map(strip), nextCursor: null };
      case "change_task": {
        const input = body.input as ChangeInput;
        changes.push(input);
        const changed = applyChange(saved.find((item) => item.id === input.id)!, input);
        saved = saved.map((item) => item.id === input.id ? changed : item);
        return strip(changed);
      }
      case "create_task": {
        const made = task(`new-${saved.length}`, { title: body.input.title.trim() });
        saved.push(made);
        return strip(made);
      }
      case "quit_ready": return null;
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const row = (title: string) => screen.getByRole("article", { name: title });
const group = (label: string) => screen.getByRole("region", { name: `${label} tasks` });

describe("the check of a Task that is not done", () => {
  it("completes an In Progress Task at 100% through the core's status change, with the completion cue", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic draft" });
    const context = FakeAudioContext.latest();
    const before = context.oscillators.length;

    await user.click(within(row("Synthetic draft")).getByRole("button", { name: "Complete “Synthetic draft”" }));

    await waitFor(() => expect(within(group("Completed")).getByRole("article", { name: "Synthetic draft" })).toBeInTheDocument());
    expect(changes).toEqual([{ id: "draft", expectedRevision: 1, status: "completed" }]);
    expect(saved[0]).toMatchObject({ status: "completed", progress: 100 });
    await waitFor(() => expect(context.oscillators.length).toBeGreaterThan(before));
    // The row stays closed: the check is not the row's open button.
    expect(screen.getByRole("button", { name: "Open Synthetic draft" })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("region", { name: "Task details" })).toBeNull();
  });
});

describe("the check of a Task that is done", () => {
  it("reopens a Completed Task to the progress it had before, by the core's reopen rule", async () => {
    const user = userEvent.setup();
    render(<App />);
    const finished = await screen.findByRole("article", { name: "Synthetic finished" });
    const context = FakeAudioContext.latest();
    const before = context.oscillators.length;

    await user.click(within(finished).getByRole("button", { name: "Reopen “Synthetic finished”" }));

    await waitFor(() => expect(within(group("In Progress")).getByRole("article", { name: "Synthetic finished" })).toBeInTheDocument());
    expect(changes).toEqual([{ id: "finished", expectedRevision: 1, status: "in_progress" }]);
    expect(saved[2]).toMatchObject({ status: "in_progress", progress: 70 });
    // Its check now completes it again.
    expect(within(row("Synthetic finished")).getByRole("button", { name: "Complete “Synthetic finished”" })).toBeInTheDocument();
    // Reopening is not finishing: no cue, and no Undo bar.
    expect(context.oscillators).toHaveLength(before);
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();
  });

  it("clears Won't Do back to the status the Task had before it was set aside (ADR 0008)", async () => {
    const user = userEvent.setup();
    render(<App />);
    const shelved = await screen.findByRole("article", { name: "Synthetic shelved" });

    await user.click(within(shelved).getByRole("button", { name: "Clear Won't do for “Synthetic shelved”" }));

    await waitFor(() => expect(within(group("In Progress")).getByRole("article", { name: "Synthetic shelved" })).toBeInTheDocument());
    expect(changes).toEqual([{ id: "shelved", expectedRevision: 1, wontDo: false }]);
    // It keeps the progress it had.
    expect(saved[3]).toMatchObject({ status: "in_progress", progress: 30, wontDo: false });
    expect(row("Synthetic shelved")).not.toHaveClass("task-card--wont-do");
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();
  });
});

describe("the check as a control", () => {
  it("is reached with Tab and pressed with Space or Return, leaving the row closed", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic idea" });
    const check = within(row("Synthetic idea")).getByRole("button", { name: "Complete “Synthetic idea”" });
    expect(check).toHaveAttribute("type", "button");

    // Right before the row's open button.
    screen.getByRole("button", { name: "Open Synthetic draft" }).focus();
    await user.tab();
    expect(check).toHaveFocus();
    await user.keyboard(" ");
    await waitFor(() => expect(saved[1]).toMatchObject({ status: "completed", progress: 100 }));
    expect(screen.getByRole("button", { name: "Open Synthetic idea" })).toHaveAttribute("aria-expanded", "false");

    const reopen = await within(row("Synthetic idea")).findByRole("button", { name: "Reopen “Synthetic idea”" });
    reopen.focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(saved[1].status).toBe("in_progress"));
  });

  it("waits while the Task has unsaved edits or a save is on its way", async () => {
    let answer = () => {};
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic draft" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "More");
    const check = within(row("Synthetic draft")).getByRole("button", { name: "Complete “Synthetic draft”" });
    expect(check).toBeDisabled();

    // The save in flight holds it too.
    const answering = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "update_task") {
        await new Promise<void>((resolve) => { answer = resolve; });
        const input = (args as { input: Task }).input;
        const found = saved.find((item) => item.id === input.id)!;
        Object.assign(found, { description: input.description, revision: found.revision + 1 });
        return strip(found);
      }
      return answering(command, args);
    });
    await user.click(screen.getByRole("button", { name: /Save changes/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: /Saving/ })).toBeInTheDocument());
    expect(within(row("Synthetic draft")).getByRole("button", { name: "Complete “Synthetic draft”" })).toBeDisabled();
    answer();
    await waitFor(() => expect(within(row("Synthetic draft")).getByRole("button", { name: "Complete “Synthetic draft”" })).toBeEnabled());
    expect(changes).toEqual([]);
  });
});

describe("the Undo offer after completing from the check, on its own", () => {
  it("stays about 6 seconds, held while the pointer or focus is on it, and a newer one replaces it", () => {
    vi.useFakeTimers();
    const first = vi.fn(async () => {});
    const second = vi.fn(async () => {});
    const { result } = renderHook(() => useCompletionUndo());
    expect(result.current.taskId).toBeNull();
    act(() => result.current.offer("a", 2, first));
    expect(result.current.taskId).toBe("a");
    expect(result.current.revision).toBe(2);

    act(() => vi.advanceTimersByTime(4000));
    act(() => result.current.offer("b", 2, second));
    act(() => vi.advanceTimersByTime(COMPLETION_UNDO_MS - 1));
    expect(result.current.taskId).toBe("b");
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.taskId).toBeNull();
    expect(COMPLETION_UNDO_MS).toBe(6000);

    act(() => result.current.offer("c", 2, second));
    act(() => vi.advanceTimersByTime(5000));
    act(() => result.current.hold("hover", true));
    act(() => vi.advanceTimersByTime(60_000));
    act(() => result.current.hold("focus", true));
    act(() => result.current.hold("hover", false));
    act(() => vi.advanceTimersByTime(60_000));
    expect(result.current.taskId).toBe("c");
    act(() => result.current.hold("focus", false));
    act(() => vi.advanceTimersByTime(999));
    expect(result.current.taskId).toBe("c");
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.taskId).toBeNull();
    expect(first).not.toHaveBeenCalled();
    expect(second).not.toHaveBeenCalled();
  });

  it("runs the newest offer once and goes; hiding for another Task leaves it", () => {
    const undo = vi.fn(async () => {});
    const { result } = renderHook(() => useCompletionUndo());
    act(() => result.current.offer("a", 2, undo));
    act(() => result.current.hide("other"));
    expect(result.current.taskId).toBe("a");
    act(() => result.current.run());
    expect(undo).toHaveBeenCalledTimes(1);
    expect(result.current.taskId).toBeNull();
    act(() => result.current.run());
    expect(undo).toHaveBeenCalledTimes(1);

    act(() => result.current.offer("b", 2, undo));
    act(() => result.current.hide("b"));
    expect(result.current.taskId).toBeNull();
    act(() => result.current.offer("c", 2, undo));
    act(() => result.current.hide());
    expect(result.current.taskId).toBeNull();
  });
});

describe("Undo after completing from the check", () => {
  it("shows “Task completed” with Undo, which restores the status and progress through the core, revision checked", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(within(await screen.findByRole("article", { name: "Synthetic draft" }))
      .getByRole("button", { name: "Complete “Synthetic draft”" }));

    const bar = await screen.findByRole("status", { name: "Task completed" });
    expect(bar).toHaveClass("undo-bar");
    expect(bar).toHaveTextContent("Task completed");
    await user.click(within(bar).getByRole("button", { name: "Undo" }));

    await waitFor(() => expect(within(group("In Progress")).getByRole("article", { name: "Synthetic draft" })).toBeInTheDocument());
    // Named against the revision the completion made, so a change since is refused.
    expect(changes[1]).toEqual({ id: "draft", expectedRevision: 2, status: "in_progress", progress: 40 });
    expect(saved[0]).toMatchObject({ status: "in_progress", progress: 40, revision: 3 });
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();
  });

  it("brings a Task completed from Ready back In Progress at 0%, as the core opens Ready only to work not yet started", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(within(await screen.findByRole("article", { name: "Synthetic idea" }))
      .getByRole("button", { name: "Complete “Synthetic idea”" }));
    const bar = await screen.findByRole("status", { name: "Task completed" });
    await user.click(within(bar).getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(within(group("In Progress")).getByRole("article", { name: "Synthetic idea" })).toBeInTheDocument());
    expect(changes[1]).toEqual({ id: "idea", expectedRevision: 2, status: "in_progress", progress: 0 });
    expect(saved[1]).toMatchObject({ status: "in_progress", progress: 0 });
  });

  it("says so in the Task view when the core refuses the Undo", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(within(await screen.findByRole("article", { name: "Synthetic draft" }))
      .getByRole("button", { name: "Complete “Synthetic draft”" }));
    const bar = await screen.findByRole("status", { name: "Task completed" });
    // Something else changed it since, as the CLI or an AI client might.
    saved[0].revision = 9;
    await user.click(within(bar).getByRole("button", { name: "Undo" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/Revision conflict/);
    expect(saved[0]).toMatchObject({ status: "completed", progress: 100 });
  });

  it("goes when the Task changes again, on leaving the Task view, and is not offered by the status control", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(within(await screen.findByRole("article", { name: "Synthetic draft" }))
      .getByRole("button", { name: "Complete “Synthetic draft”" }));
    await screen.findByRole("status", { name: "Task completed" });
    // Reopened from its check: the offer named a revision that is gone.
    await user.click(await within(row("Synthetic draft")).findByRole("button", { name: "Reopen “Synthetic draft”" }));
    await waitFor(() => expect(saved[0].status).toBe("in_progress"));
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();

    // Any other change to it, here or elsewhere, gives it a newer revision too.
    await user.click(within(row("Synthetic draft")).getByRole("button", { name: "Complete “Synthetic draft”" }));
    await screen.findByRole("status", { name: "Task completed" });
    await user.click(screen.getByRole("button", { name: "Open Synthetic draft" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Priority" }), "high");
    await waitFor(() => expect(saved[0].priority).toBe("high"));
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Close Synthetic draft" }));

    await user.click(within(row("Synthetic idea")).getByRole("button", { name: "Complete “Synthetic idea”" }));
    await screen.findByRole("status", { name: "Task completed" });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "All tasks" }));
    await user.click(await screen.findByRole("button", { name: "Open Synthetic draft" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Work status" }), "completed");
    await waitFor(() => expect(saved[0].status).toBe("completed"));
    expect(screen.queryByRole("status", { name: "Task completed" })).toBeNull();
  });

  it("takes the place of the Removed notice, whose move ⌘Z can still take back", async () => {
    const user = userEvent.setup();
    let removed: Stored[] = [];
    const answering = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation(async (command, args) => {
      if (command !== "set_task_removed") return answering(command, args);
      const input = (args as { input: { id: string; removed: boolean } }).input;
      const found = [...saved, ...removed].find((item) => item.id === input.id)!;
      found.revision += 1;
      saved = saved.filter((item) => item !== found);
      removed = removed.filter((item) => item !== found);
      (input.removed ? removed : saved).push(found);
      return { ...strip(found), removed: input.removed };
    });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic finished" }));
    await user.click(screen.getByRole("button", { name: "Move to Removed" }));
    await screen.findByRole("status", { name: "Moved to Removed" });

    await user.click(within(row("Synthetic idea")).getByRole("button", { name: "Complete “Synthetic idea”" }));
    await screen.findByRole("status", { name: "Task completed" });
    expect(screen.queryByRole("status", { name: "Moved to Removed" })).toBeNull();

    // ⌘Z is still the latest move to Removed, never the completion.
    act(() => (document.activeElement as HTMLElement | null)?.blur());
    await user.keyboard("{Meta>}z{/Meta}");
    await screen.findByRole("article", { name: "Synthetic finished" });
    expect(removed).toEqual([]);
    expect(saved.find((item) => item.id === "idea")).toMatchObject({ status: "completed" });
  });
});

describe("the list around the check", () => {
  it("asks for no scroll when a check completes a Task or Undo brings it back, and the new Task toast stays", async () => {
    saved = Array.from({ length: 30 }, (_, at) => task(`t${at + 1}`, { status: "in_progress", progress: 10 }));
    const scrollTo = vi.fn(function (this: HTMLElement, options?: ScrollToOptions | number) {
      if (typeof options === "object" && options.top !== undefined) this.scrollTop = options.top;
    });
    const original = HTMLElement.prototype.scrollTo;
    HTMLElement.prototype.scrollTo = scrollTo as unknown as typeof original;
    try {
      const user = userEvent.setup();
      render(<App />);
      await screen.findByRole("article", { name: "Synthetic t1" });
      const area = document.querySelector<HTMLElement>(".main-scroll")!;
      Object.defineProperty(area, "scrollHeight", { configurable: true, value: 5000 });
      Object.defineProperty(area, "clientHeight", { configurable: true, value: 500 });
      await user.type(screen.getByRole("textbox", { name: "Task title" }), "Synthetic fresh");
      await user.click(screen.getByRole("button", { name: "Add task" }));
      const toast = await screen.findByRole("button", { name: "Task added · Click to edit" });
      const before = scrollTo.mock.calls.length;

      await user.click(within(row("Synthetic t2")).getByRole("button", { name: "Complete “Synthetic t2”" }));
      const bar = await screen.findByRole("status", { name: "Task completed" });
      await waitFor(() => expect(within(group("Completed")).getByRole("article", { name: "Synthetic t2" })).toBeInTheDocument());
      await user.click(within(bar).getByRole("button", { name: "Undo" }));
      await waitFor(() => expect(within(group("In Progress")).getByRole("article", { name: "Synthetic t2" })).toBeInTheDocument());

      const moved = scrollTo.mock.calls.slice(before).filter(([options]) => typeof options === "object" && (options.top ?? 0) > 0);
      expect(moved).toEqual([]);
      expect(area.scrollTop).toBe(0);
      expect(toast).toBeInTheDocument();
    } finally {
      HTMLElement.prototype.scrollTo = original;
    }
  });
});
