import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

// A stand-in for the core behind the Tauri seam, with synthetic fixtures only.
const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });
let backend: NoteBackend;
let lists: TaskList[];
let tasks: (Task & { removedAt: number | null })[];
let clock: number;

function task(n: number): Task & { removedAt: number | null } {
  return {
    id: `task-${n}`, title: `T${n}`, description: "", status: "ready", priority: "medium",
    progress: 0, nextStep: "", revision: 1, createdAt: 100 + n, updatedAt: 100 + n,
    listId: null, dueDate: null, tags: [], removedAt: null,
  };
}

/** Whichever command is named here throws this once, as the core would. */
function refusedOnce(command: string): string | undefined {
  const refusal = backend.refusals.get(command);
  backend.refusals.delete(command);
  return refusal;
}

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  lists = [];
  tasks = [];
  clock = 1_000;
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((list) => ({ ...list })), nextCursor: null };
      case "list_tasks": {
        const items = tasks.filter((item) => item.removedAt === null).map(({ removedAt: _, ...item }) => item);
        return { items, nextCursor: null };
      }
      case "set_task_removed": {
        const refusal = refusedOnce(command);
        if (refusal) throw refusal;
        const found = tasks.find((item) => item.id === body.input.id)!;
        found.removedAt = body.input.removed ? ++clock : null;
        found.revision++;
        const { removedAt, ...shown } = found;
        return { ...shown, removed: removedAt !== null };
      }
      case "set_list_removed": {
        const refusal = refusedOnce(command);
        if (refusal) throw refusal;
        const list = lists.find((item) => item.id === body.input.id)!;
        list.removed = body.input.removed;
        list.revision++;
        return { ...list };
      }
      case "list_progress_entries": case "list_task_notes": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      default: return backend.handle(command, body);
    }
  });
});

afterEach(() => cleanup());

const notice = () => screen.queryByRole("status", { name: "Moved to Removed" });

describe("a refused Undo", () => {
  it("shows at once while older moves remain, and they stay undoable", async () => {
    backend.seed("# Older", "Older");
    const packing = backend.seed("# Packing", "Packing");
    backend.folder("Work");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const notes = await screen.findByRole("list", { name: "Notes" });
    await eventually(() => expect(within(notes).getByRole("button", { name: /Packing/ }))
      .toHaveAttribute("aria-current", "true"));
    await user.click(screen.getByRole("button", { name: "Move note to Removed" }));
    await screen.findByRole("status", { name: "Moved to Removed" });
    const folders = screen.getByRole("navigation", { name: "Note folders" });
    await user.click(await within(folders).findByRole("button", { name: "Remove Work" }));
    await eventually(() => expect(notice()).toHaveTextContent("“Work” moved to Removed"));

    backend.refuse("set_note_folder_removed", "A folder with this name already exists here");
    await user.click(within(notice()!).getByRole("button", { name: "Undo" }));

    // One bar at a time in the window's last row: the failure takes it.
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Couldn't undo: A folder with this name already exists here");
    expect(notice()).not.toBeInTheDocument();

    await user.click(within(alert).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(notice()).toHaveTextContent("“Packing” moved to Removed");
    await user.click(within(notice()!).getByRole("button", { name: "Undo" }));
    await eventually(() => expect(backend.removedAt.has(packing.id)).toBe(false));
  });

  it("from a removed List shows in Settings", async () => {
    lists = [{ id: "list-1", name: "Errands", revision: 1, removed: false, createdAt: 1, updatedAt: 1 } as TaskList];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    await user.click(screen.getByRole("button", { name: "Remove Errands" }));
    await screen.findByRole("status", { name: "Moved to Removed" });
    await user.click(screen.getByRole("button", { name: "Settings" }));

    backend.refuse("set_list_removed", "A List with this name already exists");
    await user.keyboard("{Meta>}z{/Meta}");

    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't undo: A List with this name already exists");
    expect(lists[0].removed).toBe(true);
  });

  it("from a removed Task shows in Settings", async () => {
    tasks = [task(1)];
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open T1" }));
    await user.click(screen.getByRole("button", { name: "Move to Removed" }));
    await screen.findByRole("status", { name: "Moved to Removed" });
    await user.click(screen.getByRole("button", { name: "Settings" }));

    backend.refuse("set_task_removed", "Revision conflict: the Task has changed; reload before saving");
    await user.keyboard("{Meta>}z{/Meta}");

    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't undo: Revision conflict");
    expect(tasks[0].removedAt).not.toBeNull();
  });
});
