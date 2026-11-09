import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import TaskRow from "./components/TaskRow";
import SettingsView from "./components/SettingsView";
import type { Task } from "./lib/task-api";
import type { UnsavedTaskEdits } from "./lib/use-task-drafts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/** Spec story 3: ⌘↵ saves whichever Task edit group is open. Synthetic fixtures only. */
const synthetic: Task = {
  id: "task-1", title: "Synthetic task", description: "Plain words", status: "in_progress",
  priority: "medium", difficulty: null, progress: 10, nextStep: "", revision: 3, createdAt: 100,
  updatedAt: 100, listId: null, dueDate: null, tags: [],
};

const onSave = vi.fn(async () => undefined);
const onChange = vi.fn(async () => undefined);
const onRecordProgress = vi.fn(async () => undefined);
const onUpdateDetails = vi.fn(async () => undefined);

/** TaskRow with its unsaved edits held above it, as App holds them. */
function Row({ initialEdits, saving = false }: { initialEdits?: UnsavedTaskEdits; saving?: boolean }) {
  const [edits, setEdits] = useState<UnsavedTaskEdits | undefined>(initialEdits);
  return (
    <TaskRow
      task={synthetic} expanded saving={saving} onToggle={() => undefined}
      onSave={onSave} onChange={onChange} onRecordProgress={onRecordProgress}
      lists={[]} today="2026-11-09" onMove={async () => undefined}
      onUpdateDetails={onUpdateDetails} onOpenNote={() => undefined}
      onRemove={async () => undefined} shown edits={edits} onEdits={setEdits}
    />
  );
}

const commandReturn = "{Meta>}{Enter}{/Meta}";

beforeEach(() => {
  call.mockReset();
  call.mockResolvedValue({ items: [], nextCursor: null });
  for (const save of [onSave, onChange, onRecordProgress, onUpdateDetails]) save.mockClear();
});
afterEach(cleanup);

function noSaves() {
  expect(onSave).not.toHaveBeenCalled();
  expect(onChange).not.toHaveBeenCalled();
  expect(onRecordProgress).not.toHaveBeenCalled();
  expect(onUpdateDetails).not.toHaveBeenCalled();
}

