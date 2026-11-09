import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Profiler } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const STORAGE_KEY = "tasknotes.sidebarWidth";

let backend: NoteBackend;
let lists: unknown[];

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  lists = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    switch (command) {
      case "list_lists": return { items: lists, nextCursor: null };
      case "list_tasks": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      default: return backend.handle(command, (args ?? {}) as Record<string, any>);
    }
  });
});

afterEach(() => {
  cleanup();
  setWindowWidth(1024);
  vi.restoreAllMocks();
});

const edge = () => screen.getByRole("separator", { name: "Resize sidebar" });
const shellWidth = () =>
  (document.querySelector(".app-shell") as HTMLElement).style.getPropertyValue("--sidebar-width");

async function open() {
  render(<App />);
  await screen.findByRole("button", { name: "All tasks" });
}

function setWindowWidth(width: number) {
  Object.defineProperty(window, "innerWidth", { configurable: true, writable: true, value: width });
}

const storedWrites = (spy: { mock: { calls: unknown[][] } }) =>
  spy.mock.calls.filter(([key]) => key === STORAGE_KEY);

function dragEdge(from: number, to: number) {
  fireEvent.mouseDown(edge(), { button: 0, clientX: from });
  fireEvent.mouseMove(window, { clientX: to });
  fireEvent.mouseUp(window, { clientX: to });
}

describe("sidebar width", () => {
  it("opens 260 px wide with an operable vertical separator", async () => {
    await open();
    const separator = edge();
    expect(separator).toHaveAttribute("aria-orientation", "vertical");
    expect(separator).toHaveAttribute("aria-valuemin", "210");
    expect(separator).toHaveAttribute("aria-valuemax", "400");
    expect(separator).toHaveAttribute("aria-valuenow", "260");
    expect(separator).toHaveAttribute("tabindex", "0");
    expect(shellWidth()).toBe("260px");
  });

  it("follows a drag of its edge and remembers the width on this device", async () => {
    await open();
    dragEdge(260, 330);
    expect(edge()).toHaveAttribute("aria-valuenow", "330");
    expect(shellWidth()).toBe("330px");
    expect(document.body).not.toHaveClass("sidebar-resizing");

    cleanup();
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "330");
    expect(shellWidth()).toBe("330px");
  });

  it("stays between 210 and 400 px however far it is dragged", async () => {
    await open();
    dragEdge(260, 1200);
    expect(edge()).toHaveAttribute("aria-valuenow", "400");
    dragEdge(400, -50);
    expect(edge()).toHaveAttribute("aria-valuenow", "210");
    expect(shellWidth()).toBe("210px");
  });

  it("ignores a press with another mouse button", async () => {
    await open();
    fireEvent.mouseDown(edge(), { button: 2, clientX: 260 });
    fireEvent.mouseMove(window, { clientX: 360 });
    fireEvent.mouseUp(window);
    expect(edge()).toHaveAttribute("aria-valuenow", "260");
  });

  it("steps with the arrow keys and jumps to the limits with Home and End", async () => {
    const user = userEvent.setup();
    await open();
    edge().focus();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(edge()).toHaveAttribute("aria-valuenow", "280");
    await user.keyboard("{ArrowLeft}");
    expect(edge()).toHaveAttribute("aria-valuenow", "270");
    await user.keyboard("{End}");
    expect(edge()).toHaveAttribute("aria-valuenow", "400");
    await user.keyboard("{ArrowRight}");
    expect(edge()).toHaveAttribute("aria-valuenow", "400");
    await user.keyboard("{Home}");
    expect(edge()).toHaveAttribute("aria-valuenow", "210");
    expect(shellWidth()).toBe("210px");

    cleanup();
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "210");
  });

  it("goes back to 260 px on a double-click, or Return on the focused edge", async () => {
    const user = userEvent.setup();
    await open();
    dragEdge(260, 380);
    fireEvent.doubleClick(edge());
    expect(edge()).toHaveAttribute("aria-valuenow", "260");

    edge().focus();
    await user.keyboard("{End}{Enter}");
    expect(edge()).toHaveAttribute("aria-valuenow", "260");
  });

  it("sits inside the sidebar landmark", async () => {
    await open();
    expect(edge().closest("aside")).toHaveClass("sidebar");
  });

  it("opens at a valid width whatever storage holds", async () => {
    window.localStorage.setItem(STORAGE_KEY, "9000");
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "400");
    cleanup();

    window.localStorage.setItem(STORAGE_KEY, "wide");
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "260");
    cleanup();

    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("blocked"); });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("blocked"); });
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "260");
    dragEdge(260, 300);
    expect(edge()).toHaveAttribute("aria-valuenow", "300");
  });
});

