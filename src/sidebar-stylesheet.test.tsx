import { describe, expect, it } from "vitest";
import { declared, parseColor, rules, selectorsOf, tokens, type Rule } from "./test/stylesheet";

/**
 * jsdom applies no stylesheet, so the sidebar's hover, width and action overlay
 * are checked on App.css. How they look on screen is the native visual check.
 */
const outside = rules.filter((rule) => rule.media === null);
const ruleFor = (selector: string, property: string, from: Rule[] = outside) =>
  from.find((rule) => selectorsOf(rule).includes(selector) && declared(rule, property) !== null) ?? null;
const value = (selector: string, property: string) => {
  const rule = ruleFor(selector, property);
  return rule && declared(rule, property);
};

describe("sidebar hover", () => {
  it("is a pale yellow, opaque and distinct from the sidebar and the current place", () => {
    for (const theme of ["dark", "light"] as const) {
      const palette = tokens(theme);
      expect(palette["side-hover"], theme).toMatch(/^#[0-9a-f]{6}$/i);
      const hover = parseColor(palette["side-hover"]);
      // Yellow: red and green well above blue, nearly equal to each other.
      expect(Math.min(hover.r, hover.g) - hover.b, theme).toBeGreaterThanOrEqual(30);
      expect(Math.abs(hover.r - hover.g), theme).toBeLessThanOrEqual(24);
      expect(palette["side-hover"], theme).not.toBe(palette.sidebar);
      expect(palette["side-hover"], theme).not.toBe(palette["side-selected"]);
    }
  });

  it("fills hovered rows but leaves the current place showing as current", () => {
    expect(value('.folder:hover:not(:disabled):not([aria-current="page"])', "background")).toBe("var(--side-hover)");
    expect(value('.folder-row:hover .folder:not(:disabled):not([aria-current="page"])', "background")).toBe("var(--side-hover)");
    expect(value('.folder[aria-current="page"]', "background")).toBe("var(--side-selected)");
    // No hover rule may reach the current place.
    for (const rule of outside) {
      for (const selector of selectorsOf(rule)) {
        if (/\.folder(?![\w-])[^ ]*:hover|:hover \.folder(?![\w-])/.test(selector) && declared(rule, "background")) {
          expect(selector, selector).toMatch(/:not\(\[aria-current="page"\]\)|data-drop-edge/);
        }
      }
    }
  });

  it("never turns a disabled row yellow", () => {
    for (const rule of outside) {
      for (const selector of selectorsOf(rule)) {
        if (/:hover/.test(selector) && /\.folder(?![\w-])/.test(selector) && declared(rule, "background") === "var(--side-hover)") {
          expect(selector, selector).toContain(":not(:disabled)");
        }
      }
    }
    // A disabled row shows no fill, so its icons cover the name on the sidebar's own colour.
    expect(value(".folder:disabled + .folder-actions", "background")).toBe("var(--sidebar)");
  });
});

describe("sidebar action icons", () => {
  it("show while the row's own button has keyboard focus, not only on hover", () => {
    expect(value(".folder-row:focus-within .folder-actions", "opacity")).toBe("1");
    expect(value(".folder-row:hover .folder-actions", "opacity")).toBe("1");
  });

  it("match the row's drop-target fill while a row is dragged onto it", () => {
    const into = value('.folder-row[data-drop-edge="into"] .folder', "background");
    expect(into).toBe("var(--side-selected)");
    expect(value('.folder-row[data-drop-edge="into"] .folder-actions', "background")).toBe(into);
    // Later in the stylesheet than the hover fill and the disabled fill, at no lower specificity, so it wins.
    const index = (selector: string) => outside.findIndex((rule) => selectorsOf(rule).includes(selector));
    expect(index('.folder-row[data-drop-edge="into"] .folder-actions')).toBeGreaterThan(index(".folder-actions"));
    expect(index('.folder-row[data-drop-edge="into"] .folder-actions')).toBeGreaterThan(index(".folder:disabled + .folder-actions"));
  });

  it("sit over the end of the name on the row's own opaque background", () => {
    expect(value(".folder-actions", "position")).toBe("absolute");
    expect(value(".folder-actions", "background")).toBe("var(--side-hover)");
    expect(value('.folder[aria-current="page"] + .folder-actions', "background")).toBe("var(--side-selected)");
  });

  it("do not reserve a gutter on hover, so a long name never reflows under them", () => {
    const reserving = outside.filter((rule) => declared(rule, "padding-right") !== null &&
      selectorsOf(rule).some((selector) => /\.folder-row:(hover|focus-within) \.folder$/.test(selector)));
    expect(reserving.map((rule) => rule.selector)).toEqual([]);
  });
});

describe("sidebar width", () => {
  it("takes the shell's first column from the remembered width, 260 px by default", () => {
    expect(value(".app-shell", "grid-template-columns")).toBe("var(--sidebar-width, 260px) minmax(0, 1fr)");
  });

  it("puts the edge on the sidebar's right border, outside the sidebar's scrolling", () => {
    expect(value(".sidebar-resizer", "cursor")).toBe("col-resize");
    expect(value(".sidebar-resizer", "position")).toBe("absolute");
    expect(value(".sidebar-resizer", "left")).toBe("calc(var(--sidebar-width, 260px) - 4px)");
    expect(value(".app-shell", "position")).toBe("relative");
    // The sidebar must not become the edge's containing block, or the edge would scroll away with it.
    expect(value(".sidebar", "position")).toBeNull();
  });

  it("hides the edge when the sidebar becomes a top strip on narrow windows", () => {
    const narrow = rules.filter((rule) => rule.media?.includes("max-width"));
    expect(ruleFor(".sidebar-resizer", "display", narrow) && declared(ruleFor(".sidebar-resizer", "display", narrow)!, "display")).toBe("none");
  });
});
