/// <reference types="node" />
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * jsdom applies no stylesheet, so focus, motion and contrast rules are checked
 * on App.css itself. The native click-checklist in Ticket 14 covers how they look.
 */
// Vitest hands CSS imports back empty, so read the file itself.
const css = readFileSync(resolve(__dirname, "App.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

interface Rule { selector: string; body: string; media: string | null }

/** Flat list of rules with the @media block (if any) each sits in. */
function rules(source: string): Rule[] {
  const found: Rule[] = [];
  let depth = 0;
  let media: string | null = null;
  let start = 0;
  let selector = "";
  for (let i = 0; i < source.length; i++) {
    const ch = source[i];
    if (ch === "{") {
      const head = source.slice(start, i).trim();
      if (head.startsWith("@media")) media = head;
      else selector = head;
      depth++;
      start = i + 1;
    } else if (ch === "}") {
      if (selector) {
        found.push({ selector, body: source.slice(start, i), media: depth > 1 ? media : null });
        selector = "";
      } else if (media) media = null;
      depth--;
      start = i + 1;
    }
  }
  return found;
}

const all = rules(css);
const has = (selector: string, property: RegExp) =>
  all.some((rule) => rule.selector.split(",").map((part) => part.trim()).includes(selector) && property.test(rule.body));

describe("Stylesheet accessibility", () => {
  it("gives every control that hides the native outline a visible focus replacement", () => {
    // The hidden outline → where focus shows instead (DESIGN.md: outline, accent underline, or the hover step).
    const replacements: Record<string, [string, RegExp]> = {
      ".folder-form input": [".folder-form:focus-within", /background:/],
      ".toolbar-search input": [".toolbar-search:focus-within", /outline: 2px/],
      ".toolbar-sort select": [".toolbar-sort:focus-within", /outline: 2px/],
      ".capture-title": [".capture:focus-within", /background:/],
      ".capture-description": [".capture:focus-within", /background:/],
      ".palette-field input": [".palette-field:focus-within", /border-bottom-color:/],
      ".property input": [".property:focus-within", /outline: 2px/],
      ".property select": [".property:focus-within", /outline: 2px/],
      ".progress-number input": [".progress-number:focus-within", /outline: 2px/],
      ".next-step-input": [".next-step-input:focus", /border-bottom-color:|border-color:/],
      ".progress-note-input": [".progress-note-input:focus-visible", /outline: 2px/],
      ".note-search input": [".note-search:focus-within", /outline: 2px/],
      // Writing surfaces: the caret is the focus indicator, as in Notes.
      ".task-page-title": [".task-page-title", /outline: none/],
      ".task-page-body": [".task-page-body", /outline: none/],
      ".note-content": [".note-content", /outline: none/],
      ".note-check": [".note-check", /outline: none/],
    };
    const hidden = all.filter((rule) => /outline:\s*(none|0)\b/.test(rule.body))
      .flatMap((rule) => rule.selector.split(",").map((part) => part.trim()));
    for (const selector of hidden) {
      expect(replacements, `no focus replacement recorded for ${selector}`).toHaveProperty([selector]);
      const [replacement, property] = replacements[selector];
      expect(has(replacement, property), `${replacement} should show focus for ${selector}`).toBe(true);
    }
    expect(has(":focus-visible", /outline: 2px solid var\(--focus\)/)).toBe(true);
  });

  it("only moves anything when Reduce Motion is off", () => {
    const moving = all.filter((rule) => /\b(transition|animation)\s*:/.test(rule.body) && !/animation:\s*none/.test(rule.body));
    expect(moving.length).toBeGreaterThan(0);
    for (const rule of moving) {
      const allowed = rule.media?.includes("prefers-reduced-motion: no-preference") ||
        // The loading spinner is turned off under Reduce Motion below.
        rule.selector === ".spin";
      expect(allowed, `${rule.selector} moves outside the no-preference block`).toBe(true);
    }
    expect(all.some((rule) => rule.media?.includes("prefers-reduced-motion: reduce") &&
      rule.selector === ".spin" && /animation:\s*none/.test(rule.body))).toBe(true);
    // DESIGN.md: 120ms fades and a 240ms progress fill; nothing slow.
    for (const duration of css.matchAll(/(\d+)ms/g)) {
      if (Number(duration[1]) !== 900) expect(Number(duration[1])).toBeLessThanOrEqual(240);
    }
  });

  it("keeps text readable on its background in both appearances", () => {
    const tokens = (selector: string) => Object.fromEntries(
      [...all.find((rule) => rule.selector === selector)!.body.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)]
        .map(([, name, value]) => [name, value]),
    );
    const luminance = (hex: string) => {
      const [r, g, b] = [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16) / 255)
        .map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const contrast = (a: string, b: string) => {
      const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
      return (hi + 0.05) / (lo + 0.05);
    };
    const dark = tokens(":root");
    const light = { ...dark, ...tokens(':root[data-theme="light"]') };
    // WCAG 2 AA for small text: 4.5:1.
    const pairs: [string, string][] = [
      ["text", "bg"], ["text-2", "bg"], ["text-3", "bg"], ["text", "sidebar"], ["text-2", "sidebar"],
      ["text-3", "sidebar"], ["accent-text", "bg"], ["red", "bg"], ["orange", "bg"],
      ["row-selected-text", "row-selected"], ["accent-ink", "accent"], ["text-2", "hover"],
    ];
    for (const [theme, palette] of [["dark", dark], ["light", light]] as const) {
      for (const [fg, bg] of pairs) {
        expect(contrast(palette[fg], palette[bg]), `${theme}: ${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});
