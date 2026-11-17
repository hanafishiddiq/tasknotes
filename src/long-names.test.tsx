import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { NoteBackend } from "./test/note-backend";
import { declared, rules, selectorsOf } from "./test/stylesheet";

/**
 * A name the person writes can be any length. Wherever one shows on a single
 * line it ends in "…" inside its own box instead of wrapping onto, or spilling
 * over, what sits next to it (the Notes column title once covered the search
 * field). jsdom lays nothing out, so the rules are checked on App.css; the
 * layout itself is checked in a browser with qa-harness/long-names.html.
 */
const outside = rules.filter((rule) => rule.media === null);
const value = (selector: string, property: string) => {
  const rule = outside.find((each) => selectorsOf(each).includes(selector) && declared(each, property) !== null);
  return rule ? declared(rule, property) : null;
};

/** Every one-line place that shows a name or title the person wrote. */
const ONE_LINE_NAMES = [
  ".note-list-header h1", // a Note Folder's name above its Notes
  ".page-title h1", // a List's name above its Tasks
  ".folder-name", // Lists and Note Folders in the sidebar
  ".note-list-title",
  ".task-row-title",
  ".linked-title",
  ".removed-item-title",
  ".note-hit-title",
  ".pomo-option-title",
  ".pomo-task-title",
  ".pomodoro-status",
  ".palette-label",
];

describe("long names on one line", () => {
  it.each(ONE_LINE_NAMES)("%s stays on one line and ends in an ellipsis", (selector) => {
    expect(value(selector, "white-space"), selector).toBe("nowrap");
    expect(value(selector, "overflow"), selector).toBe("hidden");
    expect(value(selector, "text-overflow"), selector).toBe("ellipsis");
  });

  it("ends every menu's chosen name in an ellipsis rather than cutting it mid-letter", () => {
    // Lists, Note Folders and Pomodoro Tasks are offered in menus; WebKit honours this on a closed menu.
    const base = outside.find((rule) => selectorsOf(rule).includes("select") && declared(rule, "text-overflow") !== null);
    expect(base && declared(base, "text-overflow")).toBe("ellipsis");
    expect(outside.filter((rule) => /select/.test(rule.selector) && declared(rule, "text-overflow") === "clip")).toEqual([]);
  });

  it("breaks a name with no spaces where text may wrap, instead of spilling out of its box", () => {
    // Sentences that carry a name (Undo, Unlink, Approvals, "Updated … · List") wrap; one long word must too.
    expect(value("body", "overflow-wrap")).toBe("break-word");
    // A flex row's text shrinks beside its Undo button rather than pushing it off the edge.
    expect(value(".undo-bar > span", "min-width")).toBe("0");
  });

  it("caps a sidebar entry in the narrow window, where the sidebar becomes a sideways row", () => {
    const narrow = rules.filter((rule) => rule.media?.includes("max-width: 720px"));
    const cap = narrow.find((rule) => selectorsOf(rule).includes(".folder-row") && declared(rule, "max-width") !== null);
    expect(cap && declared(cap, "max-width")).toBe("200px");
    // The name's button shrinks inside the capped entry; otherwise names run over each other.
    const shrink = narrow.find((rule) => selectorsOf(rule).includes(".folder-row .folder") && declared(rule, "min-width") !== null);
    expect(shrink && declared(shrink, "min-width")).toBe("0");
    expect(shrink && declared(shrink, "flex")).toBe("0 1 auto");
  });

  it("lets the column and page titles shrink beside their buttons and counts", () => {
    // A flex item does not shrink below its text unless told, so it would push the New note button out.
    expect(value(".note-list-header h1", "min-width")).toBe("0");
    expect(value(".page-title h1", "min-width")).toBe("0");
    expect(value(".page-title span", "flex")).toBe("none");
    // …and the button beside a long title keeps its full size, a gap away.
    expect(value(".note-list-header .icon-button", "flex")).toBe("none");
    expect(value(".note-list-header", "gap")).toBe("8px");
  });
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const LONG = "Synthetic folder name that goes on far longer than any column is wide";
let backend: NoteBackend;

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  backend = new NoteBackend();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command, args) => {
    if (command === "list_lists") {
      return { items: [{ id: "l1", name: LONG, revision: 1, createdAt: 1, updatedAt: 1, removed: false }], nextCursor: null };
    }
    if (command === "list_tasks") return { items: [], nextCursor: null };
    if (command === "quit_ready") return null;
    return backend.handle(command, (args ?? {}) as Record<string, unknown>);
  });
});

afterEach(() => cleanup());

describe("a shortened title still says the whole name", () => {
  it("on a Note Folder's column title", async () => {
    backend.folder(LONG);
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const tree = await screen.findByRole("navigation", { name: "Note folders" });
    await user.click(await within(tree).findByRole("button", { name: LONG }));
    const title = await screen.findByRole("heading", { level: 1, name: LONG });
    expect(title).toHaveAttribute("title", LONG);
  });

  it("on a List's page title", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: LONG }));
    await waitFor(() => expect(screen.getByRole("heading", { level: 1, name: LONG })).toHaveAttribute("title", LONG));
  });
});
