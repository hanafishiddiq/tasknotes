import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import PomodoroControl from "./components/PomodoroControl";
import {
  POMODORO_CHANGED, clock, type DurationLimits, type PomodoroDurations, type PomodoroView,
} from "./lib/pomodoro-api";
import { usePomodoro } from "./lib/use-pomodoro";
import type { Task } from "./lib/task-api";
import { FakeAudioContext, last } from "./test/fake-audio";
import { tauriEvents } from "./test/tauri-events";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

// The real rain takes a few dozen milliseconds to render; tests only need a buffer.
vi.mock("./lib/sound/rain", () => ({
  renderRainInSlices: async (rate: number) => [new Float32Array(rate / 10), new Float32Array(rate / 10)],
}));

const DEFAULTS: PomodoroDurations = {
  focusMinutes: 25, shortBreakMinutes: 5, longBreakMinutes: 15, sessionsBeforeLongBreak: 4,
};
/** As the shell sends them; a test may change them to show the window keeps no copy. */
const LIMITS: DurationLimits = {
  focusMinutes: { min: 1, max: 180 },
  shortBreakMinutes: { min: 1, max: 60 },
  longBreakMinutes: { min: 1, max: 60 },
  sessionsBeforeLongBreak: { min: 1, max: 12 },
};
const base: Task = {
  id: "task-1", title: "Synthetic plan", description: "", status: "in_progress", priority: "medium", difficulty: null,
  wontDo: false, progress: 40, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100,
};

/**
 * A stand-in for the shell's timer behind the Tauri seam. The timer's rules
 * (Overtime, outcomes, the long break, the seconds shown) are the core's and
 * the shell's and are tested in Rust; this fake only moves between phases as
 * a test tells it to.
 */
let view: PomodoroView;
let tasks: Task[];
let tipDue: boolean;
let shellMissing: boolean;
let failNextCommand: string | null;

const common = () => ({
  title: null, durations: view?.durations ?? { ...DEFAULTS }, limits: view?.limits ?? LIMITS,
  lastTaskId: view?.lastTaskId ?? null, silent: view?.silent ?? false, saveError: null,
});
const idle = (): PomodoroView => ({ ...common(), phase: "idle" });
const focus = (taskId: string | null, remainingSeconds = 25 * 60): PomodoroView =>
  ({ ...common(), phase: "focus", taskId, remainingSeconds, lastTaskId: taskId });
const focusDone = (overtimeSeconds: number): PomodoroView =>
  ({ ...common(), phase: "focus_done", taskId: view.lastTaskId, overtimeSeconds });
const onBreak = (): PomodoroView => ({ ...common(), phase: "break", long: false, remainingSeconds: 5 * 60 });

function shell(command: string, body: Record<string, any>) {
  if (shellMissing && /pomodoro|notification/.test(command)) throw new Error(`Command ${command} not found`);
  switch (command) {
    case "pomodoro_state": return view;
    case "pomodoro_command": {
      if (failNextCommand) {
        const message = failNextCommand;
        failNextCommand = null;
        throw new Error(message);
      }
      const { action, taskId } = body.command;
      if (action === "start_focus") view = focus(taskId ?? null);
      if (action === "start_next_focus") view = focus(view.lastTaskId);
      if (action === "start_break" || action === "count_overtime") view = onBreak();
      if (action === "stop") view = idle();
      if (action === "retry_save" || action === "discard_unsaved") view = { ...view, saveError: null };
      return view;
    }
    case "set_pomodoro_silent": {
      view = { ...view, silent: body.silent };
      return view;
    }
    case "take_pomodoro_tip": {
      const due = tipDue;
      tipDue = false;
      return due;
    }
    case "open_notification_settings": return null;
    case "set_pomodoro_durations": {
      const durations = body.durations as PomodoroDurations;
      if (durations.focusMinutes > view.limits.focusMinutes.max) {
        throw new Error("Pomodoro lengths: focus 1–180 minutes, breaks 1–60 minutes, a long break after 1–12 sessions");
      }
      view = { ...view, durations };
      return view;
    }
  }
  return undefined;
}