describe("⌘↵ saves the open edit group", () => {
  it("saves title and description edits from the description, without a newline", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const description = screen.getByRole("textbox", { name: "Edit description" });
    await user.type(description, " more");
    await user.type(description, commandReturn);
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith("Synthetic task", "Plain words more", 3);
    expect(description).toHaveValue("Plain words more");
  });

  it("saves title edits from the title field once, not twice", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const title = screen.getByRole("textbox", { name: "Edit title" });
    await user.type(title, " renamed");
    await user.type(title, commandReturn);
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith("Synthetic task renamed", "Plain words", 3);
  });

  it("names the revision the typing started from, as the Save button does", async () => {
    const user = userEvent.setup();
    render(<Row initialEdits={{ baseRevision: 2, fields: { description: "Older draft" } }} />);
    await user.type(screen.getByRole("textbox", { name: "Edit description" }), commandReturn);
    expect(onSave).toHaveBeenCalledWith("Synthetic task", "Older draft", 2);
  });

  it("saves a progress update from the Next step or Progress note field", async () => {
    const user = userEvent.setup();
    render(<Row />);
    await user.type(screen.getByRole("textbox", { name: "Next step" }), "Call");
    const note = screen.getByRole("textbox", { name: "Progress note" });
    await user.type(note, "Spoke to the printer");
    await user.type(note, commandReturn);
    expect(onRecordProgress).toHaveBeenCalledTimes(1);
    expect(onRecordProgress).toHaveBeenCalledWith(
      { nextStep: "Call", note: "Spoke to the printer" }, 3,
    );
    expect(note).toHaveValue("Spoke to the printer");
  });

  it("applies a percentage alone from the percentage field", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const percent = screen.getByRole("spinbutton", { name: "Progress percentage" });
    await user.clear(percent);
    await user.type(percent, "33");
    await user.type(percent, commandReturn);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith({ progress: 33 }, 3);
  });

  it("does not apply an out-of-range percentage, as the button is disabled for it", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const percent = screen.getByRole("spinbutton", { name: "Progress percentage" });
    await user.clear(percent);
    await user.type(percent, "150");
    await user.type(percent, commandReturn);
    noSaves();
  });

  it("saves tags and deadline from the Tags field", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const tags = screen.getByRole("textbox", { name: "Tags" });
    await user.type(tags, "errands, home");
    await user.type(tags, commandReturn);
    expect(onUpdateDetails).toHaveBeenCalledTimes(1);
    expect(onUpdateDetails).toHaveBeenCalledWith(["errands", "home"], null, 3);
  });

  it("does nothing in any group with no unsaved edits", async () => {
    const user = userEvent.setup();
    render(<Row />);
    for (const name of ["Edit title", "Edit description", "Next step", "Progress note", "Tags"]) {
      await user.type(screen.getByRole("textbox", { name }), commandReturn);
    }
    await user.type(screen.getByRole("spinbutton", { name: "Progress percentage" }), commandReturn);
    noSaves();
  });

  it("keeps plain Return as a newline in the description", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const description = screen.getByRole("textbox", { name: "Edit description" });
    await user.type(description, "{Enter}line");
    expect(description).toHaveValue("Plain words\nline");
    noSaves();
  });

  it.each([
    ["⌘↵", "Edit title"],
    ["Return", "Edit title"],
    ["⌘↵", "Edit description"],
    ["⌘↵", "Progress note"],
    ["⌘↵", "Tags"],
  ])("does not let %s in %s save while an input method is composing", async (press, name) => {
    const user = userEvent.setup();
    render(<Row />);
    const field = screen.getByRole("textbox", { name });
    await user.type(field, "x");
    fireEvent.keyDown(field, { key: "Enter", metaKey: press === "⌘↵", isComposing: true });
    noSaves();
    // The same press once composition ends does save: the guard, not the event, held it back.
    fireEvent.keyDown(field, { key: "Enter", metaKey: true });
    expect([onSave, onRecordProgress, onUpdateDetails].some((save) => save.mock.calls.length === 1)).toBe(true);
  });

  it("keeps plain Return as a newline in the progress note", async () => {
    const user = userEvent.setup();
    render(<Row />);
    const note = screen.getByRole("textbox", { name: "Progress note" });
    await user.type(note, "a{Enter}b");
    expect(note).toHaveValue("a\nb");
    noSaves();
  });
});

describe("the ⌘↵ hint on each Save button", () => {
  it.each([
    ["Save changes", "Edit description", "!"],
    ["Save tags and deadline", "Tags", "x"],
    ["Apply progress", null, ""],
  ])("shows ⌘↵ quietly on %s and names it Command Return", async (name, field, typed) => {
    const user = userEvent.setup();
    render(<Row />);
    if (field) await user.type(screen.getByRole("textbox", { name: field }), typed);
    const button = screen.getByRole("button", { name });
    expect(button).toHaveAttribute("aria-keyshortcuts", "Meta+Enter");
    expect(button).toHaveAccessibleDescription("Command Return");
    // The description's source is hidden, so reading line by line never meets it as stray text.
    const source = document.getElementById(button.getAttribute("aria-describedby")!)!;
    expect(source).toHaveAttribute("hidden");
    const hint = within(button).getByText("⌘↵");
    expect(hint.tagName).toBe("KBD");
    expect(hint).toHaveAttribute("aria-hidden", "true");
  });

  it("stays on the progress button when it reads Save progress update", async () => {
    const user = userEvent.setup();
    render(<Row />);
    await user.type(screen.getByRole("textbox", { name: "Next step" }), "Call");
    const button = screen.getByRole("button", { name: "Save progress update" });
    expect(button).toHaveAttribute("aria-keyshortcuts", "Meta+Enter");
    expect(button).toHaveAccessibleDescription("Command Return");
  });
});

it("hides the ⌘↵ hint while the button reads Saving...", () => {
  render(<Row saving initialEdits={{ baseRevision: 3, fields: { description: "Draft" } }} />);
  const button = screen.getByRole("button", { name: "Saving..." });
  expect(within(button).queryByText("⌘↵")).toBeNull();
});

it("lists ⌘↵ in the Settings keyboard shortcuts", () => {
  render(<SettingsView hidden={false} preference="system" onPreference={() => undefined} />);
  const shortcut = screen.getByText("Save the open Task edit").closest("div")!;
  expect(within(shortcut).getByText("⌘↵")).toBeInTheDocument();
});
