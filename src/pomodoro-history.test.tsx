import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import {
  POMODORO_CHANGED, type HistorySession, type PomodoroDay, type PomodoroHistoryPage, type PomodoroPhase,
  type PomodoroView,
} from "./lib/pomodoro-api";
import { localCalendarDay } from "./lib/deadlines";
import type { Task } from "./lib/task-api";
import { tauriEvents } from "./test/tauri-events";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * The core groups and counts the sessions (Rust tests cover the rules:
 * Overtime, Stopped, time zones, midnight, paging); the window shows what it
 * gets. This fake answers `pomodoro_history` with pages of made-up days.
 */
const base: Task = {
  id: "task-1", title: "Synthetic plan", description: "", status: "in_progress", priority: "medium", difficulty: null,
  wontDo: false, progress: 40, nextStep: "", listId: null, dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100,
};

const today = localCalendarDay();
/** 09:00 and so on, on this computer's clock, today. */
function todayAt(hour: number, minute = 0) {
  const [year, month, day] = today.split("-").map(Number);
  return new Date(year, month - 1, day, hour, minute).getTime();
}

/** Seconds east of UTC of this computer's clock at `time`, as the shell stores it. */
const offsetAt = (time: number) => -new Date(time).getTimezoneOffset() * 60;

/** A session started on this computer's clock, unless `utcOffsetSeconds` says otherwise. */
function session(overrides: Partial<HistorySession>): HistorySession {
  const startedAt = overrides.startedAt ?? todayAt(9);
  return {
    id: `session-${Math.random()}`, startedAt, endedAt: startedAt + 1_500_000, utcOffsetSeconds: offsetAt(startedAt),
    outcome: "finished", plannedMinutes: 25, overtimeMinutes: 0, stoppedAfterMinutes: null,
    taskId: null, taskTitle: null, taskRemoved: false, ...overrides,
  };
}

/** Oldest first within a day, as the core sends them. */
const TODAY: PomodoroDay = {
  date: today, finishedSessions: 2, focusMinutes: 57, stoppedSessions: 1,
  sessions: [
    session({ startedAt: todayAt(9), overtimeMinutes: 7, taskId: "task-1", taskTitle: "Synthetic plan" }),
    session({ startedAt: todayAt(10) }),
    session({
      startedAt: todayAt(11), outcome: "stopped", stoppedAfterMinutes: 12, taskId: "task-gone", taskRemoved: true,
    }),
  ],
};
/** 14:05 in Jakarta (07:05 UTC), wherever this computer is; then two Stopped ones. */
const MARCH_4: PomodoroDay = {
  date: "2025-03-04", finishedSessions: 1, focusMinutes: 50, stoppedSessions: 2,
  sessions: [
    session({ startedAt: Date.UTC(2025, 2, 4, 7, 5), utcOffsetSeconds: 7 * 3600, plannedMinutes: 50 }),
    session({ startedAt: Date.UTC(2025, 2, 4, 8, 0), utcOffsetSeconds: 7 * 3600, outcome: "stopped", stoppedAfterMinutes: 0 }),
    // Stored before the time until Stop was kept.
    session({ startedAt: Date.UTC(2025, 2, 4, 9, 0), utcOffsetSeconds: 7 * 3600, outcome: "stopped" }),
  ],
};
const FEB_2: PomodoroDay = {
  date: "2025-02-02", finishedSessions: 1, focusMinutes: 25, stoppedSessions: 0,
  sessions: [session({ startedAt: new Date(2025, 1, 2, 8, 0).getTime() })],
};
const JAN_5: PomodoroDay = {
  date: "2025-01-05", finishedSessions: 1, focusMinutes: 25, stoppedSessions: 0,
  sessions: [session({ startedAt: new Date(2025, 0, 5, 8, 0).getTime() })],
};

/** The text of each row, with the narrow space Intl may put before AM. */
const rowTexts = (rows: HTMLElement[]) => rows.map((row) => row.textContent?.replace(/\s/g, " "));

