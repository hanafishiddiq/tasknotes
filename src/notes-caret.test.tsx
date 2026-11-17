import { describe, expect, it } from "vitest";
import { contrast, declared, parseColor, rules, selectorsOf, tokens } from "./test/stylesheet";

/**
 * The typing caret in a Note: the owner found the dark theme's full yellow too
 * bright and asked for it a little darker (2026-11-17). Light keeps its gold.
 */
const caretRule = rules.find((rule) => rule.media === null && selectorsOf(rule).includes(".note-content") && declared(rule, "caret-color"));

describe("the Note caret", () => {
  it("uses its own colour token", () => {
    expect(caretRule && declared(caretRule, "caret-color")).toBe("var(--caret)");
  });

  it("is a little darker than the accent in the dark theme, and still yellow and easy to see", () => {
    const dark = tokens("dark");
    const caret = parseColor(dark.caret), accent = parseColor(dark.accent);
    const brightness = (c: { r: number; g: number; b: number }) => c.r + c.g + c.b;
    // A little darker: 10–30% less light than the accent, not a different colour.
    expect(brightness(caret) / brightness(accent)).toBeGreaterThan(0.7);
    expect(brightness(caret) / brightness(accent)).toBeLessThan(0.9);
    expect(Math.min(caret.r, caret.g) - caret.b).toBeGreaterThanOrEqual(100);
    expect(contrast(caret, parseColor(dark.bg))).toBeGreaterThanOrEqual(3);
  });

  it("keeps the light theme's gold", () => {
    const light = tokens("light");
    expect(light.caret).toBe(light.accent);
  });
});