beforeEach(() => {
  view = undefined as unknown as PomodoroView;
  view = idle();
  tasks = [{ ...base }, { ...base, id: "task-2", title: "Synthetic done", status: "completed", progress: 100 }];
  tipDue = false;
  shellMissing = false;
  failNextCommand = null;
  window.localStorage.clear();
  FakeAudioContext.reset();
  vi.stubGlobal("AudioContext", FakeAudioContext);
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    const answered = shell(command, body);
    if (answered !== undefined) return answered;
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: tasks.map((task) => ({ ...task })), nextCursor: null };
      case "list_removed": return { items: [], nextCursor: null };
      case "list_progress_entries": return { items: [], nextCursor: null };
      case "list_task_notes": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const commands = (name: string) => call.mock.calls.filter(([command]) => command === name).map(([, args]) => args);

async function renderApp() {
  render(<App />);
  await screen.findByRole("article", { name: "Synthetic plan" });
  return screen.findByRole("region", { name: "Pomodoro" });
}

/** The shell tells the page what the timer shows now. */
const shellChanged = (next: PomodoroView, taskChanged: string | null = null) => act(() => {
  view = next;
  return tauriEvents.emit(POMODORO_CHANGED, { view: next, taskChanged });
});

describe("Pomodoro control (Ticket 02)", () => {
  it("starts a focus without a Task, or for one unfinished Task", async () => {
    const user = userEvent.setup();
    const pomodoro = await renderApp();
    const choice = within(pomodoro).getByRole("combobox", { name: "Task for this Pomodoro" });
    // Only unfinished Tasks are offered.
    expect(within(choice).getAllByRole("option").map((option) => option.textContent))
      .toEqual(["No Task", "Synthetic plan"]);

    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(commands("pomodoro_command")).toEqual([{ command: { action: "start_focus", taskId: null } }]);
    expect(await within(pomodoro).findByText("Focus")).toBeInTheDocument();
    expect(within(pomodoro).getByLabelText("25:00 left")).toBeInTheDocument();

    await user.click(within(pomodoro).getByRole("button", { name: "Stop" }));
    await user.selectOptions(within(pomodoro).getByRole("combobox", { name: "Task for this Pomodoro" }), "task-1");
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "start_focus", taskId: "task-1" } });
    expect(await within(pomodoro).findByText("Focus · Synthetic plan")).toBeInTheDocument();
  });

  it("offers the open Task first", async () => {
    const user = userEvent.setup();
    const pomodoro = await renderApp();
    await user.click(screen.getByRole("button", { name: "Open Synthetic plan" }));
    expect(within(pomodoro).getByRole("combobox", { name: "Task for this Pomodoro" })).toHaveValue("task-1");
  });

  it("shows the running Overtime at the planned end and offers Start break or Count it", async () => {
    const user = userEvent.setup();
    view = focus("task-1");
    const pomodoro = await renderApp();
    await shellChanged(focusDone(7 * 60 + 12));
    expect(within(pomodoro).getByText("Focus done · Synthetic plan")).toBeInTheDocument();
    expect(within(pomodoro).getByLabelText("Overtime 07:12")).toHaveTextContent("+07:12");
    expect(within(pomodoro).queryByRole("button", { name: /Keep focusing/ })).not.toBeInTheDocument();
    // The count's label follows the shell's seconds.
    await shellChanged(focusDone(7 * 60 + 13));
    await user.click(within(pomodoro).getByRole("button", { name: "Count +07:13" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "count_overtime" } });
    expect(await within(pomodoro).findByText("Break")).toBeInTheDocument();

    await user.click(within(pomodoro).getByRole("button", { name: "Stop" }));
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    await shellChanged(focusDone(30));
    await user.click(within(pomodoro).getByRole("button", { name: "Start break" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "start_break" } });
  });

  it("starts the next focus for the shell's last Task, even one not on this page", async () => {
    const user = userEvent.setup();
    view = { ...onBreak(), lastTaskId: "task-elsewhere" };
    const pomodoro = await renderApp();
    await shellChanged({ ...common(), phase: "break_done", long: false, lastTaskId: "task-elsewhere" });
    expect(within(pomodoro).getByText("Break over")).toBeInTheDocument();
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "start_next_focus" } });
    expect(await within(pomodoro).findByText("Focus · a Task")).toBeInTheDocument();
  });

  it("reads the Task list again when a finished session added a Progress Entry", async () => {
    view = focus("task-1");
    await renderApp();
    const reads = commands("list_tasks").length;
    tasks[0] = { ...tasks[0], revision: 2, updatedAt: 200 };
    await shellChanged({ ...idle(), lastTaskId: "task-1" }, "task-1");
    await waitFor(() => expect(commands("list_tasks").length).toBeGreaterThan(reads));
  });

  it("shows a one-time tip about the notification style on the first start", async () => {
    const user = userEvent.setup();
    tipDue = true;
    const pomodoro = await renderApp();
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    const tip = await within(pomodoro).findByRole("note", { name: "Notification tip" });
    expect(tip).toHaveTextContent("Persistent");
    await user.click(within(tip).getByRole("button", { name: "Open Notification Settings" }));
    expect(commands("open_notification_settings")).toHaveLength(1);
    await waitFor(() => expect(within(pomodoro).queryByRole("note")).not.toBeInTheDocument());

    await user.click(within(pomodoro).getByRole("button", { name: "Stop" }));
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(commands("take_pomodoro_tip")).toHaveLength(1);
  });

  it("keeps a session that could not be saved on screen until it is saved or discarded", async () => {
    const user = userEvent.setup();
    view = { ...idle(), saveError: "The Pomodoro Session could not be saved: Local storage failed" };
    const pomodoro = await renderApp();
    const alert = within(pomodoro).getByRole("alert");
    expect(alert).toHaveTextContent("could not be saved");
    await user.click(within(alert).getByRole("button", { name: "Try again" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "retry_save" } });
    await waitFor(() => expect(within(pomodoro).queryByRole("alert")).not.toBeInTheDocument());

    await shellChanged({ ...idle(), saveError: "The Pomodoro Session could not be saved: busy" });
    await user.click(within(pomodoro).getByRole("button", { name: "Discard" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "discard_unsaved" } });
  });

  it("leaves Ambient rain playing at its level through a whole session", async () => {
    const user = userEvent.setup();
    const pomodoro = await renderApp();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    const level = () => context.levelOf(context.playingSources()[0]);
    expect(level()).toBeCloseTo(0.08);

    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    await shellChanged(focusDone(0));
    await user.click(within(pomodoro).getByRole("button", { name: "Start break" }));
    await user.click(within(pomodoro).getByRole("button", { name: "Stop" }));
    expect(context.playingSources()).toEqual([source]);
    expect(source.stoppedAt).toBeNull();
    expect(level()).toBeCloseTo(0.08);
  });

  it("says why a click failed", async () => {
    const user = userEvent.setup();
    const pomodoro = await renderApp();
    failNextCommand = "Command pomodoro_command failed";
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(await within(pomodoro).findByRole("alert")).toHaveTextContent("failed");
  });

  it("is absent where the shell has no timer", async () => {
    shellMissing = true;
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    expect(screen.queryByRole("region", { name: "Pomodoro" })).not.toBeInTheDocument();
  });
});

