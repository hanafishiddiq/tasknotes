import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { TASK_PAGE_SIZE, type Task, type TaskList } from "./lib/task-api";
import { OUTSIDE_CHANGE_POLL_MS } from "./lib/use-outside-changes";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * A stand-in for the core behind the Tauri seam, shared with "another process":
 * a test changes these directly and calls `outside()`, as the CLI or an MCP
 * client would. Which changes count as outside is the core's rule, tested there.
 * Synthetic data only.
 */
let tasks: Task[];
let lists: TaskList[];
let notes: NoteBackend;
/** The Audit Entry count, and the newest one written by another process. */
let latestEntry: number;
let latestOutside: number;
/** Holds the next save open, to show what a check does in the meantime. */
let holdSave: Promise<void> | null;

function outside() {
  latestEntry += 1;
  latestOutside = latestEntry;
}

function task(n: number, fields: Partial<Task> = {}): Task {
  return {
    id: `task-${n}`, title: `Synthetic task ${n}`, description: "", status: "ready",
    priority: "medium", progress: 0, nextStep: "", revision: 1, createdAt: 100 + n,
    updatedAt: 100 + n, listId: null, dueDate: null, tags: [], ...fields,
  };
}

function list(name: string, fields: Partial<TaskList> = {}): TaskList {
  return { id: `list-${name}`, name, revision: 1, removed: false, createdAt: 1, updatedAt: 1, ...fields } as TaskList;
}

const countOf = (command: string) => call.mock.calls.filter(([name]) => name === command).length;

/** The window regains focus. Waits for the check and whatever it starts to settle. */
async function focusWindow() {
  await act(async () => {
    window.dispatchEvent(new Event("focus"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  tasks = [];
  lists = [];
  notes = new NoteBackend();
  latestEntry = 1;
  latestOutside = 0;
  holdSave = null;
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "outside_changes":
        return { latest: latestEntry, changed: body.seen != null && latestOutside > body.seen };
      case "list_lists": return { items: lists.map((item) => ({ ...item })), nextCursor: null };
      case "list_tasks": {
        const viewed = body.view?.kind === "list" ? body.view.listId : null;
        if (viewed && lists.find((item) => item.id === viewed)?.removed) throw "This List is in Removed";
        const hidden = new Set(lists.filter((item) => item.removed).map((item) => item.id));
        const items = tasks
          .filter((item) => !(item.listId && hidden.has(item.listId)))
          .filter((item) => !viewed || item.listId === viewed)
          .filter((item) => !body.taskId || item.id === body.taskId)
          .filter((item) => !body.text || item.title.toLowerCase().includes(String(body.text).toLowerCase()));
        const start = body.cursor ? Number(body.cursor) : 0;
        const end = start + TASK_PAGE_SIZE;
        return {
          items: items.slice(start, end).map((item) => ({ ...item })),
          nextCursor: end < items.length ? String(end) : null,
        };
      }
      case "update_task": {
        if (holdSave) await holdSave;
        const found = tasks.find((item) => item.id === body.input.id)!;
        if (found.revision !== body.input.expectedRevision) {
          throw "Revision conflict: the Task has changed; reload before saving";
        }
        found.title = body.input.title;
        found.description = body.input.description;
        found.revision += 1;
        latestEntry += 1;
        return { ...found };
      }
      case "list_removed":
      case "list_progress_entries":
      case "list_task_notes":
      case "list_note_tasks":
        return { items: [], nextCursor: null };
      case "count_pending_bulk_requests": return 0;
      case "quit_ready": return null;
      default: return notes.handle(command, body);
    }
  });
});

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

