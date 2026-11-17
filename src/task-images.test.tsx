import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { Task } from "./lib/task-api";
import App from "./App";
import { withoutImages } from "./lib/task-image-node";

/**
 * Images in a Task's notes (owner, batch 4, 2026-11-18): the notes are rich
 * text like a Note, an image pasted or dropped in is kept by the core and
 * shown where it was put, and the collapsed row shows no image. The fake
 * core only stores what it is sent.
 */
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13]);
const ID = "11111111-2222-4333-8444-555555555555";

let task: Task;
let kept: { taskId: string; mediaType: string; bytes: number[] }[];

beforeEach(() => {
  window.localStorage.clear();
  kept = [];
  task = {
    id: "shelf", title: "Synthetic shelf", description: `Measure first\n\n![image](tasknotes-image:${ID})\n\nThen cut`,
    status: "ready", priority: "medium", difficulty: null, wontDo: false, progress: 0, nextStep: "", listId: null,
    dueDate: null, tags: [], revision: 1, createdAt: 100, updatedAt: 100,
  };
  URL.createObjectURL = vi.fn(() => "blob:synthetic-image");
  // jsdom has no DragEvent or ClipboardEvent; Lexical's own paste handling asks for it (WebKit has it).
  if (!("DragEvent" in globalThis)) vi.stubGlobal("DragEvent", class DragEvent extends Event {});
  if (!("ClipboardEvent" in globalThis)) vi.stubGlobal("ClipboardEvent", class ClipboardEvent extends Event {});
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args, options) => {
    switch (command) {
      case "list_lists": return { items: [], nextCursor: null };
      case "list_tasks": return { items: [{ ...task }], nextCursor: null };
      case "list_subtasks": return [];
      case "read_task_image": return PNG.buffer;
      case "add_task_image": {
        const headers = (options as { headers: Record<string, string> }).headers;
        kept.push({ taskId: headers["task-id"], mediaType: headers["media-type"], bytes: [...(args as Uint8Array)] });
        return { id: "22222222-3333-4444-8555-666666666666", taskId: headers["task-id"], mediaType: headers["media-type"], size: 12, createdAt: 1 };
      }
      case "update_task": {
        const input = (args as { input: { title: string; description: string } }).input;
        task = { ...task, ...input, revision: task.revision + 1 };
        return { ...task };
      }
      case "list_task_notes": case "list_progress_entries": case "list_removed": return { items: [], nextCursor: null };
      default: return null;
    }
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

/** A paste carrying `files`, as WebKit sends one. */
function paste(target: HTMLElement, files: File[]) {
  const event = new ClipboardEvent("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "clipboardData", {
    value: { files, items: [], types: ["Files"], getData: () => "" },
  });
  fireEvent(target, event);
}

const open = async (user: ReturnType<typeof userEvent.setup>) => {
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Open Synthetic shelf" }));
  return screen.findByRole("textbox", { name: "Edit description" });
};

describe("images in a Task's notes", () => {
  it("shows an image where the notes refer to it, read from the core", async () => {
    const user = userEvent.setup();
    const notes = await open(user);
    const image = await within(notes).findByRole("img", { name: "Image" });
    expect(image).toHaveAttribute("src", "blob:synthetic-image");
    expect(invokeMock).toHaveBeenCalledWith("read_task_image", { id: ID });
    expect(notes).toHaveTextContent("Measure first");
    expect(notes).toHaveTextContent("Then cut");
  });

  it("keeps a pasted image in the core as raw bytes and puts it in the notes", async () => {
    const user = userEvent.setup();
    const notes = await open(user);
    const file = new File([PNG], "photo.png", { type: "image/png" });
    notes.focus();
    paste(notes, [file]);
    await waitFor(() => expect(kept).toEqual([{ taskId: "shelf", mediaType: "image/png", bytes: [...PNG] }]));
    await waitFor(() => expect(within(notes).getAllByRole("img", { name: "Image" })).toHaveLength(2));
    // The notes now carry its reference, ready to save.
    await user.click(screen.getByRole("button", { name: /Save changes/ }));
    await waitFor(() => expect(task.description).toContain("![image](tasknotes-image:22222222-3333-4444-8555-666666666666)"));
    expect(task.description).toContain(`![image](tasknotes-image:${ID})`);
  });

  it("adds images chosen with Add image, for whoever does not paste", async () => {
    const user = userEvent.setup();
    await open(user);
    await user.upload(screen.getByLabelText("Choose images"), new File([PNG], "photo.png", { type: "image/png" }));
    await waitFor(() => expect(kept).toHaveLength(1));
    expect(screen.getByRole("button", { name: "Add image" })).toBeInTheDocument();
  });

  it("takes only image files and says so for anything else", async () => {
    const user = userEvent.setup();
    const notes = await open(user);
    const file = new File(["<svg/>"], "drawing.svg", { type: "image/svg+xml" });
    notes.focus();
    paste(notes, [file]);
    expect(kept).toEqual([]);
  });

  it("shows no image and no reference on the collapsed row", async () => {
    render(<App />);
    const row = await screen.findByRole("article", { name: "Synthetic shelf" });
    expect(within(row).queryByRole("img", { name: "Image" })).toBeNull();
    expect(row).not.toHaveTextContent("tasknotes-image");
    expect(withoutImages(task.description)).toBe("Measure first\n\nThen cut");
  });
});
