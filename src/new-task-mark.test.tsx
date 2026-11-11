import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, renderHook, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { OUTSIDE_MARK_MS, useNewTaskMarks } from "./lib/use-new-task-marks";
import { OUTSIDE_CHANGE_POLL_MS } from "./lib/use-outside-changes";
import type { Task } from "./lib/task-api";
import { rules } from "./test/stylesheet";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * Batch 3, Ticket 02 (owner decisions 2026-11-11, Q6–Q16): a pale-yellow mark
 * on Tasks just added. One the person adds in the window stays until any Task
 * is opened; one the CLI or an MCP client adds stays until 10 seconds after it
 * is seen, or until it is opened. View state only. Synthetic fixtures only.
 */
const JUST_ADDED = "Just added";
const TOAST = "Task added · Click to edit";

let tasks: Task[];
let created: number;
/** The audit log's end, and the Tasks another process created after each point in it. */
let latestEntry: number;
let createdOutside: { sequence: number; taskId: string }[];

function task(n: number, fields: Partial<Task> = {}): Task {
  return {
    id: `task-${n}`, title: `Synthetic task ${n}`, description: "", status: "ready",
    priority: "medium", difficulty: null, wontDo: false, progress: 0, nextStep: "", revision: 1, createdAt: 100 + n,
    updatedAt: 100 + n, listId: null, dueDate: null, tags: [], ...fields,
  };
}

/** Another process (the CLI or an MCP client) creates a Task. */
function createdByAnotherProcess(made: Task) {
  tasks.push(made);
  latestEntry += 1;
  createdOutside.push({ sequence: latestEntry, taskId: made.id });
}

/** What the rows' visibility check sees: every observed row on screen, or none. */
let onScreen: boolean;
const observers = new Set<{ callback: IntersectionObserverCallback; targets: Set<Element> }>();

class FakeIntersectionObserver {
  private entry: { callback: IntersectionObserverCallback; targets: Set<Element> };
  constructor(callback: IntersectionObserverCallback) {
    this.entry = { callback, targets: new Set() };
    observers.add(this.entry);
  }
  observe(target: Element) {
    this.entry.targets.add(target);
    this.entry.callback([{ target, isIntersecting: onScreen } as IntersectionObserverEntry], this as never);
  }
  unobserve(target: Element) { this.entry.targets.delete(target); }
  disconnect() { observers.delete(this.entry); }
  takeRecords() { return []; }
}

/** Every observed row comes on screen (or goes), as scrolling would do. */
function scrollRowsOnScreen(now: boolean) {
  onScreen = now;
  act(() => {
    for (const { callback, targets } of observers) {
      callback([...targets].map((target) => ({ target, isIntersecting: now }) as IntersectionObserverEntry), {} as never);
    }
  });
}

beforeEach(() => {
  tasks = [];
  created = 0;
  latestEntry = 1;
  createdOutside = [];
  onScreen = true;
  observers.clear();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "outside_changes": {
        const seen = body.seen as number | null;
        const after = seen === null ? [] : createdOutside.filter((entry) => entry.sequence > seen);
        return { latest: latestEntry, changed: after.length > 0, createdTasks: after.map((entry) => entry.taskId) };
      }
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: [...tasks], nextCursor: null };
      case "create_task": {
        created += 1;
        latestEntry += 1;
        const made = task(1000 + created, { title: body.input.title.trim() });
        tasks.push(made);
        return made;
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
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const row = (title: string) => screen.getByRole("article", { name: title });
const isMarked = (title: string) => row(title).classList.contains("task-card--new");

/**
 * The window's first check only learns where the log ends, once the view has
 * loaded; a Task another process makes after that is new.
 */
async function windowHasLookedOnce() {
  await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_CHANGE_POLL_MS); });
}

async function addTask(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.type(screen.getByRole("textbox", { name: "Task title" }), title);
  await user.click(screen.getByRole("button", { name: "Add task" }));
  await screen.findByRole("article", { name: title });
}

