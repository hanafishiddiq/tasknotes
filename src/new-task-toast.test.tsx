import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";
import App from "./App";
import NewTaskToast from "./components/NewTaskToast";
import VirtualTaskList from "./components/VirtualTaskList";
import { TOAST_FADE_MS, TOAST_MS, FLASH_MS, useNewTaskToast } from "./lib/use-new-task-toast";
import type { Task } from "./lib/task-api";
import { rules } from "./test/stylesheet";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

const TOAST = "Task added · Click to edit";

/** Synthetic fixtures only, behind the Tauri seam. */
let tasks: Task[];
let created: number;
/** Tasks moved to Removed: the store hides them from every list. */
let removed: Set<string>;

function task(n: number, fields: Partial<Task> = {}): Task {
  return {
    id: `task-${n}`, title: `Synthetic task ${n}`, description: "", status: "ready",
    priority: "medium", difficulty: null, wontDo: false, progress: 0, nextStep: "", revision: 1, createdAt: 100 + n,
    updatedAt: 100 + n, listId: null, dueDate: null, tags: [], ...fields,
  };
}

beforeEach(() => {
  tasks = [];
  created = 0;
  removed = new Set();
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": {
        // In Progress first, as the core orders them; new Tasks are Ready, so they land below.
        const items = [...tasks]
          .sort((a, b) => Number(b.status === "in_progress") - Number(a.status === "in_progress"))
          .filter((item) => !removed.has(item.id))
          .filter((item) => !body.taskId || item.id === body.taskId)
          .filter((item) => !body.text || item.title.toLowerCase().includes(String(body.text).toLowerCase()));
        return { items: items.slice(0, body.limit ?? 50), nextCursor: null };
      }
      case "create_task": {
        created += 1;
        const made = task(1000 + created, { title: body.input.title.trim(), description: body.input.description });
        tasks.push(made);
        return made;
      }
      case "set_task_removed": {
        const found = tasks.find((item) => item.id === body.input.id)!;
        if (body.input.removed) removed.add(found.id);
        else removed.delete(found.id);
        found.revision += 1;
        return { ...found, removed: body.input.removed };
      }
      case "list_progress_entries":
      case "list_task_notes":
      case "list_notes":
      case "list_note_folders":
      case "list_removed":
        return { items: [], nextCursor: null };
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

async function addTask(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.type(screen.getByRole("textbox", { name: "Task title" }), title);
  await user.click(screen.getByRole("button", { name: "Add task" }));
  await screen.findByRole("article", { name: title });
}

/** The toast lifecycle on its own: one hook, one component, fake timers. */
function Harness({ onOpen = () => undefined }: { onOpen?: (taskId: string) => void }) {
  const toast = useNewTaskToast();
  const [n, setN] = useState(0);
  return (
    <>
      <button type="button" onClick={() => { setN(n + 1); toast.added(`task-${n + 1}`); }}>Add</button>
      <output data-testid="flash">{toast.flashId ?? ""}</output>
      <NewTaskToast toast={toast} onOpen={onOpen} />
    </>
  );
}

describe("the new Task toast on its own", () => {
  it("shows for about 6 seconds, then fades out and leaves", () => {
    vi.useFakeTimers();
    render(<Harness />);
    expect(screen.queryByRole("button", { name: TOAST })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("button", { name: TOAST })).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(TOAST_MS - 1));
    expect(screen.getByRole("button", { name: TOAST })).not.toHaveClass("task-toast--leaving");
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByRole("button", { name: TOAST })).toHaveClass("task-toast--leaving");
    act(() => vi.advanceTimersByTime(TOAST_FADE_MS));
    expect(screen.queryByRole("button", { name: TOAST })).toBeNull();
    expect(TOAST_MS).toBe(6000);
  });

  it("pauses while hovered or focused and resumes with the time that was left", () => {
    vi.useFakeTimers();
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    const toast = screen.getByRole("button", { name: TOAST });

    act(() => vi.advanceTimersByTime(4000));
    fireEvent.mouseEnter(toast);
    act(() => vi.advanceTimersByTime(60_000));
    expect(toast).not.toHaveClass("task-toast--leaving");

    fireEvent.focus(toast);
    fireEvent.mouseLeave(toast);
    act(() => vi.advanceTimersByTime(60_000));
    expect(toast).not.toHaveClass("task-toast--leaving");

    fireEvent.blur(toast);
    act(() => vi.advanceTimersByTime(1999));
    expect(toast).not.toHaveClass("task-toast--leaving");
    act(() => vi.advanceTimersByTime(1));
    expect(toast).toHaveClass("task-toast--leaving");
  });

  it("keeps one toast for several adds, pointing at the latest, with a fresh 6 seconds", () => {
    vi.useFakeTimers();
    const onOpen = vi.fn();
    render(<Harness onOpen={onOpen} />);
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    act(() => vi.advanceTimersByTime(5000));
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getAllByRole("button", { name: TOAST })).toHaveLength(1);

    act(() => vi.advanceTimersByTime(5000));
    expect(screen.getByRole("button", { name: TOAST })).not.toHaveClass("task-toast--leaving");
    fireEvent.click(screen.getByRole("button", { name: TOAST }));
    expect(onOpen).toHaveBeenCalledWith("task-3");
    expect(screen.queryByRole("button", { name: TOAST })).toBeNull();
  });

  it("flashes the opened Task briefly, as view state only", () => {
    vi.useFakeTimers();
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.click(screen.getByRole("button", { name: TOAST }));
    expect(screen.getByTestId("flash")).toHaveTextContent("task-1");
    act(() => vi.advanceTimersByTime(FLASH_MS));
    expect(screen.getByTestId("flash")).toHaveTextContent("");
    expect(window.localStorage.length).toBe(0);
  });

  it("is announced politely and never takes focus", () => {
    render(<Harness />);
    const add = screen.getByRole("button", { name: "Add" });
    add.focus();
    fireEvent.click(add);
    const toast = screen.getByRole("button", { name: TOAST });
    expect(toast.closest("[aria-live]")).toHaveAttribute("aria-live", "polite");
    expect(add).toHaveFocus();
  });
});