/** What the shell sends on `tasknotes://pomodoro`. */
function shellView(phase: PomodoroPhase): PomodoroView {
  const minutes = { min: 1, max: 60 };
  return {
    ...phase, title: null, lastTaskId: null, silent: false, saveError: null,
    durations: { focusMinutes: 25, shortBreakMinutes: 5, longBreakMinutes: 15, sessionsBeforeLongBreak: 4 },
    limits: { focusMinutes: minutes, shortBreakMinutes: minutes, longBreakMinutes: minutes, sessionsBeforeLongBreak: minutes },
  };
}

const shellSays = (phase: PomodoroPhase, sessionStored: boolean) => act(() =>
  tauriEvents.emit(POMODORO_CHANGED, { view: shellView(phase), taskChanged: null, sessionStored }));

let pages: Record<string, PomodoroHistoryPage>;
let outsideLatest: number;
/** A history read for this cursor waits until the test lets it answer. */
let held: { cursor: string; release: () => void } | null;

beforeEach(() => {
  pages = { first: { items: [TODAY, MARCH_4], nextCursor: "p1|2025-03-04" }, "p1|2025-03-04": { items: [FEB_2], nextCursor: null } };
  outsideLatest = 1;
  held = null;
  window.localStorage.clear();
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "pomodoro_history": {
        const key = body.cursor ?? "first";
        if (held?.cursor === key) {
          await new Promise<void>((resolve) => { held!.release = resolve; });
          held = null;
        }
        const page = pages[key];
        if (!page) throw new Error("History cursor is invalid");
        return structuredClone(page);
      }
      case "outside_changes": return { latest: outsideLatest, changed: body.seen != null && outsideLatest > body.seen, createdTasks: [] };
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": {
        const items = [{ ...base }].filter((task) => !body.taskId || task.id === body.taskId);
        return { items, nextCursor: null };
      }
      case "list_removed": return { items: [], nextCursor: null };
      case "list_progress_entries": return { items: [], nextCursor: null };
      case "list_task_notes": return { items: [], nextCursor: null };
      case "view_sorts": return [];
      case "quit_ready": return null;
      default: throw new Error("Unexpected command: " + command);
    }
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const historyReads = () => call.mock.calls.filter(([command]) => command === "pomodoro_history");
const lastArgs = (command: string) => {
  const calls = call.mock.calls.filter(([name]) => name === command);
  return calls[calls.length - 1]?.[1];
};

async function openHistory() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("article", { name: "Synthetic plan" });
  expect(historyReads()).toHaveLength(0);
  await user.click(screen.getByRole("button", { name: "Pomodoro" }));
  const page = await screen.findByRole("region", { name: "Pomodoro history" });
  await within(page).findByRole("heading", { name: /^Today/ });
  return { user, page };
}

