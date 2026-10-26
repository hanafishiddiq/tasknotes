import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { QUICK_CAPTURE_REQUESTED, type QuickCaptureStatus, type ShellView } from "./lib/shell-api";
import type { Task } from "./lib/task-api";
import { tauriEvents } from "./test/tauri-events";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * A stand-in for the core and the desktop shell behind the Tauri seam, with
 * synthetic Tasks only. Which combinations are accepted is the shell's rule
 * (tested in Rust); here the fake shell answers as a test tells it to.
 */
let tasks: Task[];
let shell: ShellView;
/** What the fake shell answers to the next set_quick_capture. */
let nextCapture: ((shortcut: string | null) => QuickCaptureStatus) | null;
let shellMissing: boolean;
let clock: number;

const quickCaptureRequested = () => act(() => tauriEvents.emit(QUICK_CAPTURE_REQUESTED));

beforeEach(() => {
  tasks = [];
  clock = 1_000;
  shellMissing = false;
  nextCapture = null;
  shell = {
    keepInMenuBar: false, menuBarUnavailable: false, quickCaptureEnabled: false, quickCapture: { state: "off" },
    rememberedLabel: null, settingsUnreadable: false,
  };
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    if (shellMissing && ["get_shell_settings", "set_keep_in_menu_bar", "set_quick_capture"].includes(command)) {
      throw new Error(`Command ${command} not found`);
    }
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: tasks.map((item) => ({ ...item })), nextCursor: null };
      case "create_task": {
        const stamp = ++clock;
        const created: Task = {
          id: `task-${stamp}`, title: body.input.title, description: body.input.description ?? "",
          status: "ready", priority: "medium", progress: 0, nextStep: "", revision: 1,
          createdAt: stamp, updatedAt: stamp, listId: null, dueDate: null, tags: [],
        };
        tasks.push(created);
        return created;
      }
      case "list_removed": return { items: [], nextCursor: null };
      case "list_notes": return { items: [], nextCursor: null };
      case "list_note_folders": return { items: [], nextCursor: null };
      case "get_shell_settings": return { ...shell };
      case "set_keep_in_menu_bar":
        shell = { ...shell, keepInMenuBar: body.enabled };
        return { ...shell };
      case "set_quick_capture": {
        const { enabled, shortcut } = body.input as { enabled: boolean; shortcut: string | null };
        const status: QuickCaptureStatus = !enabled ? { state: "off" }
          : nextCapture ? nextCapture(shortcut) : { state: "active", label: "⌃⌥Space" };
        if (status.state === "active") shell = { ...shell, rememberedLabel: status.label, quickCaptureEnabled: true };
        if (!enabled) shell = { ...shell, quickCaptureEnabled: false };
        shell = { ...shell, quickCapture: status };
        return { ...shell };
      }
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => cleanup());

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Settings" }));
  return screen.findByRole("region", { name: "Menu bar and Quick Capture" });
}

describe("Quick Capture from the global shortcut", () => {
  it("brings the ordinary New task field into focus from any place, and the Task is created as usual", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    await user.click(screen.getByRole("button", { name: "Settings" }));

    await quickCaptureRequested();
    const field = screen.getByRole("textbox", { name: "Task title" });
    await waitFor(() => expect(field).toHaveFocus());

    await user.type(field, "Synthetic captured task{Enter}");
    await screen.findByText("Synthetic captured task");
    expect(call).toHaveBeenCalledWith("create_task", expect.objectContaining({
      input: expect.objectContaining({ title: "Synthetic captured task" }),
    }));
  });
});

describe("Menu bar setting", () => {
  it("is off by default, says that closing quits, and turning it on is saved by the shell", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);

    const keep = await within(section).findByRole("checkbox", { name: "Keep Tasknotes in the menu bar" });
    expect(keep).not.toBeChecked();
    expect(within(section).getByText("Closing the window quits Tasknotes.")).toBeVisible();

    await user.click(keep);
    expect(call).toHaveBeenCalledWith("set_keep_in_menu_bar", { enabled: true });
    expect(keep).toBeChecked();
    expect(within(section).getByText(/Closing the window leaves Tasknotes in the menu bar/)).toBeVisible();
  });

  it("explains when this system could not show the menu-bar item", async () => {
    shell = { ...shell, keepInMenuBar: true, menuBarUnavailable: true };
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    expect(await within(section).findByText(/could not show its menu-bar item here, so closing the window still quits/))
      .toBeVisible();
  });

  it("degrades to a short note where the desktop shell has no such settings, and the rest of Settings works", async () => {
    shellMissing = true;
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    expect(await within(section).findByText("Menu bar and Quick Capture are not available on this device."))
      .toBeVisible();
    expect(screen.getByRole("radio", { name: "Light" })).toBeEnabled();
  });
});

