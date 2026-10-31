import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import CommandPalette from "./components/CommandPalette";
import { adoptIndexHtml, RULES_JSDOM_CANNOT_JUDGE, runAxe } from "./test/axe";
import { freshWorld, SCENES, world } from "./test/a11y-scenes";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/**
 * Axe over the real components with mocked Tauri data, one scene per view and
 * state, in both appearances. Colour contrast and anything needing layout is
 * off here (see RULES_JSDOM_CANNOT_JUDGE). Synthetic data only.
 *
 * Findings accepted for now, by rule, each with its reason, are a ratchet: the
 * findings must equal KNOWN exactly, so a new one fails and so does one that
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
  freshWorld();
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

describe("the axe harness", () => {
  it("refuses an answer from a page that keeps changing", async () => {
    render(<main><h1>Synthetic</h1></main>);
    const heading = screen.getByRole("heading");
    let count = 0;
    const changing = window.setInterval(() => { heading.setAttribute("data-count", String(count++)); }, 1);
    try {
      await expect(runAxe()).rejects.toThrow("kept changing");
    } finally {
      window.clearInterval(changing);
    }
  });
});

