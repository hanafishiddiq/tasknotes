import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

let backend: NoteBackend;

const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists" || command === "list_tasks") return { items: [], nextCursor: null };
    if (command === "quit_ready") return null;
    return backend.handle(command, (args ?? {}) as Record<string, unknown>);
  });
});

afterEach(() => cleanup());

describe("Exclude from AI", () => {
  it("marks the open Note private and back, and the person keeps editing it without a conflict", async () => {
    const diary = backend.seed("Synthetic diary", "Synthetic diary");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const list = await screen.findByRole("list", { name: "Notes" });
    await user.click(await within(list).findByText("Synthetic diary", { exact: true }));
    await eventually(() => expect(screen.getByRole("textbox", { name: "Note text" })).toHaveTextContent("Synthetic diary"));

    const toggle = screen.getByRole("button", { name: "Exclude from AI" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    await user.click(toggle);
    await eventually(() => expect(screen.getByRole("button", { name: "Exclude from AI" }))
      .toHaveAttribute("aria-pressed", "true"));
    expect(backend.commands("set_note_exclude_from_ai")).toEqual([{ input: { id: diary.id, excludeFromAi: true } }]);

    // The flag is not an edit: the editor still saves over the revision it opened.
    await user.click(screen.getByRole("textbox", { name: "Note text" }));
    await user.keyboard("Private ");
    await eventually(() => expect(backend.commands("update_note")).toEqual([
      { input: { id: diary.id, markdown: "Private Synthetic diary", expectedRevision: 1 } },
    ]));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(backend.notes.get(diary.id)?.excludeFromAi).toBe(true);

    await user.click(screen.getByRole("button", { name: "Exclude from AI" }));
    await eventually(() => expect(screen.getByRole("button", { name: "Exclude from AI" }))
      .toHaveAttribute("aria-pressed", "false"));
    expect(backend.commands("set_note_exclude_from_ai")[1]).toEqual({ input: { id: diary.id, excludeFromAi: false } });
  });

  it("shows an already excluded Note as excluded when it is opened", async () => {
    backend.seed("Synthetic medical note", "Synthetic medical note", { excludeFromAi: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const list = await screen.findByRole("list", { name: "Notes" });
    await user.click(await within(list).findByText("Synthetic medical note", { exact: true }));
    await eventually(() => expect(screen.getByRole("button", { name: "Exclude from AI" }))
      .toHaveAttribute("aria-pressed", "true"));
  });
});
