import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Task } from "./lib/task-api";
import { QUIT_REQUESTED } from "./lib/quit";
import { FakeAudioContext } from "./test/fake-audio";
import { tauriEvents } from "./test/tauri-events";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

// The real rain takes a few dozen milliseconds to render; tests only need a buffer.
vi.mock("./lib/sound/rain", () => ({
  renderRainInSlices: async (rate: number) => [new Float32Array(rate / 10), new Float32Array(rate / 10)],
}));

const base: Task = {
  id: "task-1", title: "Synthetic plan", description: "", status: "ready", priority: "medium",
  progress: 0, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100,
};
let saved: Task[];
let reducedMotion: boolean;

beforeEach(() => {
  saved = [{ ...base }];
  reducedMotion = false;
  window.localStorage.clear();
  FakeAudioContext.reset();
  vi.stubGlobal("AudioContext", FakeAudioContext);
  window.matchMedia = ((query: string) => ({
    matches: query.includes("prefers-reduced-motion: reduce") ? reducedMotion : query.includes("dark"),
    media: query, addEventListener: () => undefined, removeEventListener: () => undefined,
    addListener: () => undefined, removeListener: () => undefined, onchange: null, dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const input = (args as { input?: Record<string, unknown> } | undefined)?.input ?? {};
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: saved.map((task) => ({ ...task })), nextCursor: null };
      case "create_task": {
        const created = { ...base, id: `task-${saved.length + 1}`, title: String(input.title) };
        saved.push(created);
        return created;
      }
      case "change_task": {
        const task = saved.find((item) => item.id === input.id)!;
        Object.assign(task, input.status ? { status: input.status } : {}, { revision: task.revision + 1 });
        if (task.status === "completed") task.progress = 100;
        return { ...task };
      }
      case "quit_ready": return null;
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Settings" }));
  return screen.getByRole("group", { name: "Sound" });
}

function ambientLevel(): number {
  const context = FakeAudioContext.latest();
  const [source] = context.playingSources();
  return source ? context.levelOf(source) : 0;
}

async function complete(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.click(screen.getByRole("button", { name: `Open ${title}` }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Work status" }), "completed");
  await waitFor(() => expect(saved.find((task) => task.title === title)?.status).toBe("completed"));
}

describe("Sound settings", () => {
  it("starts with both channels on, interaction at 12% and ambient at 8%", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    const sound = await openSettings(user);
    expect(within(sound).getByRole("slider", { name: "Interaction sounds volume" })).toHaveValue("12");
    expect(within(sound).getByRole("slider", { name: "Ambient rain volume" })).toHaveValue("8");
    expect(within(sound).getByRole("button", { name: "Mute interaction sounds" })).toHaveAttribute("aria-pressed", "false");
    expect(within(sound).getByRole("button", { name: "Mute ambient rain" })).toHaveAttribute("aria-pressed", "false");
    expect(ambientLevel()).toBeCloseTo(0.08);
  });

  it("mutes and sets each channel separately and keeps the choices after a restart", async () => {
    const user = userEvent.setup();
    const first = render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    let sound = await openSettings(user);
    await user.click(within(sound).getByRole("button", { name: "Mute ambient rain" }));
    expect(ambientLevel()).toBe(0);
    fireEvent.change(within(sound).getByRole("slider", { name: "Interaction sounds volume" }), { target: { value: "30" } });
    expect(within(sound).getByText("30%")).toBeInTheDocument();
    first.unmount();

    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    sound = await openSettings(user);
    expect(within(sound).getByRole("button", { name: "Mute ambient rain" })).toHaveAttribute("aria-pressed", "true");
    expect(within(sound).getByRole("button", { name: "Mute interaction sounds" })).toHaveAttribute("aria-pressed", "false");
    expect(within(sound).getByRole("slider", { name: "Interaction sounds volume" })).toHaveValue("30");
    expect(within(sound).getByRole("slider", { name: "Ambient rain volume" })).toHaveValue("8");
    expect(ambientLevel()).toBe(0);
  });
});

describe("Interaction sound", () => {
  it("marks creating and completing a Task with a soft cue, and stays silent when muted", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    const context = FakeAudioContext.latest();

    await user.type(screen.getByRole("textbox", { name: "Task title" }), "Synthetic follow-up");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await screen.findByRole("article", { name: "Synthetic follow-up" });
    const afterCreate = context.oscillators.length;
    expect(afterCreate).toBeGreaterThan(0);

    await complete(user, "Synthetic plan");
    await waitFor(() => expect(context.oscillators.length).toBeGreaterThan(afterCreate));

    const sound = await openSettings(user);
    await user.click(within(sound).getByRole("button", { name: "Mute interaction sounds" }));
    const muted = context.oscillators.length;
    await user.click(screen.getByRole("button", { name: "All tasks" }));
    await complete(user, "Synthetic follow-up");
    expect(context.oscillators).toHaveLength(muted);
  });
});

describe("When audio cannot play", () => {
  it("keeps every control and Task action working without Web Audio, even with Reduce Motion", async () => {
    vi.stubGlobal("AudioContext", undefined);
    reducedMotion = true;
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    const sound = await openSettings(user);
    expect(within(sound).getByText(/Sound isn't available/)).toBeInTheDocument();
    await user.click(within(sound).getByRole("button", { name: "Mute ambient rain" }));
    expect(within(sound).getByRole("button", { name: "Mute ambient rain" })).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByRole("button", { name: "All tasks" }));
    await complete(user, "Synthetic plan");
    expect(await screen.findByRole("region", { name: "Completed tasks" })).toBeInTheDocument();
  });

  it("waits for the first click when autoplay is blocked, then fades the rain in", async () => {
    FakeAudioContext.initialState = "suspended";
    FakeAudioContext.resumePolicy = "block";
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    expect(ambientLevel()).toBe(0);
    FakeAudioContext.resumePolicy = "allow";
    const sound = await openSettings(user);
    await waitFor(() => expect(ambientLevel()).toBeCloseTo(0.08));
    expect(within(sound).queryByText(/first click/)).toBeNull();
  });
});

describe("Quitting", () => {
  it("fades the rain out before the app answers the quit request", async () => {
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    await act(() => tauriEvents.emit(QUIT_REQUESTED, 1));
    expect(source.stoppedAt).not.toBeNull();
    expect(call).toHaveBeenCalledWith("quit_ready", { ticket: 1, proceed: true });
  });
});
