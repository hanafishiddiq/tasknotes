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
  title: null, plannedSeconds: null, finishedInCycle: 0,
  durations: view?.durations ?? { ...DEFAULTS }, limits: view?.limits ?? LIMITS,
  lastTaskId: view?.lastTaskId ?? null, silent: view?.silent ?? false, saveError: null,
});
const idle = (): PomodoroView => ({ ...common(), phase: "idle" });
const focus = (taskId: string | null, remainingSeconds = 25 * 60): PomodoroView =>
  ({ ...common(), phase: "focus", taskId, remainingSeconds, paused: false, plannedSeconds: 25 * 60, lastTaskId: taskId });
const focusDone = (overtimeSeconds: number): PomodoroView =>
  ({ ...common(), phase: "focus_done", taskId: view.lastTaskId, overtimeSeconds });
const onBreak = (): PomodoroView =>
  ({ ...common(), phase: "break", long: false, remainingSeconds: 5 * 60, paused: false, plannedSeconds: 5 * 60 });

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
      if (action === "start_break" || action === "count_overtime" || action === "break_early") view = onBreak();
      if ((action === "pause" || action === "resume") && (view.phase === "focus" || view.phase === "break")) {
        view = { ...view, paused: action === "pause" };
      }
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
      case "list_tasks": {
        // As the core filters them: by status, search text or one Task.
        const text = (body.text ?? "").toLowerCase();
        const items = tasks.filter((task) => (!body.status || task.status === body.status)
          && task.title.toLowerCase().includes(text) && (!body.taskId || task.id === body.taskId));
        return { items: items.map((task) => ({ ...task })), nextCursor: null };
      }
      case "pomodoro_history": return { items: [], nextCursor: null };
      case "pomodoro_totals": return { finishedSessions: 0, focusMinutes: 0 };
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

/** Opens the Pomodoro page from the sidebar. */
async function openPage(user: ReturnType<typeof userEvent.setup>) {
  await renderApp();
  await user.click(screen.getByRole("button", { name: "Pomodoro" }));
  return screen.getByRole("main", { name: "Pomodoro" });
}

describe("Pomodoro page timer (batch 3, Ticket 04)", () => {
  it("shows what the shell sends: the Task, the time, its state and the place in the cycle", async () => {
    const user = userEvent.setup();
    view = { ...focus("task-1", 14 * 60 + 32), finishedInCycle: 2 };
    const page = await openPage(user);
    expect(within(page).getByRole("timer", { name: "14:32 left" })).toHaveTextContent("14:32");
    expect(within(page).getByText("Focus")).toBeInTheDocument();
    expect(within(page).getByText("Synthetic plan")).toBeInTheDocument();
    expect(within(page).getByRole("img", { name: "Session 3 of 4 before the long break" })).toBeInTheDocument();
    // While the timer runs nothing waits for the person: Pause is not the yellow button.
    expect(within(page).getByRole("button", { name: "Pause" })).not.toHaveClass("pomo-pill--go");
  });

  it("pauses and resumes a focus, and can start the break before its end", async () => {
    const user = userEvent.setup();
    view = focus("task-1", 14 * 60 + 32);
    const page = await openPage(user);
    await user.click(within(page).getByRole("button", { name: "Pause" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "pause" } });
    expect(await within(page).findByText("Paused")).toBeInTheDocument();
    const resume = within(page).getByRole("button", { name: "Resume" });
    expect(resume).toHaveClass("pomo-pill--go");
    expect(screen.getByRole("region", { name: "Pomodoro" })).toHaveTextContent("Paused · Synthetic plan");

    await user.click(resume);
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "resume" } });
    await user.click(await within(page).findByRole("button", { name: "Start break" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "break_early" } });
    expect(await within(page).findByText("Break")).toBeInTheDocument();
  });

  it("offers Start break and Count +07:12 at the planned end, and Stop throughout", async () => {
    const user = userEvent.setup();
    view = focus("task-1");
    view = focusDone(7 * 60 + 12);
    const page = await openPage(user);
    expect(within(page).getByRole("timer", { name: "Overtime 07:12" })).toHaveTextContent("+07:12");
    expect(within(page).getByRole("button", { name: "Start break" })).toHaveClass("pomo-pill--go");
    await user.click(within(page).getByRole("button", { name: "Count +07:12" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "count_overtime" } });
    await user.click(await within(page).findByRole("button", { name: "Stop" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "stop" } });
  });

  it("offers Pause and Skip to focus in a break, and Start focus once it is over", async () => {
    const user = userEvent.setup();
    view = { ...onBreak(), lastTaskId: "task-1" };
    const page = await openPage(user);
    expect(within(page).getByText("Next: Synthetic plan")).toBeInTheDocument();
    await user.click(within(page).getByRole("button", { name: "Skip to focus" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "start_next_focus" } });
    await shellChanged({ ...common(), phase: "break_done", long: false, lastTaskId: "task-1" });
    expect(within(page).getByRole("button", { name: "Start focus" })).toHaveClass("pomo-pill--go");
    expect(within(page).queryByRole("button", { name: "Pause" })).not.toBeInTheDocument();
  });

  it("chooses the Task from a sheet of every unfinished Task, In Progress first, searched by the core", async () => {
    const user = userEvent.setup();
    tasks.push({ ...base, id: "task-3", title: "Synthetic review", status: "ready", progress: 0 });
    const page = await openPage(user);
    await user.click(within(page).getByRole("button", { name: "Focus without a Task, choose a Task" }));
    const sheet = screen.getByRole("dialog", { name: "Task for the focus" });
    const search = within(sheet).getByRole("combobox", { name: "Search Tasks" });
    expect(search).toHaveFocus();
    await waitFor(() => expect(within(sheet).getAllByRole("option")).toHaveLength(3));
    expect(within(sheet).getAllByRole("option").map((option) => option.textContent))
      .toEqual(["No Task", "Synthetic planInbox", "Synthetic reviewInbox"]);
    expect(within(sheet).getByRole("group", { name: "In Progress" })).toHaveTextContent("Synthetic plan");
    expect(within(sheet).getByRole("group", { name: "Ready" })).toHaveTextContent("Synthetic review");

    await user.type(search, "review");
    await waitFor(() => expect(within(sheet).getAllByRole("option")).toHaveLength(1));
    expect(commands("list_tasks")).toContainEqual(expect.objectContaining({ text: "review", status: "ready" }));
    await user.keyboard("{Enter}");
    expect(screen.queryByRole("dialog", { name: "Task for the focus" })).not.toBeInTheDocument();
    const line = within(page).getByRole("button", { name: "Focus on “Synthetic review”, choose a Task" });
    expect(line).toHaveFocus();

    await user.click(within(page).getByRole("button", { name: "Start" }));
    expect(last(commands("pomodoro_command"))).toEqual({ command: { action: "start_focus", taskId: "task-3" } });
  });

  it("closes the Task sheet with Escape and gives focus back", async () => {
    const user = userEvent.setup();
    const page = await openPage(user);
    const line = within(page).getByRole("button", { name: "Focus without a Task, choose a Task" });
    await user.click(line);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Task for the focus" })).not.toBeInTheDocument();
    expect(line).toHaveFocus();
  });
});

