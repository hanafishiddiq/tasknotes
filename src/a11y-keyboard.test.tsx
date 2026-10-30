import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { freshWorld, SCENES, world, type User } from "./test/a11y-scenes";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * Keyboard reach, checked over the same scenes axe looks at, and where focus
 * goes when something closes. jsdom has no focus rings: this says focus lands on
 * the right element, not that a person can see it. Synthetic data only.
 */
const eventually = { timeout: 5000 };

beforeAll(() => import("./components/NotesWorkspace"));

beforeEach(() => {
  window.localStorage.clear();
  freshWorld();
  call.mockReset();
  call.mockImplementation(async (command, args) => world.handle(command, (args ?? {}) as Record<string, any>));
});

afterEach(() => cleanup());

const FOCUSABLE = 'button, a[href], input, select, textarea, [contenteditable="true"], [tabindex]';

const describeElement = (element: Element) =>
  `${element.tagName.toLowerCase()} "${element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 40) ?? ""}"`;

/** What Tab should reach now: enabled, shown, outside anything inert, one stop per radio group. */
function reachableControls(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => {
    if (element.matches(":disabled") || Number(element.getAttribute("tabindex")) < 0) return false;
    if (element.closest("[hidden], [inert]")) return false;
    return !(element.matches("input[type=radio]") && !(element as HTMLInputElement).checked);
  });
}

/** Starts on the first control, presses Tab until focus comes back round, and says what it landed on. */
async function tabStops(user: User): Promise<Element[]> {
  const first = reachableControls()[0];
  act(() => first.focus());
  const stops: Element[] = [first];
  for (let press = 0; press < 400; press++) {
    await user.tab();
    const at = document.activeElement!;
    if (at === document.body || stops.includes(at)) break;
    stops.push(at);
  }
  return stops;
}

/** The handlers React holds for an element. They are delegated to the root, so only React knows. */
function reactProps(element: Element): Record<string, unknown> | null {
  const key = Object.keys(element).find((name) => name.startsWith("__reactProps$"));
  return key ? (element as unknown as Record<string, Record<string, unknown>>)[key] : null;
}

const POINTER_HANDLERS = ["onClick", "onDoubleClick", "onMouseDown", "onMouseUp", "onPointerDown", "onPointerUp"];
const NATIVELY_INTERACTIVE = "button, a[href], input, select, textarea, label, summary";
/** Pointer handlers on purpose off the Tab order, each with its keyboard way to the same thing. */
const POINTER_ONLY_ON_PURPOSE: Record<string, string> = {
  ".palette-layer": "a click on the backdrop closes the palette; Escape does the same",
  ".palette-option": "the palette's field holds focus; the arrows and Return choose an option",
};

function pointerOnlyElements(): string[] {
  return [...document.querySelectorAll("body *")].filter((element) => {
    const props = reactProps(element);
    if (!props || !POINTER_HANDLERS.some((name) => props[name])) return false;
    if (element.matches(NATIVELY_INTERACTIVE)) return false;
    return !Object.keys(POINTER_ONLY_ON_PURPOSE).some((selector) => element.matches(selector));
  }).map(describeElement);
}

describe.each(Object.keys(SCENES))("keyboard reach: %s", (name) => {
  async function show() {
    SCENES[name].given?.();
    render(<App />);
    const user = userEvent.setup();
    await SCENES[name].show(user);
    return user;
  }

  it("has no positive tabindex and nothing clickable that a keyboard cannot use", async () => {
    await show();
    expect([...document.querySelectorAll("[tabindex]")].filter((element) => Number(element.getAttribute("tabindex")) > 0)
      .map(describeElement)).toEqual([]);
    expect(pointerOnlyElements()).toEqual([]);
  });

  it("reaches every control with Tab", async () => {
    const user = await show();
    const stops = await tabStops(user);
    // Taken after Tab has moved on: leaving the shortcut recorder, for one, closes it.
    const missed = reachableControls().filter((control) => !stops.includes(control));
    expect(missed.map(describeElement)).toEqual([]);
    expect(stops.length).toBeGreaterThan(0);
  });
});

