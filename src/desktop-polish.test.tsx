import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task, TaskList } from "./lib/task-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * A stand-in for the core behind the Tauri seam. Only synthetic fixtures; the
 * rules themselves are the core's and are tested in Rust.
 */
let tasks: (Task & { removedAt: number | null })[];
let lists: TaskList[];
let clock: number;

function task(n: number, fields: Partial<Task> = {}): Task & { removedAt: number | null } {
  return {
    id: `task-${n}`, title: `Synthetic task ${n}`, description: "", status: "ready",
    priority: "medium", progress: 0, nextStep: "", revision: 1, createdAt: 100 + n,
    updatedAt: 100 + n, listId: null, dueDate: null, tags: [], removedAt: null, ...fields,
  };
}

/** Lets a test decide what the system appearance and Reduce Motion report. */
const media = { dark: true, reducedMotion: false, listeners: new Set<() => void>() };
function installMatchMedia() {
  window.matchMedia = ((query: string) => ({
    get matches() {
      if (query.includes("prefers-color-scheme: dark")) return media.dark;
      if (query.includes("prefers-reduced-motion: reduce")) return media.reducedMotion;
      return false;
    },
    media: query,
    addEventListener: (_: string, listener: () => void) => media.listeners.add(listener),
    removeEventListener: (_: string, listener: () => void) => media.listeners.delete(listener),
    addListener: () => undefined,
    removeListener: () => undefined,
    onchange: null,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

beforeEach(() => {
  tasks = [];
  lists = [];
  clock = 1_000;
  media.dark = true;
  media.reducedMotion = false;
  media.listeners.clear();
  installMatchMedia();
  window.localStorage.clear();
  delete document.documentElement.dataset.theme;
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((list) => ({ ...list })), nextCursor: null };
      case "list_tasks": {
        const hidden = new Set(lists.filter((list) => list.removed).map((list) => list.id));
        const items = tasks
          .filter((item) => item.removedAt === null && !(item.listId && hidden.has(item.listId)))
          .filter((item) => !body.taskId || item.id === body.taskId)
          .filter((item) => !body.listId || item.listId === body.listId)
          .filter((item) => !body.inbox || item.listId === null)
          .filter((item) => !body.text || item.title.toLowerCase().includes(String(body.text).toLowerCase()))
          .map(({ removedAt: _, ...item }) => item);
        return { items, nextCursor: null };
      }
      case "set_task_removed": {
        const found = tasks.find((item) => item.id === body.input.id)!;
        found.removedAt = body.input.removed ? ++clock : null;
        found.revision++;
        const { removedAt, ...shown } = found;
        return { ...shown, removed: removedAt !== null };
      }
      case "set_list_removed": {
        const list = lists.find((item) => item.id === body.input.id)!;
        list.removed = body.input.removed;
        list.revision++;
        return { ...list };
      }
      case "list_removed": return { items: [], nextCursor: null };
      case "list_progress_entries": return { items: [], nextCursor: null };
      case "list_task_notes": return { items: [], nextCursor: null };
      case "list_notes": return { items: [], nextCursor: null };
      case "list_note_folders": return { items: [], nextCursor: null };
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => cleanup());

describe("Settings and appearance", () => {
  it("opens Settings from the sidebar and follows the chosen appearance", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    expect(document.documentElement.dataset.theme).toBe("dark");

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
    const appearance = screen.getByRole("radiogroup", { name: "Appearance" });
    expect(within(appearance).getByRole("radio", { name: "Dark" })).toBeChecked();

    await user.click(within(appearance).getByRole("radio", { name: "Light" }));
    expect(document.documentElement.dataset.theme).toBe("light");

    // Match system follows what macOS reports, and changes with it.
    media.dark = false;
    await user.click(within(appearance).getByRole("radio", { name: "Match system" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    media.dark = true;
    act(() => media.listeners.forEach((listener) => listener()));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(window.localStorage.getItem("tasknotes.theme")).toBe("system");
  });
});