describe("Pomodoro lengths (batch 3, behind the icon on the page)", () => {
  async function openLengths(user: ReturnType<typeof userEvent.setup>) {
    const page = await openPage(user);
    await user.click(within(page).getByRole("button", { name: "Pomodoro lengths" }));
    return screen.getByRole("dialog", { name: "Pomodoro lengths" });
  }

  it("shows 25 / 5 / 15 and a long break after 4, and saves a change", async () => {
    const user = userEvent.setup();
    const sheet = await openLengths(user);
    expect(within(sheet).getByRole("spinbutton", { name: "Focus" })).toHaveValue(25);
    expect(within(sheet).getByRole("spinbutton", { name: "Break" })).toHaveValue(5);
    expect(within(sheet).getByRole("spinbutton", { name: "Long break" })).toHaveValue(15);
    expect(within(sheet).getByRole("spinbutton", { name: "Long break after" })).toHaveValue(4);
    expect(within(sheet).queryByRole("button", { name: "Save" })).not.toBeInTheDocument();

    await user.click(within(sheet).getByRole("button", { name: "More focus" }));
    expect(within(sheet).getByRole("spinbutton", { name: "Focus" })).toHaveValue(26);
    const focusField = within(sheet).getByRole("spinbutton", { name: "Focus" });
    await user.clear(focusField);
    await user.type(focusField, "50");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    expect(commands("set_pomodoro_durations")).toEqual([{ durations: { ...DEFAULTS, focusMinutes: 50 } }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Pomodoro lengths" })).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Pomodoro lengths" })).toHaveFocus();
  });

  it("refuses lengths outside the shell's limits before asking it", async () => {
    const user = userEvent.setup();
    // The window keeps no copy of the limits: it uses what the shell sends.
    view = { ...idle(), limits: { ...LIMITS, focusMinutes: { min: 1, max: 120 } } };
    const sheet = await openLengths(user);
    const focusField = within(sheet).getByRole("spinbutton", { name: "Focus" });
    expect(focusField).toHaveAttribute("max", "120");
    await user.clear(focusField);
    await user.type(focusField, "150");
    expect(within(sheet).getByRole("button", { name: "Save" })).toBeDisabled();
    expect(within(sheet).getByText(/Focus 1–120 minutes, breaks 1–60 minutes, a long break after 1–12 sessions/))
      .toBeInTheDocument();
    expect(commands("set_pomodoro_durations")).toHaveLength(0);
  });

  it("has a Silent setting of its own, apart from interaction sounds and Ambient rain", async () => {
    const user = userEvent.setup();
    const sheet = await openLengths(user);
    const silent = within(sheet).getByRole("checkbox", { name: "Silent notifications" });
    expect(silent).not.toBeChecked();
    await user.click(silent);
    expect(commands("set_pomodoro_silent")).toEqual([{ silent: true }]);
    await waitFor(() => expect(within(sheet).getByRole("checkbox", { name: "Silent notifications" })).toBeChecked());
  });

  it("leaves Settings one line that leads to the page", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("spinbutton", { name: /Focus/ })).not.toBeInTheDocument();
    expect(screen.getByText("Focus and break lengths and Silent notifications are on the Pomodoro page.")).toBeInTheDocument();
    // Muting interaction sounds never touches Pomodoro notifications.
    await user.click(screen.getByRole("button", { name: "Mute interaction sounds" }));
    expect(commands("set_pomodoro_silent")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Open Pomodoro" }));
    expect(screen.getByRole("main", { name: "Pomodoro" })).toBeVisible();
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
