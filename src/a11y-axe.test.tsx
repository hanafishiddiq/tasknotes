import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import CommandPalette from "./components/CommandPalette";
import { adoptIndexHtml, RULES_JSDOM_CANNOT_JUDGE, runAxe } from "./test/axe";
import {
  everyRowKind, fixtureList, fixtureRequest, fixtureTask, FIXTURE_NOW, RICH_NOTE, World,
} from "./test/a11y-world";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * Axe over the real components with mocked Tauri data, one scene per view and
 * state, in both appearances. Colour contrast and anything needing layout is
 * off here (see RULES_JSDOM_CANNOT_JUDGE). Synthetic data only.
 */
let world: World;
type User = ReturnType<typeof userEvent.setup>;
const eventually = { timeout: 5000 };

interface Scene {
  /** What the core holds before the window opens. */
  given?: () => void;
  /** Walks the window to the state under test. */
  show: (user: User) => Promise<unknown>;
}

const openTask = (user: User, title: string) => screen.findByRole("button", { name: `Open ${title}` }).then((row) => user.click(row));
const openNotes = async (user: User) => {
  await user.click(await screen.findByRole("button", { name: "All notes" }));
  await screen.findByRole("list", { name: "Notes" }, eventually);
};
const openSettings = async (user: User) => {
  await user.click(await screen.findByRole("button", { name: "Settings" }));
  await screen.findByRole("list", { name: "Backups" }, eventually);
  await screen.findByRole("heading", { name: /Quick Capture$/ }, eventually);
};
const openRemoved = async (user: User) => user.click(await screen.findByRole("button", { name: "Removed" }));
const searchTasks = async (user: User, text: string) => {
  await screen.findByRole("region", { name: "Your tasks" });
  await user.type(screen.getByRole("searchbox", { name: "Search tasks and notes" }), text);
};
const openPalette = async (user: User) => {
  await screen.findByRole("region", { name: "Your tasks" });
  await user.keyboard("{Meta>}k{/Meta}");
  await screen.findByRole("dialog", { name: "Command palette" });
};
const folderTree = () => {
  const parent = world.notes.folder("Synthetic folder");
  world.notes.folder("Synthetic subfolder", parent);
};
const removedItems = () => [
  { kind: "task" as const, id: "t", title: "Synthetic removed", revision: 1, removedAt: FIXTURE_NOW, inRemovedContainer: false },
  { kind: "note" as const, id: "n", title: "Synthetic blocked", revision: 1, removedAt: FIXTURE_NOW, inRemovedContainer: true },
];

