import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import PomodoroControl from "./components/PomodoroControl";
import { POMODORO_CHANGED, clock, shownTime, type PomodoroDurations, type PomodoroView } from "./lib/pomodoro-api";
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

const MIN = 60_000;
const DEFAULTS: PomodoroDurations = {
  focusMinutes: 25, shortBreakMinutes: 5, longBreakMinutes: 15, sessionsBeforeLongBreak: 4,
};
const base: Task = {
  id: "task-1", title: "Synthetic plan", description: "", status: "in_progress", priority: "medium", difficulty: null,
  wontDo: false, progress: 40, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100,
};

/**
 * A stand-in for the shell's timer behind the Tauri seam. The timer's rules
 * (Overtime, outcomes, the long break) are the core's and are tested in Rust;
 * this fake only moves between phases as a test tells it to.
 */
let view: PomodoroView;
let tasks: Task[];
let tipDue: boolean;
let shellMissing: boolean;
let failNextCommand: string | null;

const idle = (): PomodoroView => ({
  phase: "idle", title: null, durations: { ...DEFAULTS }, lastTaskId: null, now: Date.now(),
});

function focus(taskId: string | null, endsIn = 25 * MIN): PomodoroView {
  const now = Date.now();
  return {
    phase: "focus", taskId, startedAt: now - (25 * MIN - endsIn), endsAt: now + endsIn,
    title: null, durations: view.durations, lastTaskId: taskId, now,
  };
}

function shell(command: string, body: Record<string, any>) {
  if (shellMissing && /pomodoro|notification/.test(command)) throw new Error(`Command ${command} not found`);
  switch (command) {
    case "pomodoro_state": return { ...view, now: Date.now() };
    case "pomodoro_command": {
      if (failNextCommand) {
        const message = failNextCommand;
        failNextCommand = null;
        throw new Error(message);
      }
      const { action, taskId } = body.command;
      const now = Date.now();
      if (action === "start_focus") view = focus(taskId ?? null);
      if (action === "keep_focusing" && view.phase === "focus_done") view = { ...view, phase: "overtime", now };
      if (action === "start_break") view = {
        phase: "break", long: false, startedAt: now, endsAt: now + 5 * MIN, title: null,
        durations: view.durations, lastTaskId: view.lastTaskId, now,
      };
      if (action === "stop") view = { ...idle(), lastTaskId: view.lastTaskId, durations: view.durations };
      return { ...view, now };
    }
    case "set_pomodoro_silent": return null;
    case "take_pomodoro_tip": {
      const due = tipDue;
      tipDue = false;
      return due;
    }
    case "open_notification_settings": return null;
    case "set_pomodoro_durations": {
      const durations = body.durations as PomodoroDurations;
      if (durations.focusMinutes > 180) {
        throw new Error("Pomodoro lengths: focus 1–180 minutes, breaks 1–60 minutes, a long break after 1–12 sessions");
      }
      view = { ...view, durations };
      return { ...view, now: Date.now() };
    }
  }
  return undefined;
}

beforeEach(() => {
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
  vi.unstubAllGlobals();
});

const commands = (name: string) => call.mock.calls.filter(([command]) => command === name).map(([, args]) => args);

async function renderApp() {
  render(<App />);
  await screen.findByRole("article", { name: "Synthetic plan" });
  return screen.findByRole("region", { name: "Pomodoro" });
}