describe("sidebar width while dragging", () => {
  it("moves the edge without re-rendering the app, and commits once on release", async () => {
    let commits = 0;
    render(<Profiler id="app" onRender={() => { commits += 1; }}><App /></Profiler>);
    await screen.findByRole("button", { name: "All tasks" });
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const before = commits;

    fireEvent.mouseDown(edge(), { button: 0, clientX: 260 });
    for (const x of [270, 290, 310, 330, 350]) fireEvent.mouseMove(window, { clientX: x });
    expect(shellWidth()).toBe("350px");
    expect(edge()).toHaveAttribute("aria-valuenow", "350");
    expect(commits).toBe(before);
    expect(storedWrites(setItem)).toEqual([]);

    fireEvent.mouseUp(window, { clientX: 350 });
    expect(commits).toBe(before + 1);
    expect(storedWrites(setItem)).toEqual([[STORAGE_KEY, "350"]]);
    expect(shellWidth()).toBe("350px");
  });

  it("writes the width once per keyboard step", async () => {
    const user = userEvent.setup();
    await open();
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    edge().focus();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(storedWrites(setItem)).toEqual([[STORAGE_KEY, "270"], [STORAGE_KEY, "280"]]);
  });

  it("ends the drag when the window loses focus, keeping the width it reached", async () => {
    await open();
    fireEvent.mouseDown(edge(), { button: 0, clientX: 260 });
    fireEvent.mouseMove(window, { clientX: 320 });
    fireEvent.blur(window);
    expect(document.body).not.toHaveClass("sidebar-resizing");
    expect(edge()).toHaveAttribute("aria-valuenow", "320");
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe("320");

    // The lost mouseup never arrives; later moves no longer resize.
    fireEvent.mouseMove(window, { clientX: 390 });
    expect(shellWidth()).toBe("320px");
  });

  it("puts the starting width back on Escape", async () => {
    await open();
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    fireEvent.mouseDown(edge(), { button: 0, clientX: 260 });
    fireEvent.mouseMove(window, { clientX: 360 });
    expect(shellWidth()).toBe("360px");
    fireEvent.keyDown(window, { key: "Escape" });
    expect(document.body).not.toHaveClass("sidebar-resizing");
    expect(shellWidth()).toBe("260px");
    expect(edge()).toHaveAttribute("aria-valuenow", "260");
    expect(storedWrites(setItem)).toEqual([]);

    fireEvent.mouseMove(window, { clientX: 390 });
    fireEvent.mouseUp(window, { clientX: 390 });
    expect(shellWidth()).toBe("260px");
  });

  it("releases the resize cursor if the app unmounts mid-drag", async () => {
    await open();
    fireEvent.mouseDown(edge(), { button: 0, clientX: 260 });
    expect(document.body).toHaveClass("sidebar-resizing");
    cleanup();
    expect(document.body).not.toHaveClass("sidebar-resizing");
  });
});

describe("sidebar width on a narrow window", () => {
  it("leaves the main view at least 480 px without changing the remembered width", async () => {
    window.localStorage.setItem(STORAGE_KEY, "400");
    setWindowWidth(800);
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "320");
    expect(edge()).toHaveAttribute("aria-valuemax", "320");
    expect(shellWidth()).toBe("320px");
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe("400");

    dragEdge(320, 600);
    expect(edge()).toHaveAttribute("aria-valuenow", "320");

    act(() => {
      setWindowWidth(1400);
      window.dispatchEvent(new Event("resize"));
    });
    expect(edge()).toHaveAttribute("aria-valuenow", "400");
    expect(shellWidth()).toBe("400px");
  });

  it("never goes below the 210 px minimum", async () => {
    setWindowWidth(600);
    await open();
    expect(edge()).toHaveAttribute("aria-valuenow", "210");
    expect(edge()).toHaveAttribute("aria-valuemax", "210");
  });
});

describe("sidebar row actions", () => {
  it("follow the name of each List and Note Folder in the container that covers its end", async () => {
    lists = [{ id: "list-1", name: "A List with a name far too long to fit", revision: 1, createdAt: 1, updatedAt: 1, removed: false }];
    backend.folder("Content");
    await open();
    const list = await screen.findByRole("button", { name: "A List with a name far too long to fit" });
    // The stylesheet gives `.folder-actions` the row's opaque fill, and the current
    // row's fill to the container right after a current `.folder`.
    expect(list.nextElementSibling).toHaveClass("folder-actions");
    expect(list.nextElementSibling).toContainElement(screen.getByRole("button", { name: "Rename A List with a name far too long to fit" }));

    fireEvent.click(screen.getByRole("button", { name: "All notes" }));
    const folder = await screen.findByRole("button", { name: "Content" });
    expect(folder.nextElementSibling).toHaveClass("folder-actions");
    for (const action of ["New folder in", "Rename", "Move", "Remove"]) {
      expect(folder.nextElementSibling).toContainElement(screen.getByRole("button", { name: `${action} Content` }));
    }
  });
});