describe("Pomodoro settings", () => {
  async function openSettings(user: ReturnType<typeof userEvent.setup>) {
    await renderApp();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    return screen.getByRole("form", { name: "Pomodoro" });
  }

  it("shows 25 / 5 / 15 and a long break after 4, and saves a change", async () => {
    const user = userEvent.setup();
    const form = await openSettings(user);
    expect(within(form).getByRole("spinbutton", { name: /^Focus/ })).toHaveValue(25);
    expect(within(form).getByRole("spinbutton", { name: /^Break/ })).toHaveValue(5);
    expect(within(form).getByRole("spinbutton", { name: /^Long break(?! after)/ })).toHaveValue(15);
    expect(within(form).getByRole("spinbutton", { name: /^Long break after/ })).toHaveValue(4);
    expect(within(form).queryByRole("button", { name: "Save lengths" })).not.toBeInTheDocument();

    const focusField = within(form).getByRole("spinbutton", { name: /^Focus/ });
    await user.clear(focusField);
    await user.type(focusField, "50");
    await user.click(within(form).getByRole("button", { name: "Save lengths" }));
    expect(commands("set_pomodoro_durations")).toEqual([{ durations: { ...DEFAULTS, focusMinutes: 50 } }]);
    expect(await within(form).findByRole("status")).toHaveTextContent("next focus or break");
    expect(within(form).getByRole("spinbutton", { name: /^Focus/ })).toHaveValue(50);
  });

  it("refuses lengths outside the shell's limits before asking it", async () => {
    const user = userEvent.setup();
    // The window keeps no copy of the limits: it uses what the shell sends.
    view = { ...idle(), limits: { ...LIMITS, focusMinutes: { min: 1, max: 120 } } };
    const form = await openSettings(user);
    const focusField = within(form).getByRole("spinbutton", { name: /^Focus/ });
    expect(focusField).toHaveAttribute("max", "120");
    await user.clear(focusField);
    await user.type(focusField, "150");
    expect(within(form).getByRole("button", { name: "Save lengths" })).toBeDisabled();
    expect(within(form).getByText(/Focus 1–120 minutes, breaks 1–60 minutes, a long break after 1–12 sessions/))
      .toBeInTheDocument();
    await user.clear(focusField);
    await user.type(focusField, "0");
    expect(within(form).getByRole("button", { name: "Save lengths" })).toBeDisabled();
    expect(commands("set_pomodoro_durations")).toHaveLength(0);
  });

  it("has a Silent setting of its own, apart from interaction sounds and Ambient rain", async () => {
    const user = userEvent.setup();
    const form = await openSettings(user);
    // Muting interaction sounds no longer touches Pomodoro notifications.
    await user.click(screen.getByRole("button", { name: "Mute interaction sounds" }));
    expect(commands("set_pomodoro_silent")).toHaveLength(0);

    const silent = within(form).getByRole("checkbox", { name: "Silent notifications" });
    expect(silent).not.toBeChecked();
    await user.click(silent);
    expect(commands("set_pomodoro_silent")).toEqual([{ silent: true }]);
    await waitFor(() => expect(within(form).getByRole("checkbox", { name: "Silent notifications" })).toBeChecked());
  });
});

describe("Pomodoro time", () => {
  it("formats the shell's seconds as the menu bar does", () => {
    expect(clock(12 * 60 + 34)).toBe("12:34");
    expect(clock(90 * 60)).toBe("90:00");
    expect(clock(0)).toBe("00:00");
  });

  it("shows only what the shell sends, with no clock of its own", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout", "Date"] });
    const intervals = vi.spyOn(window, "setInterval");
    view = focus(null, 12 * 60 + 34);
    function Harness() {
      const pomodoro = usePomodoro({ onTaskChanged: () => {} });
      return <PomodoroControl pomodoro={pomodoro} tasks={[]} openTaskId={null} />;
    }
    render(<Harness />);
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(screen.getByLabelText("12:34 left")).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(3000); });
    expect(screen.getByLabelText("12:34 left")).toBeInTheDocument();
    expect(intervals).not.toHaveBeenCalled();
    await shellChanged(focus(null, 12 * 60 + 31));
    expect(screen.getByLabelText("12:31 left")).toBeInTheDocument();
  });
});
