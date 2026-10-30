import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { TaskList } from "./lib/task-api";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

// A stand-in for the core behind the Tauri seam, with synthetic fixtures only.
const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });
let backend: NoteBackend;
let lists: TaskList[];

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  lists = [];
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((list) => ({ ...list })), nextCursor: null };
      case "list_tasks": return { items: [], nextCursor: null };
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
});