/** The shell tells the page the timer moved on. */
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
    expect(within(pomodoro).getByLabelText(/left$/)).toHaveTextContent(/^2[45]:\d\d$/);

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

  it("waits for a click at the focus end: Start break or Keep focusing, then Overtime", async () => {
    const user = userEvent.setup();
    view = focus("task-1");
    const pomodoro = await renderApp();
    const now = Date.now();
    await shellChanged({ ...view, phase: "focus_done", endsAt: now, now } as PomodoroView);
    expect(within(pomodoro).getByText("Focus done · Synthetic plan")).toBeInTheDocument();
    expect(within(pomodoro).getByText("Done")).toBeInTheDocument();
    expect(within(pomodoro).getByRole("button", { name: "Start break" })).toBeInTheDocument();

    await user.click(within(pomodoro).getByRole("button", { name: "Keep focusing" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "keep_focusing" } });
    expect(await within(pomodoro).findByText("Overtime · Synthetic plan")).toBeInTheDocument();
    expect(within(pomodoro).queryByRole("button", { name: "Keep focusing" })).not.toBeInTheDocument();

    await user.click(within(pomodoro).getByRole("button", { name: "Start break" }));
    expect(await within(pomodoro).findByText("Break")).toBeInTheDocument();
    // After the break the next focus keeps the Task.
    await shellChanged({ ...view, phase: "break_done", long: false } as PomodoroView);
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "start_focus", taskId: "task-1" } });
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

  it("makes the notification silent while interaction sounds are muted", async () => {
    const user = userEvent.setup();
    await renderApp();
    await waitFor(() => expect(commands("set_pomodoro_silent")).toEqual([{ silent: false }]));
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "Mute interaction sounds" }));
    await waitFor(() => expect(last(commands("set_pomodoro_silent"))).toEqual({ silent: true }));
  });

  it("leaves Ambient rain playing at its level through a whole session", async () => {
    const user = userEvent.setup();
    const pomodoro = await renderApp();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    const level = () => context.levelOf(context.playingSources()[0]);
    expect(level()).toBeCloseTo(0.08);

    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    await shellChanged({ ...view, phase: "focus_done", now: Date.now() } as PomodoroView);
    await user.click(within(pomodoro).getByRole("button", { name: "Start break" }));
    await user.click(within(pomodoro).getByRole("button", { name: "Stop" }));
    expect(context.playingSources()).toEqual([source]);
    expect(source.stoppedAt).toBeNull();
    expect(level()).toBeCloseTo(0.08);
  });

  it("says why a click failed", async () => {
    const user = userEvent.setup();
    const pomodoro = await renderApp();
    failNextCommand = "The Pomodoro Session could not be saved: Local storage failed";
    await user.click(within(pomodoro).getByRole("button", { name: "Start focus" }));
    expect(await within(pomodoro).findByRole("alert")).toHaveTextContent("could not be saved");
  });

  it("is absent where the shell has no timer", async () => {
    shellMissing = true;
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    expect(screen.queryByRole("region", { name: "Pomodoro" })).not.toBeInTheDocument();
  });
});

describe("Pomodoro lengths in Settings", () => {
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

  it("refuses lengths outside the limits before asking the shell", async () => {
    const user = userEvent.setup();
    const form = await openSettings(user);
    const focusField = within(form).getByRole("spinbutton", { name: /^Focus/ });
    await user.clear(focusField);
    await user.type(focusField, "0");
    expect(within(form).getByRole("button", { name: "Save lengths" })).toBeDisabled();
    expect(within(form).getByText(/Focus 1–180 minutes/)).toBeInTheDocument();
    expect(commands("set_pomodoro_durations")).toHaveLength(0);
  });
});

describe("Pomodoro time", () => {
  it("formats time left and Overtime as the menu bar does", () => {
    expect(clock(12 * 60 + 34)).toBe("12:34");
    expect(clock(90 * 60)).toBe("90:00");
    const running = { phase: "focus", taskId: null, startedAt: 0, endsAt: 25 * MIN } as const;
    expect(shownTime(running, 0)).toBe("25:00");
    // Rounded up, so 00:00 never shows before the end.
    expect(shownTime(running, 25 * MIN - 1)).toBe("00:01");
    const over = { phase: "overtime", taskId: null, startedAt: 0, endsAt: 25 * MIN } as const;
    expect(shownTime(over, 27 * MIN + 15_000)).toBe("+02:15");
    expect(shownTime({ phase: "idle" }, 0)).toBeNull();
  });

  it("counts down every second on the shell's clock", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });
    vi.setSystemTime(1_800_000_000_000);
    view = focus(null, 12 * MIN + 34_000);
    function Harness() {
      const pomodoro = usePomodoro({ silent: false, onTaskChanged: () => {} });
      return <PomodoroControl pomodoro={pomodoro} tasks={[]} openTaskId={null} />;
    }
    render(<Harness />);
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(screen.getByLabelText("12:34 left")).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(2000); });
    expect(screen.getByLabelText("12:32 left")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  });
});