describe("the marks on their own", () => {
  it("keeps the person's own adds until any Task is opened", () => {
    const { result } = renderHook(() => useNewTaskMarks());
    act(() => { result.current.addedHere("a"); result.current.addedHere("b"); });
    expect([...result.current.marked].sort()).toEqual(["a", "b"]);
    act(() => result.current.opened("some-other-task"));
    expect(result.current.marked.size).toBe(0);
  });

  it("keeps an outside add until 10 seconds after it is seen, never counting before", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useNewTaskMarks());
    act(() => result.current.addedOutside(["x"]));
    expect(result.current.marked.has("x")).toBe(true);
    expect(result.current.waitingToBeSeen.has("x")).toBe(true);
    act(() => vi.advanceTimersByTime(60_000));
    expect(result.current.marked.has("x")).toBe(true);

    act(() => result.current.seen("x"));
    expect(result.current.waitingToBeSeen.has("x")).toBe(false);
    act(() => vi.advanceTimersByTime(OUTSIDE_MARK_MS - 1));
    // Seen again: the 10 seconds do not start over.
    act(() => result.current.seen("x"));
    expect(result.current.marked.has("x")).toBe(true);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.marked.has("x")).toBe(false);
    expect(OUTSIDE_MARK_MS).toBe(10_000);
  });

  it("drops one outside mark when that Task is opened, leaving the others", () => {
    const { result } = renderHook(() => useNewTaskMarks());
    act(() => { result.current.addedOutside(["x", "y"]); result.current.addedHere("a"); });
    act(() => result.current.opened("x"));
    expect([...result.current.marked]).toEqual(["y"]);
  });

  it("forgets a Task moved to Removed, and stores nothing", () => {
    const { result } = renderHook(() => useNewTaskMarks());
    act(() => { result.current.addedHere("a"); result.current.addedOutside(["x"]); });
    act(() => { result.current.forget("a"); result.current.forget("x"); });
    expect(result.current.marked.size).toBe(0);
    expect(window.localStorage.length).toBe(0);
  });
});