describe("the keyboard checks themselves", () => {
  it("see pointer handlers and catch a clickable div", () => {
    render(<div><button type="button" onClick={() => undefined}>Fine</button><div className="bad" onClick={() => undefined}>Bad</div></div>);
    expect(reactProps(screen.getByRole("button"))?.onClick).toBeTypeOf("function");
    expect(pointerOnlyElements()).toEqual(['div "Bad"']);
  });

  it("catch a control that traps Tab", async () => {
    render(<div>
      <button type="button" onKeyDown={(event) => { if (event.key === "Tab") event.preventDefault(); }}>Trap</button>
      <button type="button">After</button>
    </div>);
    const user = userEvent.setup();
    const stops = await tabStops(user);
    expect(reachableControls().filter((control) => !stops.includes(control)).map(describeElement)).toEqual(['button "After"']);
  });
});

describe("focus when the command palette closes", () => {
  async function fromInbox() {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Your list is clear");
    const inbox = screen.getByRole("button", { name: "Inbox" });
    inbox.focus();
    await user.keyboard("{Meta>}k{/Meta}");
    return { user, inbox };
  }

  it("stays in its field on Tab and Shift+Tab", async () => {
    const { user } = await fromInbox();
    const field = screen.getByRole("combobox", { name: "Command" });
    await user.tab();
    expect(field).toHaveFocus();
    await user.tab({ shift: true });
    expect(field).toHaveFocus();
  });

  it("goes back after a click outside it", async () => {
    const { inbox } = await fromInbox();
    fireEvent.mouseDown(document.querySelector(".palette-layer")!);
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => expect(inbox).toHaveFocus());
  });

  it("goes back after a command that moves focus nowhere", async () => {
    const { user, inbox } = await fromInbox();
    await user.type(screen.getByRole("combobox", { name: "Command" }), "light");
    await user.keyboard("{Enter}");
    expect(document.documentElement.dataset.theme).toBe("light");
    await waitFor(() => expect(inbox).toHaveFocus());
  });

  it("stays where a command moved it", async () => {
    const { user } = await fromInbox();
    await user.type(screen.getByRole("combobox", { name: "Command" }), "new task");
    await user.keyboard("{Enter}");
    const field = screen.getByRole("textbox", { name: "Task title" });
    await waitFor(() => expect(field).toHaveFocus());
    // Later than the palette's own hand-back, which must not take it away again.
    await act(() => new Promise((done) => setTimeout(done)));
    expect(field).toHaveFocus();
  });
});

describe("focus when the shortcut recorder closes", () => {
  async function recording() {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    const toggle = await screen.findByRole("checkbox", { name: "Open Quick Capture with a global shortcut" });
    await user.click(toggle);
    expect(screen.getByRole("button", { name: "Press the new shortcut" })).toHaveFocus();
    return { user, toggle };
  }

  it("goes back to the switch on Escape", async () => {
    const { user, toggle } = await recording();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("button", { name: "Press the new shortcut" })).toBeNull();
    expect(toggle).toHaveFocus();
  });

  it("goes back to the switch once a shortcut is taken", async () => {
    const { user, toggle } = await recording();
    await user.keyboard("{Control>}{Alt>}{ }{/Alt}{/Control}");
    await waitFor(() => expect(toggle).toHaveFocus());
  });
});

describe("Tab in the Note text", () => {
  it.each([
    ["a paragraph", "Plain"],
    ["a bullet", "- Item"],
    ["a checklist item", "- [ ] Item"],
    ["a code block", "```\nconst synthetic = true;\n```"],
  ])("leaves %s rather than being kept by the editor", async (_, markdown) => {
    world.notes.seed(markdown, "Synthetic");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "All notes" }));
    const text = await screen.findByRole("textbox", { name: "Note text" }, eventually);
    await user.click(text);
    await user.keyboard("x");
    expect(text).toHaveFocus();
    await user.tab({ shift: true });
    expect(text).not.toHaveFocus();
    expect(document.activeElement).not.toBe(document.body);
  });
});