describe("Changes made outside the window", () => {
  it("shows a Task and a List another process added once the window regains focus", async () => {
    tasks = [task(1)];
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });

    tasks.unshift(task(2, { title: "Added through MCP" }));
    lists.push(list("Imported"));
    outside();
    await focusWindow();

    expect(await screen.findByRole("article", { name: "Added through MCP" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Imported" })).toBeVisible();
    expect(screen.getByRole("article", { name: "Synthetic task 1" })).toBeVisible();
  });

  it("finds them on its own while the window stays in focus", async () => {
    tasks = [task(1)];
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });

    tasks.unshift(task(2, { title: "Added through the CLI" }));
    outside();
    await act(async () => { await vi.advanceTimersByTimeAsync(OUTSIDE_CHANGE_POLL_MS); });

    expect(await screen.findByRole("article", { name: "Added through the CLI" })).toBeVisible();
  });

  it("reads nothing again while nothing changed outside", async () => {
    tasks = [task(1)];
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    await focusWindow();
    const reads = countOf("list_tasks");

    // A change this window made itself is not an outside change.
    latestEntry += 1;
    await focusWindow();

    expect(countOf("list_tasks")).toBe(reads);
  });

  it("keeps an open Task's unsaved text and focus when the Task itself changes outside", async () => {
    tasks = [task(1), task(2)];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    const field = screen.getByRole("textbox", { name: "Edit description" });
    await user.type(field, "Half a thought");

    tasks[0] = { ...tasks[0], title: "Renamed through MCP", revision: 2, updatedAt: 500 };
    tasks.unshift(task(3, { title: "Added through MCP" }));
    outside();
    await focusWindow();

    expect(await screen.findByRole("article", { name: "Added through MCP" })).toBeVisible();
    const edited = screen.getByRole("textbox", { name: "Edit description" });
    expect(edited).toHaveValue("Half a thought");
    expect(edited).toBe(field);
    expect(edited).toHaveFocus();
    // What was not typed follows the newer version, and the save names the older one.
    expect(screen.getByRole("textbox", { name: "Edit title" })).toHaveValue("Renamed through MCP");
    expect(screen.getByRole("alert", { name: "Changed elsewhere" })).toBeVisible();
    expect(screen.queryByText("Loading your tasks...")).not.toBeInTheDocument();
  });

  it("keeps what is typed in New task", async () => {
    tasks = [task(1)];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    const capture = screen.getByRole("textbox", { name: "Task title" });
    await user.type(capture, "Half a title");

    tasks.unshift(task(2, { title: "Added through MCP" }));
    outside();
    await focusWindow();

    await screen.findByRole("article", { name: "Added through MCP" });
    expect(capture).toHaveValue("Half a title");
    expect(capture).toHaveFocus();
  });

  it("keeps an open Task on screen with its text when the change takes it out of the view", async () => {
    tasks = [task(1), task(2)];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "Half a thought");

    // Removed outside: no list asked for by this window shows it any more.
    tasks.shift();
    tasks.unshift(task(3, { title: "Added through MCP" }));
    outside();
    await focusWindow();

    await screen.findByRole("article", { name: "Added through MCP" });
    expect(screen.getByRole("textbox", { name: "Edit description" })).toHaveValue("Half a thought");

    // Closing is not discarding: the text is still reachable from its row.
    await user.click(screen.getByRole("button", { name: "Close Synthetic task 1" }));
    const kept = await screen.findByRole("article", { name: "Synthetic task 1" });
    expect(within(kept).getByText("Unsaved edits")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Open Synthetic task 1" }));
    const field = screen.getByRole("textbox", { name: "Edit description" });
    expect(field).toHaveValue("Half a thought");

    // With nothing left unsaved, the list is what the core says again.
    await user.clear(field);
    await waitFor(() =>
      expect(screen.queryByRole("article", { name: "Synthetic task 1" })).not.toBeInTheDocument());
  });

  it("keeps a closed row's unsaved text when the change takes it out of the view", async () => {
    tasks = [task(1), task(2)];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "Half a thought");
    await user.click(screen.getByRole("button", { name: "Close Synthetic task 1" }));

    tasks.shift();
    outside();
    await focusWindow();

    const kept = await screen.findByRole("article", { name: "Synthetic task 1" });
    expect(within(kept).getByText("Unsaved edits")).toBeVisible();
  });

  it("shows the latest version of an open Task that only left this view", async () => {
    const work = list("Work");
    lists = [work];
    tasks = [task(1, { listId: work.id })];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Work" }));
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "Half a thought");

    tasks[0] = { ...tasks[0], listId: null, revision: 2 };
    outside();
    await focusWindow();

    expect(screen.getByRole("textbox", { name: "Edit description" })).toHaveValue("Half a thought");
    // Its newer revision is what lets "Keep my edits" save over the change.
    expect(await screen.findByRole("alert", { name: "Changed elsewhere" })).toBeVisible();
  });

  it("stays on the page showing, and leaves it only when nothing is left there", async () => {
    tasks = Array.from({ length: TASK_PAGE_SIZE + 5 }, (_, i) => task(i + 1));
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await screen.findByRole("article", { name: `Synthetic task ${TASK_PAGE_SIZE + 1}` });

    tasks[TASK_PAGE_SIZE] = { ...tasks[TASK_PAGE_SIZE], title: "Renamed on page two", revision: 2 };
    outside();
    await focusWindow();
    expect(await screen.findByRole("article", { name: "Renamed on page two" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Previous page" })).toBeVisible();

    tasks = tasks.slice(0, 2);
    outside();
    await focusWindow();
    expect(await screen.findByRole("article", { name: "Synthetic task 1" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Previous page" })).not.toBeInTheDocument();
  });

  it("waits for a save in flight, then shows the change", async () => {
    tasks = [task(1), task(2)];
    let release = () => {};
    holdSave = new Promise((resolve) => { release = resolve; });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 1" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "Saved text");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    tasks.push(task(3, { title: "Added through MCP" }));
    outside();
    await focusWindow();
    expect(screen.queryByRole("article", { name: "Added through MCP" })).not.toBeInTheDocument();

    await act(async () => { release(); });
    await focusWindow();
    expect(await screen.findByRole("article", { name: "Added through MCP" })).toBeVisible();
  });

  it("asks again next time when reading the change failed, without a banner", async () => {
    tasks = [task(1)];
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    const real = call.getMockImplementation()!;
    call.mockImplementationOnce(real); // the check itself
    call.mockImplementationOnce(async () => { throw "database is locked"; }); // the read after it

    tasks.unshift(task(2, { title: "Added through MCP" }));
    outside();
    await focusWindow();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    await focusWindow();
    expect(await screen.findByRole("article", { name: "Added through MCP" })).toBeVisible();
  });

  it("goes to All tasks when the List on screen was removed outside", async () => {
    const work = list("Work");
    lists = [work];
    tasks = [task(1), task(2, { listId: work.id, title: "In Work" })];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Work" }));
    await screen.findByRole("article", { name: "In Work" });

    lists[0] = { ...work, removed: true, revision: 2 };
    outside();
    await focusWindow();

    expect(await screen.findByRole("heading", { name: "All tasks" })).toBeVisible();
    expect(await screen.findByRole("article", { name: "Synthetic task 1" })).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not leave a removed List while its open Task holds unsaved text", async () => {
    const work = list("Work");
    lists = [work];
    tasks = [task(2, { listId: work.id, title: "In Work" })];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Work" }));
    await user.click(await screen.findByRole("button", { name: "Open In Work" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "Half a thought");

    lists[0] = { ...work, removed: true, revision: 2 };
    outside();
    await focusWindow();

    const field = screen.getByRole("textbox", { name: "Edit description" });
    expect(field).toHaveValue("Half a thought");

    await user.clear(field);
    expect(await screen.findByRole("heading", { name: "All tasks" })).toBeVisible();
  });
});

describe("The Task list's scroll position", () => {
  let restore: () => void;
  beforeEach(() => {
    // jsdom has no layout: a list item sits at its index, the list scrolled by scrollTop.
    const offset = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight")!;
    const rect = Element.prototype.getBoundingClientRect;
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
      configurable: true,
      get(this: HTMLElement) {
        if (this.classList.contains("main-scroll")) return 600;
        if (this.dataset.index !== undefined) return 50;
        return 0;
      },
    });
    Element.prototype.getBoundingClientRect = function (this: Element) {
      const element = this as HTMLElement;
      if (element.dataset.index === undefined) return new DOMRect(0, 0, 800, 0);
      const scrolled = document.querySelector<HTMLElement>("main:not([hidden]) .main-scroll")!.scrollTop;
      return new DOMRect(0, Number(element.dataset.index) * 50 - scrolled, 800, 50);
    };
    restore = () => {
      Object.defineProperty(HTMLElement.prototype, "offsetHeight", offset);
      Element.prototype.getBoundingClientRect = rect;
    };
    tasks = Array.from({ length: 40 }, (_, i) => task(i + 1));
  });
  afterEach(() => restore());

  const scrollArea = () => document.querySelector<HTMLElement>("main:not([hidden]) .main-scroll")!;

  it("keeps an open Task where it is when rows are added above it", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open Synthetic task 20" }));
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), "Half a thought");
    const area = scrollArea();
    area.scrollTop = 800;
    fireEvent.scroll(area);

    tasks.unshift(task(50, { title: "Added through MCP" }), task(51, { title: "Also through MCP" }));
    outside();
    await focusWindow();

    expect(screen.getByRole("textbox", { name: "Edit description" })).toHaveValue("Half a thought");
    // Two rows of 50 went in above it.
    await waitFor(() => expect(area.scrollTop).toBe(900));
  });

  it("keeps the rows being read in place when nothing is open", async () => {
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic task 1" });
    const area = scrollArea();
    area.scrollTop = 400;
    fireEvent.scroll(area);

    tasks.unshift(task(50, { title: "Added through MCP" }));
    outside();
    await focusWindow();

    await waitFor(() => expect(area.scrollTop).toBe(450));
  });
});

describe("Notes changed outside the window", () => {
  async function openNotes() {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    await waitFor(() => expect(document.querySelector(".note-content")).not.toBeNull());
    return user;
  }

  const noteList = () => screen.findByRole("list", { name: "Notes" });

  it("lists a Note added outside and its folder", async () => {
    const first = notes.seed("First note", "First note");
    await openNotes();
    await within(await noteList()).findByText("First note");

    notes.seed("Added through MCP", "Added through MCP");
    notes.folder("Imported folder");
    outside();
    await focusWindow();

    expect(await within(await noteList()).findByText("Added through MCP")).toBeVisible();
    expect(await screen.findByRole("button", { name: "Imported folder" })).toBeVisible();
    expect(notes.notes.get(first.id)?.revision).toBe(1);
  });

  it("shows the newer text of the open Note when nothing is typed there", async () => {
    const open = notes.seed("Old text", "Old text");
    await openNotes();
    await screen.findByText("Old text", { selector: ".note-content *" });

    notes.notes.set(open.id, { ...open, markdown: "Newer text", title: "Newer text", revision: 2 });
    outside();
    await focusWindow();

    expect(await screen.findByText("Newer text", { selector: ".note-content *" })).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("leaves the text alone while the cursor is in it, and follows once the cursor leaves", async () => {
    const open = notes.seed("Old text", "Old text");
    await openNotes();
    await screen.findByText("Old text", { selector: ".note-content *" });
    const editor = document.querySelector<HTMLElement>(".note-content")!;
    act(() => editor.focus());

    notes.notes.set(open.id, { ...open, markdown: "Newer text", title: "Newer text", revision: 2 });
    outside();
    await focusWindow();

    expect(document.querySelector(".note-content")).toBe(editor);
    expect(editor).toHaveTextContent("Old text");
    // Nothing was typed, so there is nothing to choose between.
    expect(screen.queryByRole("button", { name: "Load latest" })).not.toBeInTheDocument();

    act(() => editor.blur());
    expect(await screen.findByText("Newer text", { selector: ".note-content *" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Load latest" })).not.toBeInTheDocument();
  });

  it("follows when the window is elsewhere, even with the cursor still in the editor", async () => {
    const open = notes.seed("Old text", "Old text");
    await openNotes();
    await screen.findByText("Old text", { selector: ".note-content *" });
    act(() => document.querySelector<HTMLElement>(".note-content")!.focus());
    const hasFocus = vi.spyOn(document, "hasFocus").mockReturnValue(false);

    notes.notes.set(open.id, { ...open, markdown: "Newer text", title: "Newer text", revision: 2 });
    outside();
    await focusWindow();

    expect(await screen.findByText("Newer text", { selector: ".note-content *" })).toBeVisible();
    hasFocus.mockRestore();
  });

  it("asks only once typing meets the newer version", async () => {
    const open = notes.seed("Old text", "Old text");
    const user = await openNotes();
    await screen.findByText("Old text", { selector: ".note-content *" });
    await user.click(document.querySelector<HTMLElement>(".note-content")!);

    notes.notes.set(open.id, { ...open, markdown: "Newer text", title: "Newer text", revision: 2 });
    outside();
    await focusWindow();
    expect(screen.queryByRole("button", { name: "Load latest" })).not.toBeInTheDocument();

    await user.keyboard(" and mine");
    expect(await screen.findByRole("button", { name: "Load latest" }, { timeout: 3000 })).toBeVisible();
    expect(document.querySelector(".note-content")).toHaveTextContent("and mine");
    expect(notes.notes.get(open.id)?.markdown).toBe("Newer text");
  });

  it("keeps unsaved text in the open Note and never saves over the newer version", async () => {
    const open = notes.seed("Old text", "Old text");
    const user = await openNotes();
    await screen.findByText("Old text", { selector: ".note-content *" });
    await user.click(document.querySelector<HTMLElement>(".note-content")!);
    await user.keyboard(" and mine");

    notes.notes.set(open.id, { ...open, markdown: "Newer text", title: "Newer text", revision: 2 });
    outside();
    await focusWindow();

    const typed = document.querySelector(".note-content");
    expect(typed).toHaveTextContent("Old text");
    expect(typed).toHaveTextContent("and mine");
    expect(await screen.findByRole("button", { name: "Load latest" }, { timeout: 3000 })).toBeVisible();
    expect(notes.notes.get(open.id)?.markdown).toBe("Newer text");
  });

  it("moves on when the open Note was removed outside and nothing is typed there", async () => {
    notes.seed("Stays", "Stays");
    const open = notes.seed("Goes away", "Goes away");
    await openNotes();
    await screen.findByText("Goes away", { selector: ".note-content *" });

    notes.removedAt.set(open.id, 1);
    outside();
    await focusWindow();

    expect(await screen.findByText("Stays", { selector: ".note-content *" })).toBeVisible();
    expect(within(await noteList()).queryByText("Goes away")).not.toBeInTheDocument();
  });

  it("keeps the Notes already loaded with Load more", async () => {
    for (let n = 1; n <= 55; n += 1) notes.seed(`Note ${n}`, `Note ${n}`);
    const user = await openNotes();
    await user.click(await screen.findByRole("button", { name: "Load more Notes" }));
    await within(await noteList()).findByText("Note 1");

    notes.seed("Added through MCP", "Added through MCP");
    outside();
    await focusWindow();

    await within(await noteList()).findByText("Added through MCP");
    expect(within(await noteList()).getByText("Note 1")).toBeInTheDocument();
  });

  it("refreshes the Notes found by a global search", async () => {
    notes.seed("Plan alpha", "Plan alpha");
    const user = userEvent.setup();
    render(<App />);
    await user.type(await screen.findByRole("searchbox", { name: "Search tasks and notes" }), "plan");
    const hits = await screen.findByRole("region", { name: "Matching notes" });
    await within(hits).findByText("Plan alpha");

    notes.seed("Plan beta", "Plan beta");
    outside();
    await focusWindow();

    expect(await within(hits).findByText("Plan beta")).toBeVisible();
  });
});