const SCENES: Record<string, Scene> = {
  // Task view
  "Task view: rows collapsed": {
    given: () => { world.tasks = everyRowKind(); world.lists = [fixtureList("l1", "Synthetic list")]; },
    show: () => screen.findByRole("article", { name: "Synthetic draft" }),
  },
  "Task view: pages": {
    given: () => { world.tasks = everyRowKind(); world.taskNextCursor = "2"; },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: "Next page" }));
      await screen.findByRole("button", { name: "Previous page" });
    },
  },
  "Task view: one row open": {
    given: () => {
      world.tasks = everyRowKind();
      world.history = [{
        sequence: 1, id: "h", taskId: "a", previousStatus: "ready", status: "in_progress", previousProgress: 0,
        progress: 40, note: "Synthetic history note", nextStep: "Synthetic next", createdAt: FIXTURE_NOW,
      }];
      world.notesOfTask = [{ linkId: "lk", noteId: "n", title: "Synthetic linked note", updatedAt: FIXTURE_NOW }];
    },
    show: async (user) => {
      await openTask(user, "Synthetic draft");
      await screen.findByText("Synthetic linked note");
      await screen.findByText("Synthetic history note");
    },
  },
  "Task view: open row with unsaved edits and the Note picker": {
    given: () => { world.tasks = everyRowKind(); world.notes.seed("Synthetic choice", "Synthetic choice"); },
    show: async (user) => {
      await openTask(user, "Synthetic draft");
      await user.type(screen.getByRole("textbox", { name: "Tags" }), "x");
      await user.click(screen.getByRole("button", { name: "Link a Note" }));
      await screen.findByRole("button", { name: "Synthetic choice" });
    },
  },
  "Task view: search results": {
    given: () => { world.tasks = everyRowKind(); world.notes.seed("Synthetic draft outline", "Synthetic outline"); },
    show: async (user) => {
      await searchTasks(user, "draft");
      await screen.findByRole("button", { name: /Synthetic outline/ }, eventually);
    },
  },
  "Task view: filters active": {
    given: () => { world.tasks = everyRowKind(); },
    show: async (user) => {
      await screen.findByRole("article", { name: "Synthetic draft" });
      await user.selectOptions(screen.getByRole("combobox", { name: "Filter by status" }), "in_progress");
      await user.selectOptions(screen.getByRole("combobox", { name: "Filter by priority" }), "high");
      await user.type(screen.getByRole("textbox", { name: "Filter by tag" }), "alpha");
      await screen.findByRole("button", { name: "Clear search and filters" });
      await screen.findByRole("article", { name: "Synthetic review" });
      await waitFor(() => expect(screen.queryByRole("article", { name: "Synthetic draft" })).toBeNull(), eventually);
    },
  },
  "Empty state: first run": { show: () => screen.findByText("Your list is clear") },
  "Empty state: no matches": {
    show: async (user) => {
      await searchTasks(user, "zzz");
      await screen.findByText("No matching tasks", {}, eventually);
      await screen.findByText("No matching notes", {}, eventually);
    },
  },
  "Empty state: search refused": {
    show: async (user) => {
      await screen.findByText("Your list is clear");
      world.refuse.set("list_tasks", "Synthetic refusal");
      world.refuse.set("list_notes", "Synthetic refusal");
      await user.type(screen.getByRole("searchbox", { name: "Search tasks and notes" }), "zzz");
      await screen.findByText("Search couldn't run", {}, eventually);
      await screen.findByText("Notes couldn't be searched.", {}, eventually);
    },
  },
  "Empty state: Tasks could not load": {
    given: () => world.refuse.set("list_tasks", "Synthetic refusal"),
    show: () => screen.findByText("Couldn't load your tasks"),
  },

  // Notes
  "Notes: folder tree and an open Note": {
    given: () => {
      folderTree();
      world.notes.seed("Plain", "Plain");
      world.notes.seed(RICH_NOTE, "Synthetic heading", { pinned: true });
      world.tasksOfNote = [{ linkId: "lk", taskId: "a", title: "Synthetic linked task", status: "ready" }];
    },
    show: async (user) => {
      await openNotes(user);
      await screen.findByRole("textbox", { name: "Note text" }, eventually);
      await screen.findByText("Synthetic linked task");
      await user.click(await screen.findByRole("button", { name: "Expand Synthetic folder" }));
      await screen.findByRole("button", { name: "Synthetic subfolder" });
    },
  },
  "Notes: a Note with no pin and the Task picker": {
    given: () => { world.notes.seed("Plain", "Plain"); },
    show: async (user) => {
      await openNotes(user);
      await screen.findByRole("textbox", { name: "Note text" }, eventually);
      await user.click(screen.getByRole("button", { name: "Link a Task" }));
      await screen.findByRole("searchbox", { name: "Find a Task to link" });
    },
  },
  "Notes: a checklist Note": {
    given: () => { world.notes.seed("- [ ] Open item\n- [x] Done item", "Checklist"); },
    show: async (user) => {
      await openNotes(user);
      await screen.findByText("Open item", {}, eventually);
    },
  },
  "Notes: no Notes yet": {
    show: async (user) => {
      await openNotes(user);
      await screen.findByRole("textbox", { name: "Note text" }, eventually);
    },
  },
  "Notes: search with snippets": {
    given: () => { world.notes.seed("Synthetic needle words", "Synthetic match"); },
    show: async (user) => {
      await openNotes(user);
      await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "needle");
      await screen.findByText("needle", { selector: "mark" }, eventually);
    },
  },
  "Notes: search without a match": {
    show: async (user) => {
      await openNotes(user);
      await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "zzz");
      await screen.findByText("No matching notes", {}, eventually);
    },
  },
  "Notes: folder forms for a new, renamed and moved folder": {
    given: () => { folderTree(); },
    show: async (user) => {
      await openNotes(user);
      await user.click(await screen.findByRole("button", { name: "New folder in Synthetic folder" }));
      await screen.findByRole("textbox", { name: "New folder name in Synthetic folder" });
      await user.click(screen.getByRole("button", { name: "Move Synthetic folder" }));
      await screen.findByRole("combobox", { name: "Move Synthetic folder to" });
      await user.click(screen.getByRole("button", { name: "Rename Synthetic folder" }));
      await screen.findByRole("textbox", { name: "Rename folder" });
    },
  },
  "Notes: a Note changed elsewhere": {
    given: () => { world.notes.seed("Plain", "Plain"); },
    show: async (user) => {
      await openNotes(user);
      const text = await screen.findByRole("textbox", { name: "Note text" }, eventually);
      world.notes.editElsewhere([...world.notes.notes.keys()][0], "Newer");
      // A real input event, as typing reaches Lexical: its update is what autosaves.
      await user.click(text);
      await user.keyboard("x");
      await screen.findByText(/changed somewhere else/, {}, eventually);
    },
  },
  "Notes: a save refused": {
    given: () => { world.notes.seed("Plain", "Plain"); },
    show: async (user) => {
      await openNotes(user);
      const text = await screen.findByRole("textbox", { name: "Note text" }, eventually);
      world.notes.refuse("update_note", "Synthetic refusal");
      await user.click(text);
      await user.keyboard("x");
      await screen.findByText(/Couldn't save this Note/, {}, eventually);
    },
  },

  // Removed
  "Removed: items": {
    given: () => { world.removed = removedItems(); },
    show: async (user) => {
      await openRemoved(user);
      await screen.findByText("Synthetic removed");
    },
  },
  "Removed: empty": {
    show: async (user) => {
      await openRemoved(user);
      await screen.findByText("Nothing in Removed");
    },
  },
  "Removed: search without a match": {
    given: () => { world.removed = removedItems(); },
    show: async (user) => {
      await openRemoved(user);
      await user.type(await screen.findByRole("searchbox", { name: "Search Removed" }), "zzz");
      await screen.findByText("No matching removed items", {}, eventually);
    },
  },
  "Removed: could not load": {
    given: () => world.refuse.set("list_removed", "Synthetic refusal"),
    show: async (user) => {
      await openRemoved(user);
      await screen.findByText(/Couldn't load Removed/);
    },
  },

  // Settings
  "Settings: all sections, shortcut on, restore asked, export done": {
    given: () => {
      world.shell = {
        ...world.shell, keepInMenuBar: true, quickCaptureEnabled: true, rememberedLabel: "⌃⌥Space",
        quickCapture: { state: "active", label: "⌃⌥Space" },
      };
    },
    show: async (user) => {
      await openSettings(user);
      await user.click(screen.getAllByRole("button", { name: /^Restore the backup/ })[0]);
      await screen.findByRole("button", { name: "Replace and restore" });
      await user.click(screen.getByRole("button", { name: "Export as JSON" }));
      await screen.findByText(/Saved to/);
    },
  },
  "Settings: recording a shortcut": {
    show: async (user) => {
      await openSettings(user);
      await user.click(screen.getByRole("checkbox", { name: "Open Quick Capture with a global shortcut" }));
      await screen.findByRole("button", { name: "Press the new shortcut" });
    },
  },
  "Settings: a shortcut refused, settings unreadable, export refused": {
    given: () => {
      world.refuse.set("export_data", "Synthetic refusal");
      world.shell = {
        ...world.shell, settingsUnreadable: true, quickCaptureEnabled: true, rememberedLabel: "⌃⌥Space",
        quickCapture: { state: "unavailable", label: "⌃⌥Space", reason: "Synthetic reason.", current: null },
      };
    },
    show: async (user) => {
      await openSettings(user);
      await screen.findByText("Synthetic reason.");
      await user.click(screen.getByRole("button", { name: "Export as JSON" }));
      await screen.findByRole("alert");
    },
  },
  "Settings: nothing the shell can offer": {
    given: () => {
      world.refuse.set("get_shell_settings", "Synthetic refusal");
      world.refuse.set("list_backups", "Synthetic refusal");
    },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: "Settings" }));
      await screen.findByText("These settings are not available on this device.");
      await screen.findByText("Backups are not available on this device.");
    },
  },
  "Settings: no backups yet": {
    given: () => { world.backups = []; },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: "Settings" }));
      await screen.findByText("No backups yet.");
    },
  },

  // Waiting for approval
  "Waiting for approval: requests": {
    given: () => {
      world.pending = [
        fixtureRequest("r1"),
        fixtureRequest("r2", {
          stale: true, target: "Synthetic folder", total: 5,
          request: { ...fixtureRequest("r2").request, action: { action: "remove_folder" }, count: null },
        }),
      ];
    },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: /^Waiting for approval/ }));
      await screen.findByRole("listitem", { name: "Move 2 Tasks to Removed" });
    },
  },
  "Waiting for approval: nothing left": {
    given: () => { world.pending = [fixtureRequest("r1")]; },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: /^Waiting for approval/ }));
      await user.click(await screen.findByRole("button", { name: /^Decline/ }));
      await screen.findByText("Nothing waiting for approval");
    },
  },
  "Waiting for approval: could not load": {
    given: () => { world.pending = [fixtureRequest("r1")]; world.refuse.set("list_pending_bulk_requests", "Synthetic refusal"); },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: /^Waiting for approval/ }));
      await screen.findByRole("alert");
    },
  },

  // Command palette
  "Command palette: open": { show: openPalette },
  "Command palette: a filtered list": {
    given: () => { world.lists = [fixtureList("l1", "Synthetic list")]; },
    show: async (user) => {
      await openPalette(user);
      await user.type(screen.getByRole("combobox", { name: "Command" }), "synth");
      await user.keyboard("{ArrowDown}");
    },
  },
  "Command palette: free text": {
    show: async (user) => {
      await openPalette(user);
      await user.type(screen.getByRole("combobox", { name: "Command" }), "zzz");
      await screen.findByRole("option", { name: "Search for “zzz”" });
    },
  },

  // Quick Capture: the ordinary New task field, reached by ⌘N or the global shortcut.
  "Quick Capture: field open": {
    show: async (user) => {
      await screen.findByText("Your list is clear");
      await user.keyboard("{Meta>}n{/Meta}");
      await user.keyboard("Synthetic capture");
      await screen.findByRole("button", { name: "Add task" });
    },
  },
  "Quick Capture: a shortcut could not be registered": {
    given: () => { world.launchNotice = "Synthetic notice."; },
    show: () => screen.findByRole("status", { name: "Quick Capture shortcut" }),
  },
  "Quick Capture: a new Task hidden by the filters": {
    given: () => { world.tasks = everyRowKind(); },
    show: async (user) => {
      await searchTasks(user, "draft");
      await waitFor(() => expect(screen.queryByRole("article", { name: "Synthetic errand" })).toBeNull(), eventually);
      await screen.findByRole("article", { name: "Synthetic draft" });
      await user.type(screen.getByRole("textbox", { name: "Task title" }), "Other{Enter}");
      await screen.findByRole("status", { name: "Task hidden by filters" }, eventually);
    },
  },

  // Undo bar and banners
  "Undo bar": {
    given: () => { world.tasks = [fixtureTask("a")]; },
    show: async (user) => {
      await openTask(user, "Synthetic task a");
      await user.click(screen.getByRole("button", { name: "Move to Removed" }));
      await screen.findByRole("status", { name: "Moved to Removed" });
    },
  },
  "Undo bar: Undo refused": {
    given: () => { world.tasks = [fixtureTask("a")]; },
    show: async (user) => {
      await openTask(user, "Synthetic task a");
      await user.click(screen.getByRole("button", { name: "Move to Removed" }));
      const notice = await screen.findByRole("status", { name: "Moved to Removed" });
      world.refuse.set("set_task_removed", "Synthetic refusal");
      await user.click(notice.querySelector("button")!);
      await screen.findByRole("button", { name: "Dismiss" }, eventually);
    },
  },
  "Banner: an error and a list that could not refresh": {
    given: () => { world.tasks = [fixtureTask("a")]; },
    show: async (user) => {
      await screen.findByRole("article", { name: "Synthetic task a" });
      await user.type(screen.getByRole("textbox", { name: "Task title" }), "Other");
      world.refuse.set("list_tasks", "Synthetic refusal");
      await user.keyboard("{Enter}");
      await screen.findByRole("button", { name: "Refresh list" }, eventually);
    },
  },
  "Banner: a Task changed elsewhere": {
    given: () => { world.tasks = [fixtureTask("a")]; },
    show: async (user) => {
      await openTask(user, "Synthetic task a");
      await user.type(screen.getByRole("textbox", { name: "Next step" }), "Synthetic");
      world.tasks[0].revision++;
      await user.selectOptions(screen.getByRole("combobox", { name: "Sort tasks" }), "Newest first");
      await screen.findByRole("alert", { name: "Changed elsewhere" }, eventually);
    },
  },
  "Banner: a Task opened from a Note": {
    given: () => {
      world.tasks = [fixtureTask("a")];
      world.notes.seed("Plain", "Plain");
      world.tasksOfNote = [{ linkId: "lk", taskId: "a", title: "Synthetic linked task", status: "ready" }];
    },
    show: async (user) => {
      await openNotes(user);
      await user.click(await screen.findByRole("button", { name: /^Synthetic linked task/ }, eventually));
      await screen.findByRole("status", { name: "Linked Task" }, eventually);
    },
  },
  "Banner: sidebar Lists could not load": {
    given: () => world.refuse.set("list_lists", "Synthetic refusal"),
    show: () => screen.findByText(/Could not load Lists/),
  },
  "Sidebar: a List being renamed": {
    given: () => { world.lists = [fixtureList("l1", "Synthetic list")]; },
    show: async (user) => {
      await user.click(await screen.findByRole("button", { name: "Rename Synthetic list" }));
      await screen.findByRole("textbox", { name: "Rename list" });
    },
  },
  "Banner: a sidebar folder refusal": {
    given: () => { folderTree(); },
    show: async (user) => {
      await openNotes(user);
      world.notes.refuse("set_note_folder_removed", "Synthetic refusal");
      await user.click(await screen.findByRole("button", { name: "Remove Synthetic folder" }));
      await screen.findByRole("button", { name: "Dismiss" }, eventually);
    },
  },
};

