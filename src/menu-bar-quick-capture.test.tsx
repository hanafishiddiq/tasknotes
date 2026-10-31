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
/** Commands the fake shell fails, as when the settings file cannot be written. */
let failingSaves: Set<string>;
/** The shell's queued Quick Capture and launch notice, collected by the page. */
let pendingCapture: boolean;
let launchNotice: string | null;
let clock: number;

/** The fake shell's close hint; the real wording is the shell's (tested in Rust). */
const withHint = (view: ShellView): ShellView => ({
  ...view,
  closeHint: !view.keepInMenuBar ? "Shell says: closing quits." : "Shell says: closing keeps it in the tray.",
});

/** The shell queues the request, then tells the page. */
const quickCaptureRequested = () => act(() => {
  pendingCapture = true;
  return tauriEvents.emit(QUICK_CAPTURE_REQUESTED);
});

beforeEach(() => {
  tasks = [];
  clock = 1_000;
  shellMissing = false;
  failingSaves = new Set();
  pendingCapture = false;
  launchNotice = null;
  nextCapture = null;
  shell = withHint({
    keepInMenuBar: false, trayPlace: "menu bar", closeHint: "", quickCaptureEnabled: false,
    quickCapture: { state: "off" }, rememberedLabel: null, settingsUnreadable: false,
  });
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    if (shellMissing && command.match(/shell_settings|menu_bar|quick_capture|launch_notice/)) {
      throw new Error(`Command ${command} not found`);
    }
    if (failingSaves.has(command)) throw new Error("Could not save the setting: synthetic disk error");
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
        shell = withHint({ ...shell, keepInMenuBar: body.enabled });
        return { ...shell };
      case "take_quick_capture": {
        const pending = pendingCapture;
        pendingCapture = false;
        return pending;
      }
      case "take_launch_notice": {
        const notice = launchNotice;
        launchNotice = null;
        return notice;
      }
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
  return screen.findByRole("region", { name: /Quick Capture$/ });
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

describe("Quick Capture at launch", () => {
  it("is not lost when the shortcut was pressed before the page was listening", async () => {
    pendingCapture = true;
    render(<App />);
    await screen.findByText("Your list is clear");
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Task title" })).toHaveFocus());
    expect(pendingCapture).toBe(false);
  });

  it("says once, without blocking, that the saved shortcut could not be used, and Tasks still work", async () => {
    launchNotice = "Shell notice: ⌃⌥Space could not be used.";
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const notice = await screen.findByRole("status", { name: "Quick Capture shortcut" });
    expect(notice).toHaveTextContent("Shell notice: ⌃⌥Space could not be used. ⌘N still creates a task.");

    const field = screen.getByRole("textbox", { name: "Task title" });
    await user.type(field, "Synthetic task beside the notice{Enter}");
    await screen.findByText("Synthetic task beside the notice");

    await user.click(within(notice).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status", { name: "Quick Capture shortcut" })).toBeNull();
    expect(call.mock.calls.filter(([command]) => command === "take_launch_notice")).toHaveLength(1);
  });

  it("opens Settings from the launch notice", async () => {
    launchNotice = "Shell notice: ⌃⌥Space could not be used.";
    const user = userEvent.setup();
    render(<App />);
    const notice = await screen.findByRole("status", { name: "Quick Capture shortcut" });
    await user.click(within(notice).getByRole("button", { name: "Open Settings" }));
    expect(await screen.findByRole("region", { name: /Quick Capture$/ })).toBeVisible();
    expect(screen.queryByRole("status", { name: "Quick Capture shortcut" })).toBeNull();
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
    expect(within(section).getByText("Shell says: closing quits.")).toBeVisible();

    await user.click(keep);
    expect(call).toHaveBeenCalledWith("set_keep_in_menu_bar", { enabled: true });
    expect(keep).toBeChecked();
    expect(within(section).getByText("Shell says: closing keeps it in the tray.")).toBeVisible();
  });

  it("names the tray as this system does and shows the shell's own words for what closing does", async () => {
    shell = { ...shell, keepInMenuBar: true, trayPlace: "notification area", closeHint: "Shell says: no tray icon here." };
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    expect(section).toHaveAccessibleName("Notification area and Quick Capture");
    expect(within(section).getByRole("checkbox", { name: "Keep Tasknotes in the notification area" })).toBeChecked();
    expect(within(section).getByText("Shell says: no tray icon here.")).toBeVisible();
    expect(within(section).queryByText(/macOS|menu bar|⌘Q/)).toBeNull();
  });

  it("says calmly when the saved settings could not be read and the defaults are in use", async () => {
    shell = { ...shell, settingsUnreadable: true };
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    expect(within(section).getByRole("status")).toHaveTextContent(
      "Tasknotes could not read these saved settings, so the defaults are in use. Changing a setting saves them again.");
  });

  it("shows the shell's error when saving fails and the checkbox shows what the shell really has", async () => {
    failingSaves.add("set_keep_in_menu_bar");
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    const keep = within(section).getByRole("checkbox", { name: "Keep Tasknotes in the menu bar" });
    call.mockClear();

    await user.click(keep);
    expect(await within(section).findByRole("alert")).toHaveTextContent("Could not save the setting: synthetic disk error");
    await waitFor(() => expect(call.mock.calls.map(([command]) => command)).toContain("get_shell_settings"));
    expect(keep).not.toBeChecked();
    expect(within(section).getByText("Shell says: closing quits.")).toBeVisible();
  });

  it("degrades to a short note where the desktop shell has no such settings, and the rest of Settings works", async () => {
    shellMissing = true;
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const section = await openSettings(user);
    expect(await within(section).findByText("These settings are not available on this device."))
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
    nextCapture = () => ({ state: "rejected", reason: "Shell reason: two modifiers are needed.", current: null });
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    await user.keyboard("{Meta>}n{/Meta}");

    expect(call).toHaveBeenCalledWith("set_quick_capture", { input: { enabled: true, shortcut: "Super+KeyN" } });
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
    expect(await within(section).findByText("Shell reason: two modifiers are needed.")).toBeVisible();
    expect(within(section).getByText("⌘N still creates a task inside Tasknotes.")).toBeVisible();
    expect(within(section).getByRole("button", { name: "Press the new shortcut" })).toHaveFocus();
  });

  it("shows a fallback when another app holds the combination, and ordinary Task creation still works", async () => {
    nextCapture = () => ({ state: "unavailable", label: "⌥⇧T", reason: "Shell reason: ⌥⇧T is taken.", current: null });
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    await user.keyboard("{Alt>}{Shift>}t{/Shift}{/Alt}");

    expect(await within(section).findByText("Shell reason: ⌥⇧T is taken.")).toBeVisible();
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
      quickCapture: { state: "unavailable", label: "⌃⌥Space", reason: "Shell reason: ⌃⌥Space is taken.", current: null },
    };
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    expect(toggle).toBeChecked();
    expect(within(section).getByText("Shell reason: ⌃⌥Space is taken.")).toBeVisible();
    expect(within(section).getByRole("button", { name: "Change shortcut" })).toBeVisible();
  });

  it("refreshes from the shell when saving the shortcut fails, so the switch does not claim it is on", async () => {
    failingSaves.add("set_quick_capture");
    const user = userEvent.setup();
    const { section, toggle } = await shortcutControls(user);
    await user.click(toggle);
    await user.keyboard("{Control>}{Alt>}{ }{/Alt}{/Control}");

    expect(await within(section).findByRole("alert")).toHaveTextContent("Could not save the setting: synthetic disk error");
    await user.keyboard("{Escape}");
    expect(toggle).not.toBeChecked();
    expect(within(section).getByText("Off. ⌘N creates a task inside Tasknotes.")).toBeVisible();
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