describe("a Task the person adds in the window", () => {
  it("is marked, with every add in a row, until any Task is opened", async () => {
    tasks = [task(1, { title: "Older task" })];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Older task" });

    await addTask(user, "First new");
    await addTask(user, "Second new");
    expect(isMarked("First new")).toBe(true);
    expect(isMarked("Second new")).toBe(true);
    expect(isMarked("Older task")).toBe(false);
    // Screen readers hear it too, not only the colour, and also when moving between titles.
    expect(within(row("Second new")).getByText(JUST_ADDED)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Second new" })).toHaveAccessibleDescription(JUST_ADDED);

    await user.click(screen.getByRole("button", { name: "Open Older task" }));
    expect(isMarked("First new")).toBe(false);
    expect(isMarked("Second new")).toBe(false);
    expect(screen.queryByText(JUST_ADDED)).toBeNull();
  });

  it("loses its mark when the toast opens it", async () => {
    tasks = [task(1, { title: "Older task" })];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Older task" });
    await addTask(user, "Fresh idea");
    expect(isMarked("Fresh idea")).toBe(true);
    await user.click(screen.getByRole("button", { name: TOAST }));
    expect(isMarked("Fresh idea")).toBe(false);
  });
});

describe("a Task another process adds", () => {
  it("is marked with no toast, and goes 10 seconds after it is seen", async () => {
    tasks = [task(1, { title: "Older task" })];
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<App />);
    await screen.findByRole("article", { name: "Older task" });
    await windowHasLookedOnce();
    // Off screen at first (another List, or scrolled away): its 10 seconds wait.
    scrollRowsOnScreen(false);

    createdByAnotherProcess(task(2, { title: "Added by an AI" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_CHANGE_POLL_MS); });
    await screen.findByRole("article", { name: "Added by an AI" });
    expect(isMarked("Added by an AI")).toBe(true);
    expect(within(row("Added by an AI")).getByText(JUST_ADDED)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: TOAST })).toBeNull();

    await act(async () => { await vi.advanceTimersByTimeAsync(30_000); });
    expect(isMarked("Added by an AI")).toBe(true);

    scrollRowsOnScreen(true);
    await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_MARK_MS - 100); });
    expect(isMarked("Added by an AI")).toBe(true);
    await act(async () => { await vi.advanceTimersByTimeAsync(100); });
    expect(isMarked("Added by an AI")).toBe(false);
  });

  it("waits while the window is not in front, even on screen", async () => {
    tasks = [task(1, { title: "Older task" })];
    vi.mocked(document.hasFocus).mockReturnValue(false);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<App />);
    await screen.findByRole("article", { name: "Older task" });
    await windowHasLookedOnce();

    createdByAnotherProcess(task(2, { title: "Added while away" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_CHANGE_POLL_MS); });
    await screen.findByRole("article", { name: "Added while away" });
    await act(async () => { await vi.advanceTimersByTimeAsync(30_000); });
    expect(isMarked("Added while away")).toBe(true);

    vi.mocked(document.hasFocus).mockReturnValue(true);
    act(() => { fireEvent.focus(window); });
    await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_MARK_MS); });
    expect(isMarked("Added while away")).toBe(false);
  });

  it("keeps the others marked when one of them is opened", async () => {
    tasks = [task(1, { title: "Older task" })];
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<App />);
    await screen.findByRole("article", { name: "Older task" });
    await windowHasLookedOnce();
    scrollRowsOnScreen(false);

    createdByAnotherProcess(task(2, { title: "AI task one" }));
    createdByAnotherProcess(task(3, { title: "AI task two" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_CHANGE_POLL_MS); });
    await screen.findByRole("article", { name: "AI task two" });

    fireEvent.click(screen.getByRole("button", { name: "Open AI task one" }));
    expect(isMarked("AI task one")).toBe(false);
    expect(isMarked("AI task two")).toBe(true);
  });

  it("gets no mark when it is open already by the time the check names it", async () => {
    tasks = [task(1, { title: "Older task" })];
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<App />);
    await screen.findByRole("article", { name: "Older task" });
    await windowHasLookedOnce();

    // The list shows it before the next check: the read after the person's own add finds it.
    createdByAnotherProcess(task(2, { title: "AI task seen early" }));
    await addTask(user, "Mine");
    await user.click(await screen.findByRole("button", { name: "Open AI task seen early" }));
    await windowHasLookedOnce();
    expect(isMarked("AI task seen early")).toBe(false);
    expect(screen.queryByText(JUST_ADDED)).toBeNull();
  });

  it("gets no mark when it was added before the window opened", async () => {
    createdByAnotherProcess(task(2, { title: "Added while closed" }));
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<App />);
    await screen.findByRole("article", { name: "Added while closed" });
    scrollRowsOnScreen(false);
    await windowHasLookedOnce();
    await windowHasLookedOnce();
    expect(isMarked("Added while closed")).toBe(false);
  });
});

describe("the mark's stylesheet", () => {
  const bodyOf = (selector: string) => rules.filter((rule) =>
    rule.selector.split(",").map((part) => part.replace(/\s+/g, " ").trim()).includes(selector)).map((rule) => rule.body).join("\n");

  it("fills the row with a paler sidebar hover yellow, keeps it under the pointer, and draws no border", () => {
    // Half the sidebar hover yellow over the canvas: the full one drops red, orange and faint
    // row text and the status ring below WCAG AA (measured in a11y-contrast.test).
    const fill = /background: color-mix\(in srgb, var\(--side-hover\) \d+%, var\(--bg\)\)/;
    const resting = bodyOf(".task-card--new:not(.task-card--expanded) .task-row");
    const hovered = bodyOf(".task-card--new:not(.task-card--expanded) .task-row:hover");
    expect(resting).toMatch(fill);
    expect(hovered).toMatch(fill);
    expect(`${resting}\n${hovered}`).not.toMatch(/border|outline/);
  });
});