/**
 * Findings accepted for now, by rule, each with its reason. A ratchet: the
 * findings must equal this exactly, so a new one fails and so does one that
 * has gone away, which is the cue to delete its entry.
 */
const KNOWN: Record<string, string[]> = {
  // Lexical draws checklist items as <li role="checkbox"> directly under <ul>, which
  // the list rules refuse. It is the library's markup: reported to the owner.
  "Notes: a checklist Note": ["aria-allowed-role", "list"],
};

beforeAll(async () => {
  adoptIndexHtml();
  await import("./components/NotesWorkspace");
});

beforeEach(() => {
  window.localStorage.clear();
  world = new World();
  call.mockReset();
  call.mockImplementation(async (command, args) => world.handle(command, (args ?? {}) as Record<string, any>));
});

afterEach(() => cleanup());

describe.each(["dark", "light"] as const)("axe in the %s appearance", (theme) => {
  it.each(Object.keys(SCENES))("%s", async (name) => {
    window.localStorage.setItem("tasknotes.theme", theme);
    SCENES[name].given?.();
    render(<App />);
    await SCENES[name].show(userEvent.setup());
    expect(document.documentElement.dataset.theme).toBe(theme);

    const found = await runAxe();
    expect(found.crashed, "axe rules that threw, so tested nothing").toEqual([]);
    const rules = [...found.violations, ...found.incomplete].sort();
    expect({ rules, details: found.details }).toEqual({ rules: KNOWN[name] ?? [], details: expect.anything() });
  });
});

describe("the axe check itself", () => {
  it("finds a real defect, so a clean result means something", async () => {
    render(<main><h1>Synthetic</h1><button type="button" /></main>);
    const found = await runAxe();
    expect(found.violations).toContain("button-name");
    expect(found.crashed).toEqual([]);
  });

  it("accepts the palette's no-match state, which the app itself never reaches", async () => {
    render(<CommandPalette commands={[]} textCommand={() => null} onClose={() => undefined} />);
    expect(screen.getByRole("status")).toHaveTextContent("No matching commands");
    const found = await runAxe();
    expect(found.crashed).toEqual([]);
    expect(found.violations).toEqual([]);
    // Axe leaves an empty listbox for a person to judge. Here the status line beside it says why it is empty.
    expect(found.incomplete).toEqual(["aria-required-children"]);
  });

  it("names only rules axe has when it switches rules off", () => {
    const known = axe.getRules().map((rule) => rule.ruleId);
    for (const rule of Object.keys(RULES_JSDOM_CANNOT_JUDGE)) expect(known).toContain(rule);
  });
});
