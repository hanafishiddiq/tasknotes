import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { TaskList } from "./lib/task-api";
import { STYLE_ICONS } from "./lib/style-icons";
import { STYLE_COLORS, STYLE_ICON_NAMES, searchIcons } from "./lib/style";
import { contrast, declared, parseColor, rules, selectorsOf, tokens, type Theme } from "./test/stylesheet";
import { NoteBackend } from "./test/note-backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

let lists: TaskList[];
let backend: NoteBackend;

const eventually = (assertion: () => void) => waitFor(assertion, { timeout: 3000 });

function list(name: string, fields: Partial<TaskList> = {}): TaskList {
  return { id: `list-${name}`, name, revision: 1, createdAt: 1, updatedAt: 1, removed: false, ...fields };
}

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  lists = [list("Alpha"), list("🏠 Home", { id: "list-home", icon: "house", color: "green" })];
  backend = new NoteBackend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_lists": return { items: lists.map((item) => ({ ...item })), nextCursor: null };
      case "list_tasks": return { items: [], nextCursor: null };
      case "quit_ready": return null;
      case "set_list_style": {
        const { id, expectedRevision, icon, color } = body.input;
        const current = lists.find((item) => item.id === id)!;
        if (current.revision !== expectedRevision) throw "Revision conflict: reload before saving";
        const next = { ...current, icon, color, revision: current.revision + 1 };
        lists = lists.map((item) => (item.id === id ? next : item));
        return { ...next };
      }
      default: return backend.handle(command, body);
    }
  });
});

afterEach(() => cleanup());

const listsNav = () => screen.getByRole("navigation", { name: "Lists" });
const rowIcon = (row: HTMLElement) => row.querySelector("svg")!;
const styleCalls = (command: string) =>
  invokeMock.mock.calls.filter(([name]) => name === command).map(([, args]) => (args as any).input);

describe("the allow-list the window shares with the core", () => {
  it("draws exactly the icons the core accepts", () => {
    expect(Object.keys(STYLE_ICONS)).toEqual([...STYLE_ICON_NAMES]);
    expect(STYLE_COLORS).toHaveLength(12);
  });

  it("searches icon names by every word typed", () => {
    expect(searchIcons("chef")).toEqual(["chef-hat"]);
    expect(searchIcons("folder lock")).toEqual(["folder-lock"]);
    expect(searchIcons("FOLDER-OPEN")).toEqual(["folder-open"]);
    expect(searchIcons("zzz")).toEqual([]);
  });
});

describe.each<Theme>(["dark", "light"])("the twelve colours in the %s appearance", (theme) => {
  const palette = tokens(theme);
  // WCAG AA for a graphical object (1.4.11): 3:1 on every fill a sidebar row can have.
  for (const key of STYLE_COLORS) {
    for (const surface of ["sidebar", "side-hover", "side-selected"]) {
      it(`${key} reads at 3:1 or more on ${surface}`, () => {
        const value = palette[`style-${key}`];
        expect(value, `--style-${key}`).toBeDefined();
        expect(contrast(parseColor(value), parseColor(palette[surface]))).toBeGreaterThanOrEqual(3);
      });
    }
  }

  it("gives each key its own value", () => {
    const values = STYLE_COLORS.map((key) => palette[`style-${key}`]);
    expect(new Set(values).size).toBe(12);
  });
});