describe("Quick Capture shortcut setting", () => {
  async function shortcutControls(user: ReturnType<typeof userEvent.setup>) {
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    const toggle = await within(section).findByRole("checkbox", { name: "Open Quick Capture with a global shortcut" });
    return { section, toggle };
  }

  it("is off by default and registers nothing", async () => {
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    expect(toggle).not.toBeChecked();
    expect(within(section).getByText("Off. ⌘N creates a task inside Tasknotes.")).toBeVisible();
    expect(call).not.toHaveBeenCalledWith("set_quick_capture", expect.anything());
  });

  it("turns on once a combination is recorded, and tells how to use it", async () => {
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    const recorder = within(section).getByRole("button", { name: "Press the new shortcut" });
    expect(recorder).toHaveFocus();

    await user.keyboard("{Control>}{Alt>}{ }{/Alt}{/Control}");
    expect(call).toHaveBeenCalledWith("set_quick_capture", { input: { enabled: true, shortcut: "Control+Alt+Space" } });
    expect(await within(section).findByText("Press ⌃⌥Space in any app to add a task.")).toBeVisible();
    expect(toggle).toBeChecked();
  });

  it("keeps an in-app shortcut pressed while recording from acting, and explains the refusal", async () => {
    nextCapture = () => ({ state: "rejected", problem: { kind: "needsTwoModifiers" }, current: null });
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    await user.keyboard("{Meta>}n{/Meta}");

    expect(call).toHaveBeenCalledWith("set_quick_capture", { input: { enabled: true, shortcut: "Super+KeyN" } });
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
    expect(await within(section).findByText(/Use at least two modifier keys/)).toBeVisible();
    expect(within(section).getByRole("button", { name: "Press the new shortcut" })).toHaveFocus();
  });

  it("shows a fallback when another app holds the combination, and ordinary Task creation still works", async () => {
    nextCapture = () => ({ state: "unavailable", label: "⌥⇧T", current: null });
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    await user.keyboard("{Alt>}{Shift>}t{/Shift}{/Alt}");

    expect(await within(section).findByText(/⌥⇧T is already used by another app or by macOS/)).toBeVisible();
    expect(within(section).getByText("⌘N still creates a task inside Tasknotes.")).toBeVisible();

    await user.keyboard("{Escape}");
    expect(toggle).not.toBeChecked();
    await user.keyboard("{Meta>}n{/Meta}");
    expect(screen.getByRole("textbox", { name: "Task title" })).toHaveFocus();
    expect(screen.getByRole("textbox", { name: "Task title" })).toBeEnabled();
  });

  it("reports a saved shortcut that could not be registered at launch", async () => {
    shell = {
      ...shell, quickCaptureEnabled: true, rememberedLabel: "⌃⌥Space",
      quickCapture: { state: "unavailable", label: "⌃⌥Space", current: null },
    };
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    expect(toggle).toBeChecked();
    expect(within(section).getByText(/⌃⌥Space is already used by another app or by macOS/)).toBeVisible();
    expect(within(section).getByRole("button", { name: "Change shortcut" })).toBeVisible();
  });

  it("turning it off keeps the combination for next time", async () => {
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    await user.keyboard("{Control>}{Alt>}{ }{/Alt}{/Control}");
    await within(section).findByText("Press ⌃⌥Space in any app to add a task.");

    await user.click(toggle);
    expect(call).toHaveBeenCalledWith("set_quick_capture", { input: { enabled: false, shortcut: null } });
    expect(await within(section).findByText("Off. ⌃⌥Space is kept for when you turn it back on.")).toBeVisible();

    call.mockClear();
    await user.click(toggle);
    // The remembered combination is used again without recording.
    expect(call).toHaveBeenCalledWith("set_quick_capture", { input: { enabled: true, shortcut: null } });
  });
});
