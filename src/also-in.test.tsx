import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { Task, TaskList } from "./lib/task-api";
import App from "./App";

/**
 * One Task in several Lists (owner, batch 4, 2026-11-18): the same Task in
 * each, added from the Task menu, the open Task or by Option-dragging it onto
 * a List; in a List it shares, Remove offers this List only or every List.
 * The fake core keeps what it is sent; the rules are the core's.
 */
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const listOf = (id: string, name: string): TaskList =>
  ({ id, name, revision: 1, createdAt: 1, updatedAt: 1, removed: false, parentId: null, hasChildren: false } as TaskList);
const PERSONAL = listOf("11111111-1111-4111-8111-111111111111", "Synthetic Personal");
const BIG = listOf("22222222-2222-4222-8222-222222222222", "Synthetic Big Task");
let trip: Task;

const calls = (name: string) => invokeMock.mock.calls.filter(([command]) => command === name).map(([, args]) => args);

beforeEach(() => {
  window.localStorage.clear();
  trip = {
    id: "trip", title: "Synthetic trip", description: "", status: "ready", priority: "medium", difficulty: null,
    wontDo: false, progress: 0, nextStep: "", listId: PERSONAL.id, dueDate: null, tags: [], revision: 1,
    createdAt: 100, updatedAt: 100, alsoIn: [],
  };
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: [PERSONAL, BIG], nextCursor: null };
      case "list_tasks": {
        const view = body.view as { kind: string; listId?: string } | undefined;
        const inView = !view || view.kind !== "list" || trip.listId === view.listId || (trip.alsoIn ?? []).includes(view.listId!);
        return { items: inView ? [{ ...trip }] : [], nextCursor: null };
      }
      case "show_task_in": {
        trip = { ...trip, alsoIn: [...(trip.alsoIn ?? []), body.input.listId], revision: trip.revision + 1 };
        return { ...trip };
      }
      case "hide_task_from": {
        const listId = body.input.listId as string;
        trip = trip.listId === listId
          ? { ...trip, listId: trip.alsoIn![0], alsoIn: trip.alsoIn!.slice(1), revision: trip.revision + 1 }
          : { ...trip, alsoIn: trip.alsoIn!.filter((id) => id !== listId), revision: trip.revision + 1 };
        return { ...trip };
      }
      case "list_subtasks": return [];
      case "list_task_notes": case "list_progress_entries": case "list_removed": return { items: [], nextCursor: null };
      default: return null;
    }
  });
});
afterEach(cleanup);

async function openTrip(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Open Synthetic trip" }));
  return screen.getByRole("region", { name: "Task details" });
}

describe("one Task in several Lists", () => {
  it("is shown in another List from the open Task, and shows where else it is", async () => {
    const user = userEvent.setup();
    render(<App />);
    const page = await openTrip(user);
    await user.selectOptions(within(page).getByRole("combobox", { name: "Also show in" }), BIG.id);
    expect(calls("show_task_in")).toEqual([{ input: { id: "trip", expectedRevision: 1, listId: BIG.id } }]);
    const chip = await within(page).findByText("Also in Synthetic Big Task");
    expect(chip).toBeVisible();
    // Its own List is never offered again, nor one it is in already.
    await waitFor(() => expect(within(page).queryByRole("combobox", { name: "Also show in" })).toBeNull());

    await user.click(within(page).getByRole("button", { name: "Stop showing in Synthetic Big Task" }));
    expect(calls("hide_task_from")).toEqual([{ input: { id: "trip", expectedRevision: 2, listId: BIG.id } }]);
  });

  it("is shown in another List from the Task menu", async () => {
    const user = userEvent.setup();
    render(<App />);
    const row = await screen.findByRole("article", { name: "Synthetic trip" });
    fireEvent.contextMenu(within(row).getByRole("button", { name: "Open Synthetic trip" }));
    await user.click(screen.getByRole("menuitem", { name: "Also show in" }));
    const lists = screen.getByRole("menu", { name: "Also show in" });
    expect(within(lists).getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Synthetic Big Task"]);
    await user.click(within(lists).getByRole("menuitem", { name: "Synthetic Big Task" }));
    await waitFor(() => expect(calls("show_task_in")).toHaveLength(1));
  });

  it("offers Remove from this List or from every List in a List it shares", async () => {
    trip = { ...trip, alsoIn: [BIG.id] };
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Synthetic Big Task" }));
    const page = await openTrip(user);
    expect(within(page).getByRole("button", { name: /Move to Removed from all Lists/ })).toBeInTheDocument();
    await user.click(within(page).getByRole("button", { name: "Remove from this List" }));
    expect(calls("hide_task_from")).toEqual([{ input: { id: "trip", expectedRevision: 1, listId: BIG.id } }]);
  });

  it("offers only Move to Removed for a Task in one List", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Synthetic Personal" }));
    const page = await openTrip(user);
    expect(within(page).queryByRole("button", { name: "Remove from this List" })).toBeNull();
    expect(within(page).getByRole("button", { name: /^Move to Removed$/ })).toBeInTheDocument();
  });

  it("is shown in a List it is dragged onto with Option", async () => {
    render(<App />);
    const row = await screen.findByRole("article", { name: "Synthetic trip" });
    const sidebarRow = (await screen.findByRole("button", { name: "Synthetic Big Task" })).closest<HTMLElement>("[data-list-target]")!;
    const from = within(row).getByRole("button", { name: "Open Synthetic trip" });
    document.elementFromPoint = vi.fn(() => sidebarRow);
    fireEvent.mouseDown(from, { button: 0, clientX: 300, clientY: 100, altKey: true });
    fireEvent.mouseMove(window, { clientX: 100, clientY: 200, altKey: true });
    expect(sidebarRow).toHaveClass("drop-also-here");
    fireEvent.mouseUp(window, { clientX: 100, clientY: 200, altKey: true });
    await waitFor(() => expect(calls("show_task_in")).toEqual([{ input: { id: "trip", expectedRevision: 1, listId: BIG.id } }]));
    expect(sidebarRow).not.toHaveClass("drop-also-here");
  });
});