describe("Pomodoro history page (Ticket 14)", () => {
  it("is a page of the sidebar named like its heading, like Settings", async () => {
    const { page } = await openHistory();
    expect(within(page).getByRole("heading", { level: 1 })).toHaveTextContent("Pomodoro");
    const row = screen.getByRole("button", { name: "Pomodoro" });
    expect(row).toHaveTextContent(/^Pomodoro$/);
    expect(row).toHaveAttribute("aria-current", "page");
    expect(historyReads()[0][1]).toMatchObject({ limit: 14, cursor: null });
  });

  it("is reachable from the ⌘K palette", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    await user.keyboard("{Meta>}k{/Meta}");
    const palette = screen.getByRole("dialog", { name: "Command palette" });
    await user.type(within(palette).getByRole("combobox", { name: "Command" }), "pomodoro");
    expect(within(palette).getByRole("option", { name: "Pomodoro" })).toBeInTheDocument();
    await user.keyboard("{Enter}");
    const page = await screen.findByRole("region", { name: "Pomodoro history" });
    expect(await within(page).findByRole("heading", { name: /^Today/ })).toBeVisible();
  });

  it("shows each day's count, focus minutes and sessions as the core counted them", async () => {
    const { page } = await openHistory();
    const day = within(page).getByRole("region", { name: "Today" });
    expect(within(day).getByRole("heading")).toHaveTextContent("Today");
    expect(within(day).getByText("2 sessions · 57 min focus · 1 stopped")).toBeInTheDocument();

    const rows = within(day).getAllByRole("listitem");
    // Oldest first: the day reads as a log.
    expect(rowTexts(rows)).toEqual([
      "9:00 AMSynthetic plan25 + 7 minFinished",
      "10:00 AMNo Task25 minFinished",
      "11:00 AMTask removed25 min plannedStopped after 12 min",
    ]);
    expect(within(rows[0]).getByRole("button", { name: "Synthetic plan" })).toBeInTheDocument();
    expect(within(rows[2]).queryByRole("button")).not.toBeInTheDocument();

    const older = within(page).getByRole("region", { name: "Tuesday, Mar 4, 2025" });
    expect(within(older).getByText("1 session · 50 min focus · 2 stopped")).toBeInTheDocument();
    // On the clock each session started on (Jakarta), whatever this computer's time zone is.
    expect(rowTexts(within(older).getAllByRole("listitem"))).toEqual([
      "2:05 PMNo Task50 minFinished",
      "3:00 PMNo Task25 min plannedStopped after <1 min",
      "4:00 PMNo Task25 min plannedStopped",
    ]);
  });

  it("says how days are counted, travel and midnight included", async () => {
    const { page } = await openHistory();
    expect(page).toHaveTextContent("Stopped sessions are listed but not counted");
    expect(page).toHaveTextContent("A session keeps the day and time of the clock it started on, even after you travel");
    expect(page).toHaveTextContent("a session that runs past midnight stays on the day it started");
  });

  it("loads older days only when asked", async () => {
    const { user, page } = await openHistory();
    expect(within(page).queryByRole("region", { name: "Sunday, Feb 2, 2025" })).not.toBeInTheDocument();
    await user.click(within(page).getByRole("button", { name: "Load more days" }));
    expect(await within(page).findByRole("region", { name: "Sunday, Feb 2, 2025" })).toBeInTheDocument();
    expect(lastArgs("pomodoro_history")).toMatchObject({ cursor: "p1|2025-03-04" });
    expect(within(page).queryByRole("button", { name: "Load more days" })).not.toBeInTheDocument();
  });

  it("opens a session's Task in the Task view", async () => {
    const { user, page } = await openHistory();
    await user.click(within(page).getByRole("button", { name: "Synthetic plan" }));
    expect(await screen.findByRole("status", { name: "Pomodoro Task" })).toHaveTextContent(
      "Showing the Task of a Pomodoro Session",
    );
    expect(screen.getByRole("region", { name: "Pomodoro history", hidden: true }).closest("main")).not.toBeVisible();
    expect(lastArgs("list_tasks")).toMatchObject({ taskId: "task-1" });
  });

  it("shows an empty history as empty, with nothing made up", async () => {
    pages = { first: { items: [], nextCursor: null } };
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    await user.click(screen.getByRole("button", { name: "Pomodoro" }));
    const page = await screen.findByRole("region", { name: "Pomodoro history" });
    expect(await within(page).findByRole("heading", { name: "No Pomodoro sessions yet" })).toBeInTheDocument();
    expect(within(page).queryByText(/min focus/)).not.toBeInTheDocument();
    expect(within(page).queryByRole("button", { name: "Load more days" })).not.toBeInTheDocument();
  });

  it("reads the history again when the shell stores a session, not on every tick", async () => {
    const { page } = await openHistory();
    // The window's first outside-changes check reads everything once more; on a
    // slow machine it can land seconds later, so let it finish before counting.
    await waitFor(async () => {
      await act(async () => { window.dispatchEvent(new Event("focus")); });
      expect(lastArgs("outside_changes")).toMatchObject({ seen: 1 });
    });
    const before = historyReads().length;
    await shellSays({ phase: "focus", taskId: null, remainingSeconds: 100 }, false);
    await shellSays({ phase: "focus", taskId: null, remainingSeconds: 99 }, false);
    expect(historyReads()).toHaveLength(before);

    pages.first = { items: [{ ...TODAY, finishedSessions: 3, focusMinutes: 82 }, MARCH_4], nextCursor: "p1|2025-03-04" };
    await shellSays({ phase: "break", long: false, remainingSeconds: 300 }, true);
    expect(await within(page).findByText("3 sessions · 82 min focus · 1 stopped")).toBeInTheDocument();
  });

  it("keeps the older days it showed when it reads again", async () => {
    const { user, page } = await openHistory();
    await user.click(within(page).getByRole("button", { name: "Load more days" }));
    await within(page).findByRole("region", { name: "Sunday, Feb 2, 2025" });
    await shellSays({ phase: "idle" }, true);
    await waitFor(() => expect(lastArgs("pomodoro_history")).toMatchObject({ cursor: "p1|2025-03-04" }));
    expect(within(page).getByRole("region", { name: "Sunday, Feb 2, 2025" })).toBeInTheDocument();
  });

  it("never skips a day when a session is stored while older days are loading", async () => {
    const { user, page } = await openHistory();
    held = { cursor: "p1|2025-03-04", release: () => {} };
    await user.click(within(page).getByRole("button", { name: "Load more days" }));
    // Meanwhile a session lands on a day between: the first page now ends a day later.
    const MARCH_10: PomodoroDay = { ...MARCH_4, date: "2025-03-10" };
    pages.first = { items: [TODAY, MARCH_10], nextCursor: "p1|2025-03-10" };
    pages["p1|2025-03-10"] = { items: [MARCH_4], nextCursor: "p1|2025-03-04" };
    await shellSays({ phase: "idle" }, true);
    await within(page).findByRole("region", { name: "Monday, Mar 10, 2025" });
    await act(async () => held?.release());

    // The older page answered for the list as it was; what shows is the core's order.
    await waitFor(() => expect(within(page).getByRole("button", { name: "Load more days" })).toBeEnabled());
    expect(within(page).queryByRole("region", { name: "Sunday, Feb 2, 2025" })).not.toBeInTheDocument();
    await user.click(within(page).getByRole("button", { name: "Load more days" }));
    expect(await within(page).findByRole("region", { name: "Tuesday, Mar 4, 2025" })).toBeInTheDocument();
  });

  it("never shows a day twice when a read from the top overtakes Load more", async () => {
    pages["p1|2025-03-04"] = { items: [FEB_2], nextCursor: "p1|2025-02-02" };
    pages["p1|2025-02-02"] = { items: [JAN_5], nextCursor: null };
    const { user, page } = await openHistory();
    await user.click(within(page).getByRole("button", { name: "Load more days" }));
    await within(page).findByRole("region", { name: "Sunday, Feb 2, 2025" });
    held = { cursor: "p1|2025-02-02", release: () => {} };
    await user.click(within(page).getByRole("button", { name: "Load more days" }));
    // Meanwhile a session is stored and the core's pages end elsewhere: the
    // read from the top already brings Jan 5 that the held page also holds.
    pages["p1|2025-03-04"] = { items: [FEB_2, JAN_5], nextCursor: null };
    await shellSays({ phase: "idle" }, true);
    await within(page).findByRole("region", { name: "Sunday, Jan 5, 2025" });
    await act(async () => held?.release());

    await waitFor(() => expect(within(page).queryByRole("button", { name: /Load/ })).not.toBeInTheDocument());
    const days = within(page).getAllByRole("region").map((day) => day.getAttribute("aria-labelledby"))
      .map((id) => document.getElementById(id!)?.textContent);
    expect(days).toEqual(["Today", "Tuesday, Mar 4, 2025", "Sunday, Feb 2, 2025", "Sunday, Jan 5, 2025"]);
  });

  it("reads the history again after a change made outside the window", async () => {
    const { page } = await openHistory();
    const before = historyReads().length;
    pages.first = {
      items: [{ ...TODAY, sessions: TODAY.sessions.map((s) => s.taskId === "task-1" ? { ...s, taskTitle: "Synthetic plan, renamed" } : s) }],
      nextCursor: null,
    };
    outsideLatest = 2;
    await act(async () => { window.dispatchEvent(new Event("focus")); });
    expect(await within(page).findByRole("button", { name: "Synthetic plan, renamed" })).toBeInTheDocument();
    expect(historyReads().length).toBeGreaterThan(before);
  });

  it("says so when the history cannot be read", async () => {
    pages = {};
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("article", { name: "Synthetic plan" });
    await user.click(screen.getByRole("button", { name: "Pomodoro" }));
    const page = await screen.findByRole("region", { name: "Pomodoro history" });
    expect(await within(page).findByRole("alert")).toHaveTextContent("Couldn't load the Pomodoro history");
    expect(within(page).queryByRole("heading", { name: "No Pomodoro sessions yet" })).not.toBeInTheDocument();
  });
});