describe("icons and colours on Lists", () => {
  it("shows a chosen icon in its colour and the default look otherwise, leaving emoji names alone", async () => {
    render(<App />);
    const home = await within(listsNav()).findByRole("button", { name: "🏠 Home" });
    expect(home).toHaveTextContent("🏠 Home");
    expect(rowIcon(home)).toHaveClass("lucide-house");
    expect(rowIcon(home)).toHaveAttribute("style", "color: var(--style-green);");
    const alpha = within(listsNav()).getByRole("button", { name: "Alpha" });
    expect(rowIcon(alpha)).toHaveClass("lucide-folder");
    expect(rowIcon(alpha)).not.toHaveAttribute("style");
    // The picker opens from the row's own actions, inside the opaque overlay.
    const action = within(listsNav()).getByRole("button", { name: "Icon and colour for Alpha" });
    expect(action.closest(".folder-actions")).not.toBeNull();
  });

  it("chooses a colour and a searched icon with the mouse, saves both in one write and shows them", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await within(listsNav()).findByRole("button", { name: "Icon and colour for Alpha" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for Alpha" });
    expect(within(picker).getByRole("button", { name: "Save" })).toBeDisabled();

    await user.click(within(picker).getByRole("radio", { name: "Blue" }));
    await user.type(within(picker).getByRole("searchbox", { name: "Search icons" }), "chef");
    const icons = within(picker).getByRole("radiogroup", { name: "Icon" });
    expect(within(icons).getAllByRole("radio").map((radio) => radio.getAttribute("aria-label"))).toEqual(["Chef hat"]);
    await user.click(within(icons).getByRole("radio", { name: "Chef hat" }));
    await user.click(within(picker).getByRole("button", { name: "Save" }));

    await eventually(() => expect(styleCalls("set_list_style")).toEqual([
      { id: "list-Alpha", expectedRevision: 1, icon: "chef-hat", color: "blue" },
    ]));
    await eventually(() => expect(screen.queryByRole("group", { name: /Icon and colour/ })).not.toBeInTheDocument());
    const alpha = within(listsNav()).getByRole("button", { name: "Alpha" });
    expect(rowIcon(alpha)).toHaveClass("lucide-chef-hat");
    expect(rowIcon(alpha)).toHaveAttribute("style", "color: var(--style-blue);");
    expect(alpha).toHaveFocus();
  });

  it("works from the keyboard: arrows move and choose, Down leaves the search, Escape cancels", async () => {
    const user = userEvent.setup();
    render(<App />);
    await within(listsNav()).findByRole("button", { name: "🏠 Home" });
    await user.click(within(listsNav()).getByRole("button", { name: "Icon and colour for 🏠 Home" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for 🏠 Home" });
    const colours = within(picker).getByRole("radiogroup", { name: "Colour" });

    // One tab stop per group: the chosen option.
    const green = within(colours).getByRole("radio", { name: "Green" });
    expect(green).toHaveAttribute("aria-checked", "true");
    expect(green).toHaveAttribute("tabindex", "0");
    expect(within(colours).getByRole("radio", { name: "Red" })).toHaveAttribute("tabindex", "-1");
    green.focus();
    await user.keyboard("{ArrowRight}");
    expect(within(colours).getByRole("radio", { name: "Mint" })).toHaveFocus();
    expect(within(colours).getByRole("radio", { name: "Mint" })).toHaveAttribute("aria-checked", "true");
    // Seven swatches a row: Down lands on the one below.
    await user.keyboard("{ArrowDown}");
    expect(within(colours).getByRole("radio", { name: "Brown" })).toHaveFocus();
    await user.keyboard("{Home}");
    expect(within(colours).getByRole("radio", { name: "No colour" })).toHaveFocus();

    await user.tab();
    const search = within(picker).getByRole("searchbox", { name: "Search icons" });
    expect(search).toHaveFocus();
    await user.keyboard("folder");
    await user.keyboard("{ArrowDown}");
    const icons = within(picker).getByRole("radiogroup", { name: "Icon" });
    expect(within(icons).getByRole("radio", { name: "Folder" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(within(icons).getByRole("radio", { name: "Folder open" })).toHaveAttribute("aria-checked", "true");

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("group", { name: /Icon and colour/ })).not.toBeInTheDocument();
    expect(styleCalls("set_list_style")).toEqual([]);
    expect(within(listsNav()).getByRole("button", { name: "🏠 Home" })).toHaveFocus();
  });

  it("clears back to the default look and keeps the picker open when the core refuses", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await within(listsNav()).findByRole("button", { name: "Icon and colour for 🏠 Home" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for 🏠 Home" });
    await user.click(within(picker).getByRole("radio", { name: "No colour" }));
    await user.click(within(picker).getByRole("radio", { name: "Default icon" }));
    // Someone else changed the List meanwhile.
    lists = lists.map((item) => (item.name === "🏠 Home" ? { ...item, revision: 5 } : item));
    await user.click(within(picker).getByRole("button", { name: "Save" }));
    await eventually(() => expect(screen.getByText(/Revision conflict/)).toBeInTheDocument());
    expect(screen.getByRole("group", { name: "Icon and colour for 🏠 Home" })).toBeInTheDocument();
    expect(styleCalls("set_list_style")).toEqual([
      { id: "list-home", expectedRevision: 1, icon: null, color: null },
    ]);
  });

  it("keeps focus on Save while the write is pending and after the core refuses it", async () => {
    let refuse!: (reason: string) => void;
    const base = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((command, args) => command === "set_list_style"
      ? new Promise((_, reject) => { refuse = reject; })
      : base(command, args));
    const user = userEvent.setup();
    render(<App />);
    await user.click(await within(listsNav()).findByRole("button", { name: "Icon and colour for Alpha" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for Alpha" });
    await user.click(within(picker).getByRole("radio", { name: "Blue" }));
    const save = within(picker).getByRole("button", { name: "Save" });
    await user.click(save);

    await eventually(() => expect(picker).toHaveAttribute("aria-busy", "true"));
    expect(save).toHaveFocus();
    expect(save).toHaveAttribute("aria-disabled", "true");
    // Nothing in the picker changes while saving, but nothing drops focus either.
    const red = within(picker).getByRole("radio", { name: "Red" });
    expect(red).not.toBeDisabled();
    expect(red).toHaveAttribute("aria-disabled", "true");
    await user.click(red);
    expect(red).toHaveAttribute("aria-checked", "false");
    await user.click(save);
    expect(styleCalls("set_list_style")).toHaveLength(1);

    refuse("Revision conflict: reload before saving");
    await eventually(() => expect(picker).not.toHaveAttribute("aria-busy"));
    expect(save).toHaveFocus();
    expect(save).not.toHaveAttribute("aria-disabled");
  });

  it("moves Down by the columns the picker actually shows when a narrow sidebar wraps it", async () => {
    // jsdom has no layout: lay the icon buttons out six to a row by hand.
    const width = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth")!;
    const top = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetTop")!;
    const inIcons = (element: HTMLElement) => element.parentElement?.getAttribute("aria-label") === "Icon";
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
      configurable: true, get(this: HTMLElement) { return inIcons(this) ? 22 : 0; },
    });
    Object.defineProperty(HTMLElement.prototype, "offsetTop", {
      configurable: true,
      get(this: HTMLElement) { return inIcons(this) ? Math.floor([...this.parentElement!.children].indexOf(this) / 6) * 24 : 0; },
    });
    try {
      const user = userEvent.setup();
      render(<App />);
      await user.click(await within(listsNav()).findByRole("button", { name: "Icon and colour for Alpha" }));
      const icons = within(screen.getByRole("group", { name: "Icon and colour for Alpha" })).getByRole("radiogroup", { name: "Icon" });
      const radios = within(icons).getAllByRole("radio");
      radios[0].focus();
      await user.keyboard("{ArrowDown}");
      expect(radios[6]).toHaveFocus();
      await user.keyboard("{ArrowUp}");
      expect(radios[0]).toHaveFocus();
    } finally {
      Object.defineProperty(HTMLElement.prototype, "offsetWidth", width);
      Object.defineProperty(HTMLElement.prototype, "offsetTop", top);
    }
  });

  it("does not start a drag from the picker and still drags a styled row", async () => {
    render(<App />);
    const home = await within(listsNav()).findByRole("button", { name: "🏠 Home" });
    expect(home.closest("[data-drag-row]")).toHaveAttribute("data-drag-row", "list-home");
    fireEvent.click(within(listsNav()).getByRole("button", { name: "Icon and colour for Alpha" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for Alpha" });
    expect(picker.closest("[data-drag-row]")).toBeNull();
  });
});

describe("the picker at a narrow sidebar and a deep folder", () => {
  const value = (selector: string, property: string) => {
    const rule = rules.find((item) => item.media === null && selectorsOf(item).includes(selector) && declared(item, property));
    return rule ? declared(rule, property) : null;
  };

  it("wraps its grids instead of shrinking a cell below a 22px target", () => {
    // jsdom has no layout, so the rule itself is checked: never more columns than the
    // keyboard expects, never a cell narrower than 22px; a narrow picker wraps.
    for (const grid of [".style-swatches", ".style-icons"]) {
      expect(value(grid, "grid-template-columns"), grid)
        .toBe("repeat(auto-fill, minmax(max(22px, calc((100% - (var(--columns) - 1) * 2px) / var(--columns))), 1fr))");
      expect(value(grid, "gap"), grid).toBe("2px");
    }
    // 7 swatches of 22px fit 210px - sidebar padding (20) - border (1) - picker border and padding (18).
    expect(7 * 22 + 6 * 2).toBeLessThanOrEqual(210 - 20 - 1 - 18);
  });

  it("opens at the sidebar's full width under a folder five levels deep", async () => {
    let parent = backend.folder("Level 1");
    for (let level = 2; level <= 5; level++) parent = backend.folder(`Level ${level}`, parent);
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const tree = () => screen.getByRole("navigation", { name: "Note folders" });
    for (let level = 1; level <= 4; level++) {
      await user.click(await within(tree()).findByRole("button", { name: `Expand Level ${level}` }));
    }
    await user.click(await within(tree()).findByRole("button", { name: "Icon and colour for Level 5" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for Level 5" });
    expect(picker.style.marginLeft).toBe("");
    expect(picker.style.paddingLeft).toBe("");
    expect(picker.closest(".folder-row")).toBeNull();
  });
});

describe("icons and colours on Note Folders", () => {
  it("styles a folder from its row actions and shows it in the tree", async () => {
    backend.folder("Recipes");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const tree = () => screen.getByRole("navigation", { name: "Note folders" });
    await user.click(await within(tree()).findByRole("button", { name: "Icon and colour for Recipes" }));
    const picker = screen.getByRole("group", { name: "Icon and colour for Recipes" });
    await user.click(within(picker).getByRole("radio", { name: "Pink" }));
    await user.type(within(picker).getByRole("searchbox", { name: "Search icons" }), "chef");
    await user.click(within(picker).getByRole("radio", { name: "Chef hat" }));
    await user.click(within(picker).getByRole("button", { name: "Save" }));

    await eventually(() => expect(backend.commands("set_note_folder_style")).toEqual([
      { input: { id: expect.any(String), expectedRevision: 1, icon: "chef-hat", color: "pink" } },
    ]));
    await eventually(() => expect(rowIcon(within(tree()).getByRole("button", { name: "Recipes" }))).toHaveClass("lucide-chef-hat"));
    expect(rowIcon(within(tree()).getByRole("button", { name: "Recipes" }))).toHaveAttribute("style", "color: var(--style-pink);");
  });
});