describe("the new Task toast in the Task view", () => {
  it("appears after a Task is added, without moving the list or taking focus", async () => {
    tasks = Array.from({ length: 30 }, (_, i) => task(i + 1, { status: "in_progress" }));
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    const area = document.querySelector<HTMLElement>(".main-scroll")!;
    const scrollTo = vi.fn();
    area.scrollTo = scrollTo;
    area.scrollTop = 0;

    await addTask(user, "Fresh idea");
    expect(await screen.findByRole("button", { name: TOAST })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Task title" })).toHaveFocus();
    expect(scrollTo).not.toHaveBeenCalled();
    expect(area.scrollTop).toBe(0);
  });

  it("clicking it scrolls to the new Task, opens it and flashes it", async () => {
    tasks = Array.from({ length: 30 }, (_, i) => task(i + 1, { status: "in_progress" }));
    const scrollTo = vi.fn(function (this: HTMLElement, options?: ScrollToOptions | number) {
      if (typeof options === "object" && options.top !== undefined) this.scrollTop = options.top;
    });
    const original = HTMLElement.prototype.scrollTo;
    HTMLElement.prototype.scrollTo = scrollTo as unknown as typeof original;
    try {
      const user = userEvent.setup();
      render(<App />);
      await screen.findByRole("article", { name: "Synthetic task 1" });
      await addTask(user, "First new");
      await addTask(user, "Second new");
      expect(screen.getAllByRole("button", { name: TOAST })).toHaveLength(1);
      // Adding never moves the list: the new Tasks sit below the fold until asked for.
      const area = document.querySelector<HTMLElement>(".main-scroll")!;
      expect(area.scrollTop).toBe(0);
      const before = scrollTo.mock.calls.length;
      // jsdom lays nothing out: give the scroll area room, as a real window has.
      Object.defineProperty(area, "scrollHeight", { configurable: true, value: 5000 });
      Object.defineProperty(area, "clientHeight", { configurable: true, value: 500 });

      await user.click(screen.getByRole("button", { name: TOAST }));
      const row = await screen.findByRole("article", { name: "Second new" });
      expect(within(row).getByRole("region", { name: "Task details" })).toBeInTheDocument();
      expect(row.closest(".task-slot")).toHaveClass("task-slot--flash");
      const asked = scrollTo.mock.calls.slice(before).map(([options]) => (options as ScrollToOptions).top ?? 0);
      expect(Math.max(...asked)).toBeGreaterThan(0);
      expect(area.scrollTop).toBeGreaterThan(0);
      for (const [options] of scrollTo.mock.calls) {
        expect((options as ScrollToOptions).behavior).not.toBe("smooth");
      }
      await waitFor(() => expect(screen.getByRole("button", { name: "Close Second new" })).toHaveFocus());
      expect(screen.queryByRole("button", { name: TOAST })).toBeNull();
    } finally {
      HTMLElement.prototype.scrollTo = original;
    }
  });

  it("opens a new Task hidden by the search on its own, saying why", async () => {
    tasks = [task(1, { title: "Alpha" })];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Alpha" });
    await user.type(screen.getByRole("searchbox", { name: "Search tasks and notes" }), "Alpha");
    await waitFor(() => expect(call).toHaveBeenCalledWith("list_tasks", expect.objectContaining({ text: "Alpha" })));

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Beta");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await screen.findByRole("status", { name: "Task hidden by filters" });
    expect(screen.queryByRole("article", { name: "Beta" })).toBeNull();

    await user.click(screen.getByRole("button", { name: TOAST }));
    const row = await screen.findByRole("article", { name: "Beta" });
    expect(within(row).getByRole("region", { name: "Task details" })).toBeInTheDocument();
    expect(screen.getByRole("status", { name: "Added Task" })).toHaveTextContent("Showing the Task you just added");
    expect(screen.queryByRole("article", { name: "Alpha" })).toBeNull();
    // One banner: the Task is no longer hidden, so the filter notice goes.
    expect(screen.queryByRole("status", { name: "Task hidden by filters" })).toBeNull();
    expect(document.querySelectorAll(".notice-banner")).toHaveLength(1);
  });
});

/** Records every scroll the list asks for, and gives the scroll area room as a real window has. */
function watchScrolls() {
  const scrollTo = vi.fn(function (this: HTMLElement, options?: ScrollToOptions | number) {
    if (typeof options === "object" && options.top !== undefined) this.scrollTop = options.top;
  });
  const original = HTMLElement.prototype.scrollTo;
  HTMLElement.prototype.scrollTo = scrollTo as unknown as typeof original;
  return {
    scrollTo,
    roomy() {
      const area = document.querySelector<HTMLElement>(".main-scroll")!;
      Object.defineProperty(area, "scrollHeight", { configurable: true, value: 5000 });
      Object.defineProperty(area, "clientHeight", { configurable: true, value: 500 });
      return area;
    },
    /** Scrolls asked for since `from` that would move the list down. */
    movedSince(from: number) {
      return scrollTo.mock.calls.slice(from)
        .filter(([options]) => typeof options === "object" && (options.top ?? 0) > 0);
    },
    restore() { HTMLElement.prototype.scrollTo = original; },
  };
}

describe("the list never moves by itself after the toast", () => {
  it("a scroll request that misses is let go at once, so a Task arriving later never moves the list", () => {
    const scrollTo = vi.fn();
    function Shell({ shown, reveal }: { shown: Task[]; reveal: { taskId: string } | null }) {
      const scrollRef = useRef<HTMLDivElement>(null);
      return (
        <div ref={scrollRef} style={{ height: 500, overflow: "auto" }}>
          <VirtualTaskList tasks={shown} hasMore={false} scrollRef={scrollRef} keepMounted={new Set()}
            renderTask={(item) => <article aria-label={item.title} />} expandedId={null}
            onCollapse={() => undefined} reveal={reveal} onRevealed={onRevealed} onAnnounce={() => undefined} />
        </div>
      );
    }
    const onRevealed = vi.fn();
    const shown = Array.from({ length: 30 }, (_, i) => task(i + 1, { status: "in_progress" }));
    const { rerender, container } = render(<Shell shown={shown} reveal={{ taskId: "task-999" }} />);
    (container.firstElementChild as HTMLElement).scrollTo = scrollTo;
    expect(onRevealed).toHaveBeenCalledTimes(1);

    // The parent dropped the request as asked; the Task then arrives.
    rerender(<Shell shown={[...shown, task(999)]} reveal={null} />);
    const down = scrollTo.mock.calls.filter(([options]) => (options?.top ?? 0) > 0);
    expect(down).toEqual([]);
    expect(onRevealed).toHaveBeenCalledTimes(1);
  });

  it("moving its Task to Removed takes the toast away; Undo brings neither the toast nor a scroll", async () => {
    tasks = Array.from({ length: 30 }, (_, i) => task(i + 1, { status: "in_progress" }));
    const scrolls = watchScrolls();
    try {
      const user = userEvent.setup();
      render(<App />);
      await screen.findByRole("article", { name: "Synthetic task 1" });
      await addTask(user, "Short lived");
      const area = scrolls.roomy();
      await user.click(screen.getByRole("button", { name: "Open Short lived" }));
      await user.click(screen.getByRole("button", { name: "Move to Removed" }));
      await screen.findByRole("status", { name: "Moved to Removed" });
      // Nothing left to open: no empty "Showing the Task you just added" view can be reached.
      expect(screen.queryByRole("button", { name: TOAST })).toBeNull();

      area.scrollTop = 0;
      const before = scrolls.scrollTo.mock.calls.length;
      await user.click(screen.getByRole("button", { name: "Undo" }));
      await screen.findByRole("article", { name: "Short lived" });
      await act(async () => { await new Promise((resolve) => setTimeout(resolve, 50)); });
      expect(scrolls.movedSince(before)).toEqual([]);
      expect(area.scrollTop).toBe(0);
      expect(screen.queryByRole("button", { name: TOAST })).toBeNull();
      expect(screen.queryByRole("status", { name: "Added Task" })).toBeNull();
    } finally {
      scrolls.restore();
    }
  });

  it("removing another Task leaves the toast pointing at the new one", async () => {
    tasks = [task(1, { title: "Keep me" }), task(2, { title: "Drop me" })];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Drop me" });
    await addTask(user, "Brand new");
    await user.click(screen.getByRole("button", { name: "Open Drop me" }));
    await user.click(screen.getByRole("button", { name: "Move to Removed" }));
    await screen.findByRole("status", { name: "Moved to Removed" });
    await user.click(screen.getByRole("button", { name: TOAST }));
    const row = await screen.findByRole("article", { name: "Brand new" });
    expect(within(row).getByRole("region", { name: "Task details" })).toBeInTheDocument();
  });
});

describe("the toast and flash stylesheet", () => {
  const bySelector = (selector: string) => rules.filter((rule) =>
    rule.selector.split(",").map((part) => part.trim()).includes(selector));

  it("fades only when Reduce Motion is off, and still shows the flash when it is on", () => {
    for (const selector of [".task-toast", ".task-toast--leaving", ".task-slot--flash", ".task-slot"]) {
      for (const rule of bySelector(selector)) {
        if (/\b(transition|animation)\s*:/.test(rule.body) && !/animation:\s*none/.test(rule.body)) {
          expect(rule.media, `${selector} moves outside no-preference`).toContain("prefers-reduced-motion: no-preference");
        }
      }
    }
    // The highlight itself sits outside any motion query, so Reduce Motion still sees it.
    expect(bySelector(".task-slot--flash").some((rule) => rule.media === null && /outline:/.test(rule.body))).toBe(true);
    expect(bySelector(".task-toast").some((rule) => rule.media === null && /box-shadow/.test(rule.body))).toBe(false);
  });
});
