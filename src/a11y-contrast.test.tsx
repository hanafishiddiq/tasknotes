import { describe, expect, it } from "vitest";
import {
  brightened, contrast, declared, mixed, over, parseColor, ruleDeclaring, rules, selectorsOf, tokens,
  type Color, type Theme,
} from "./test/stylesheet";

/**
 * WCAG AA contrast computed from the values in App.css, for both appearances.
 *
 * Which colour sits on which surface cannot be read from the stylesheet alone:
 * it depends on where the components put each class. So TEXT and INDICATORS
 * below say, by selector, where each one lands. The colour always comes from the
 * stylesheet, and a completeness check fails when a rule with a colour is in
 * neither table, so a new one cannot go unchecked.
 */
const THEMES: Theme[] = ["dark", "light"];
const AA_TEXT = 4.5;
const AA_LARGE = 3;
const AA_NON_TEXT = 3;

/** Where things sit. All but the active filter chip are tokens used as fills. */
type Surface = "bg" | "sidebar" | "hover" | "side-selected" | "field" | "row-selected" | "accent" | "filter-chip";

interface Placement {
  on: Surface[];
  /** For a rule that says `inherit` or `currentColor`: the token that comes through. */
  through?: string;
}

const ROW: Surface[] = ["bg", "hover"];
const SELECTED: Placement = { on: ["row-selected"], through: "row-selected-text" };

/** Text, by the selector that gives it its colour. 4.5:1, or 3:1 when the rule makes it large. */
const TEXT: Record<string, Placement> = {
  // Text with no colour of its own: headings, row titles, palette rows, code, the "%" sign.
  ":root": { on: ["bg", "hover", "sidebar", "field"] },
  // Shell and sidebar
  ".undo-bar": { on: ["hover"] },
  ".undo-bar--error": { on: ["hover"] },
  ".sidebar-heading": { on: ["sidebar"] },
  ".folder": { on: ["sidebar", "hover", "side-selected"] },
  ".folder-count": { on: ["sidebar", "hover"] },
  '.folder[aria-current="page"] .folder-count': { on: ["side-selected"] },
  ".folder--quiet": { on: ["sidebar", "hover", "side-selected"] },
  ".folder-form input": { on: ["sidebar", "hover"] },
  ".folder-form input::placeholder": { on: ["sidebar", "hover"] },
  ".sidebar-error": { on: ["sidebar"] },
  ".row-drag-ghost": { on: ["side-selected"] },
  ".sidebar-link": { on: ["sidebar"] },
  ".sidebar-link:hover:not(:disabled)": { on: ["sidebar"] },
  ".folder-form select": { on: ["field"] },
  // Toolbar, filters, capture
  ".toolbar-search input": { on: ["field"] },
  ".toolbar-search input::placeholder": { on: ["field"] },
  ".toolbar-sort": { on: ["bg"] },
  ".toolbar-sort:hover": { on: ["hover"] },
  ".property.filter-chip--active": { on: ["filter-chip"] },
  ".property.filter-chip--active select": { on: ["filter-chip"] },
  ".property.filter-chip--active input": { on: ["filter-chip"] },
  ".page-title span": { on: ["bg"] },
  ".capture-title": { on: ROW },
  ".capture-title::placeholder": { on: ROW },
  ".capture-description": { on: ["hover"] },
  ".capture-description::placeholder": { on: ["hover"] },
  ".capture-actions": { on: ["hover"] },
  // Buttons and banners
  ".button": { on: ["bg", "hover"] },
  ".button:hover:not(:disabled)": { on: ["hover"] },
  ".button-primary": { on: ["accent"] },
  ".button-primary:hover:not(:disabled)": { on: ["accent"] },
  ".error-banner": { on: ["hover"] },
  ".notice-banner": { on: ["hover"] },
  ".loading-state": { on: ["bg"] },
  ".empty-state": { on: ["bg"] },
  ".empty-state h2": { on: ["bg"] },
  // Settings
  ".settings-heading": { on: ["bg"] },
  ".settings-choice": { on: ROW },
  ".settings-hint": { on: ["bg"] },
  ".settings-sound-row": { on: ROW },
  ".settings-sound-row[data-muted] label": { on: ROW },
  ".settings-sound-row[data-muted] output": { on: ROW },
  ".settings-sound-row output": { on: ROW },
  ".settings-shortcut dt": { on: ["bg"] },
  ".settings-backup": { on: ["bg"] },
  ".settings-backup-kind": { on: ["bg"] },
  ".settings-hint--error": { on: ["bg"] },
  ".settings-hint--error p:first-child": { on: ["bg"] },
  "kbd": { on: ["bg"] },
  // Palette
  ".palette-field input": { on: ["sidebar"] },
  '.palette-option[aria-selected="true"]': { on: ["row-selected"] },
  ".palette-group": { on: ["sidebar"] },
  '.palette-option[aria-selected="true"] .palette-group': SELECTED,
  ".palette-empty": { on: ["sidebar"] },
  // Removed and approvals
  ".removed-hint": { on: ["bg"] },
  ".removed-item": { on: ["bg"] },
  ".removed-item-title": { on: ["bg"] },
  ".removed-item-meta": { on: ["bg"] },
  ".removed-item-blocked": { on: ["bg"] },
  ".approval-title": { on: ["bg"] },
  ".approval-meta": { on: ["bg"] },
  ".approval-stale": { on: ["bg"] },
  // Task rows
  ".task-group-heading span": { on: ["bg"] },
  ".note-hit": { on: ROW },
  ".note-hits-empty": { on: ["bg"] },
  ".task-card--expanded .task-row": { on: ["row-selected"] },
  ".task-card--completed .task-row-title": { on: ROW },
  ".task-row-subtitle": { on: ROW },
  ".progress-readout": { on: ROW },
  ".task-row-meta": { on: ROW },
  ".priority-pill--high": { on: ROW },
  ".priority-pill--very_high": { on: ROW },
  ".deadline-pill--overdue": { on: ROW },
  ".deadline-pill--today": { on: ROW },
  ".unsaved-marker": { on: ROW },
  ".tag-pill": { on: ROW },
  ".tag-pill--more": { on: ROW },
  ".task-card--expanded .task-row-title": SELECTED,
  ".task-card--expanded .task-row-subtitle": SELECTED,
  ".task-card--expanded .progress-readout": SELECTED,
  ".task-card--expanded .task-row-meta": SELECTED,
  ".task-card--expanded .task-row-meta > span": SELECTED,
  // The Task page
  ".task-stamp": { on: ["bg"] },
  ".task-page-title": { on: ["bg"] },
  ".task-page-body": { on: ["bg"] },
  ".task-page-body::placeholder": { on: ["bg"] },
  ".next-step-input::placeholder": { on: ["bg"] },
  ".progress-note-input::placeholder": { on: ["field"] },
  ".property input::placeholder": { on: ["field"] },
  ".property": { on: ["field"] },
  ".property-date-empty-label": { on: ["field"] },
  ".property select": { on: ["field"] },
  ".property input": { on: ["field"] },
  ".field-label": { on: ["bg"] },
  ".progress-number input": { on: ["field"] },
  ".next-step-input": { on: ["bg"] },
  ".progress-note-input": { on: ["field"] },
  ".metadata-hint": { on: ["bg"] },
  ".progress-history-heading": { on: ["bg"] },
  ".history-hint": { on: ["bg"] },
  ".history-hint button": { on: ["bg"] },
  ".history-load-more": { on: ["bg"] },
  ".history-entry-summary": { on: ["bg"] },
  ".history-entry-date": { on: ["bg"] },
  ".history-entry-note": { on: ["bg"] },
  ".history-entry-next": { on: ["bg"] },
  ".history-error": { on: ["bg"] },
  // Links between Tasks and Notes
  ".linked-heading": { on: ["bg"] },
  ".linked-open": { on: ["bg", "hover", "field"] },
  ".linked-status": { on: ["bg", "hover", "field"] },
  ".linked-add": { on: ["bg"] },
  ".linked-picker-actions .text-link": { on: ["field"] },
  ".linked-undo .text-link": { on: ["bg"] },
  ".linked-undo": { on: ["bg"] },
  ".linked-search": { on: ["bg"] },
  ".linked-empty": { on: ["field"] },
  ".linked-section .inline-error": { on: ["bg"] },
  // Notes
  ".note-list-item": { on: ROW },
  '.note-list-item[aria-current="true"]': { on: ["row-selected"] },
  ".note-list-more": { on: ["bg"] },
  ".note-list-date": { on: ROW },
  '.note-list-item[aria-current="true"] .note-list-date': SELECTED,
  ".note-folder-picker": { on: ["field"] },
  ".note-folder-picker select": { on: ["field"] },
  ".note-save-state": { on: ["bg"] },
  ".note-search input": { on: ["field"] },
  ".note-list-empty": { on: ["bg"] },
  ".note-list-snippet": { on: ROW },
  ".note-list-snippet mark": { on: ROW },
  '.note-list-item[aria-current="true"] .note-list-snippet': SELECTED,
  '.note-list-item[aria-current="true"] .note-list-snippet mark': SELECTED,
  ".note-list-heading": { on: ["bg"] },
  ".note-content": { on: ["bg"] },
  ".note-placeholder": { on: ["bg"] },
  ".note-link": { on: ["bg"] },
  ".note-quote": { on: ["bg"] },
  ".note-check--done": { on: ["bg"] },
};

/** Marks that carry meaning without words: 3:1. Each is one rule's property. */
const INDICATORS: { selector: string; property: string; means: string; place: Placement }[] = [
  { selector: ":focus-visible", property: "outline", means: "focus ring", place: { on: ["bg", "sidebar", "hover", "side-selected", "field", "row-selected"] } },
  { selector: ".palette-field:focus-within", property: "border-bottom-color", means: "focus underline", place: { on: ["sidebar"] } },
  { selector: ".next-step-input:focus", property: "border-bottom-color", means: "focus underline", place: { on: ["bg"] } },
  { selector: ".task-check", property: "border", means: "Ready ring", place: { on: ["bg", "hover", "field"] } },
  { selector: ".task-check--in_progress", property: "border-color", means: "In Progress ring", place: { on: ["bg", "hover", "field"] } },
  { selector: ".task-check--in_progress::after", property: "background", means: "In Progress dot", place: { on: ["bg", "hover", "field"] } },
  { selector: ".task-check--completed", property: "background", means: "Completed disc", place: { on: ["bg", "hover", "field"] } },
  { selector: ".task-check--completed", property: "color", means: "Completed tick", place: { on: ["accent"] } },
  { selector: ".task-card--expanded .task-row .task-check", property: "border-color", means: "ring on the open row", place: SELECTED },
  { selector: ".task-card--expanded .task-row .task-check--in_progress::after", property: "background", means: "dot on the open row", place: SELECTED },
  { selector: ".task-card--expanded .task-row .task-check--completed", property: "background", means: "disc on the open row", place: { on: ["row-selected"] } },
  { selector: ".capture-ring", property: "border", means: "New task ring", place: { on: ["bg", "hover"] } },
  { selector: ".note-check::before", property: "border", means: "open checklist circle", place: { on: ["bg"] } },
  { selector: ".note-check--done::before", property: "background", means: "done checklist circle", place: { on: ["bg"] } },
  { selector: ".note-check--done::after", property: "border", means: "checklist tick", place: { on: ["accent"] } },
  { selector: ".settings-choice input", property: "accent-color", means: "checked radio or checkbox", place: { on: ["bg", "hover"] } },
  { selector: ".settings-sound-row input", property: "accent-color", means: "volume slider fill", place: { on: ["bg", "hover"] } },
  // Icon-only controls, where the glyph is all there is to see.
  { selector: ".icon-button", property: "color", means: "icon-only button", place: { on: ["bg"] } },
  { selector: ".icon-button:hover:not(:disabled)", property: "color", means: "icon-only button", place: { on: ["hover"] } },
  { selector: '.note-toolbar .icon-button[aria-pressed="true"]', property: "color", means: "pressed icon button", place: { on: ["bg"] } },
  { selector: ".folder-action", property: "color", means: "row icon button", place: { on: ["sidebar", "hover", "side-selected"] } },
  { selector: ".folder-action:hover:not(:disabled)", property: "color", means: "row icon button", place: { on: ["hover"] } },
  { selector: ".folder-disclosure", property: "color", means: "folder disclosure arrow", place: { on: ["sidebar"] } },
  { selector: "button.folder-disclosure:hover:not(:disabled)", property: "color", means: "folder disclosure arrow", place: { on: ["sidebar"] } },
  { selector: ".linked-unlink", property: "color", means: "unlink button", place: { on: ["bg", "hover"] } },
  { selector: ".settings-mute", property: "color", means: "mute button", place: { on: ["bg", "hover"] } },
];

/** Rules with a colour that deliberately need no pair here, each with why. */
const NO_PAIR_NEEDED: Record<string, string> = {
  "::selection": "checked on the highlight below, as what every selected text takes",
  "button": "says inherit: what comes through is checked at its source",
  "input": "says inherit: what comes through is checked at its source",
  "textarea": "says inherit: what comes through is checked at its source",
  "select": "says inherit: what comes through is checked at its source",
  ".toolbar-sort select": "says inherit: checked as .toolbar-sort",
  ".button-primary:disabled": "disabled controls are exempt from WCAG contrast",
  ".folder-row--removed": "no component uses this class any more",
  // Decorative icons beside a text label that says the same thing.
  ".folder svg": "decorative icon beside its name",
  ".folder--quiet svg": "decorative icon beside its name",
  ".folder-form svg": "decorative icon beside its field",
  ".folder-form--new svg": "decorative icon beside its field",
  ".folder-form--nested svg": "decorative icon beside its field",
  ".settings-choice svg": "decorative icon beside its label",
  ".note-hit svg": "decorative icon beside its title",
  ".linked-open svg": "decorative icon beside its title",
  ".note-folder-picker svg": "decorative icon beside its select",
  ".toolbar-search": "the colour reaches only a decorative icon; the field sets its own",
  ".palette-field": "the colour reaches only a decorative icon; the field sets its own",
  ".palette-field:focus-within": "the colour reaches only a decorative icon; the underline is checked",
  ".note-search": "the colour reaches only a decorative icon; the field sets its own",
  ".task-card--expanded .task-row .task-check--completed": "this colour is the disc's; the disc is checked",
  ".task-card--expanded .task-row .task-check--completed svg": "the tick sits on the disc; the disc is checked",
};

/** Lines and fills that separate or decorate and say nothing by themselves. */
const DECORATIVE_LINES = [
  ".undo-bar", ".sidebar", ".toolbar", ".settings-shortcut", ".settings-backup", ".palette", ".palette-field",
  ".removed-item", ".approval", ".task-slot + .task-slot .task-card::before", ".history-entry", ".linked-item",
  ".note-list-pane", ".next-step-input", ".note-quote", ".row-drag-ghost",
];

/** These repeat the ring on a wrapper, so they are the same colour on the same surfaces. */
const OUTLINED_LIKE_THE_RING = [
  ".toolbar-search:focus-within", ".toolbar-sort:focus-within", ".settings-choice:has(input:focus-visible)",
  ".settings-mute:focus-visible", ".settings-sound-row input:focus-visible", ".property:focus-within",
  ".progress-number:focus-within", ".progress-note-input:focus-visible", ".note-search:focus-within",
];

/** Text colours as selected text, against the highlight over what they are written on. */
const SELECTED_TEXT: Record<string, Surface[]> = {
  ":root": ["bg", "hover", "field", "sidebar"],
  ".note-quote": ["bg"],
  ".note-link": ["bg"],
  ".note-check--done": ["bg"],
  ".capture-description": ["hover"],
};

interface Pair { id: string; foreground: Color; surface: Color; need: number }

function palette(theme: Theme) {
  const values = tokens(theme);
  const color = (name: string): Color => parseColor(values[name]);
  const chip = /color-mix\(in srgb, var\(--([\w-]+)\) (\d+)%, var\(--([\w-]+)\)\)/.exec(
    declared(ruleDeclaring(".property.filter-chip--active", "background")!, "background")!,
  )!;
  const surface = (name: Surface): Color =>
    name === "filter-chip" ? mixed(color(chip[1]), Number(chip[2]), color(chip[3])) : color(name);
  /** The one var() or currentColor in a declaration's value, as a colour. */
  const resolve = (value: string, place: Placement, own: string | null): Color => {
    const named = /var\(--([\w-]+)\)/.exec(value);
    if (named) return color(named[1]);
    if (/currentColor|inherit/i.test(value)) {
      if (own && !/currentColor|inherit/i.test(own)) return resolve(own, place, null);
      if (place.through) return color(place.through);
    }
    throw new Error(`Cannot tell what colour "${value}" is here`);
  };
  return { color, surface, resolve };
}

/** A translucent colour or element as it really shows, then its contrast there. */
function shown(foreground: Color, surface: Color, opacity: number): [Color, Color] {
  return [over({ ...foreground, a: foreground.a * opacity }, surface), surface];
}

function textPairs(theme: Theme): Pair[] {
  const { surface, resolve } = palette(theme);
  return Object.entries(TEXT).flatMap(([selector, place]) => {
    const rule = ruleDeclaring(selector, "color");
    if (!rule) throw new Error(`No colour in App.css for ${selector}`);
    const size = Number(/([\d.]+)px/.exec(declared(rule, "font-size") ?? "")?.[1] ?? 0);
    const bold = Number(declared(rule, "font-weight") ?? 400) >= 700;
    const large = size >= 24 || (size >= 18.66 && bold);
    const opacity = Number(declared(rule, "opacity") ?? 1);
    const lift = /brightness\(([\d.]+)\)/.exec(declared(rule, "filter") ?? "")?.[1];
    return place.on.map((name): Pair => {
      let [foreground, under] = shown(resolve(declared(rule, "color")!, place, null), surface(name), opacity);
      if (lift) [foreground, under] = [brightened(foreground, Number(lift)), brightened(under, Number(lift))];
      return { id: `${theme} text ${selector} on ${name}`, foreground, surface: under, need: large ? AA_LARGE : AA_TEXT };
    });
  });
}

function indicatorPairs(theme: Theme): Pair[] {
  const { surface, resolve } = palette(theme);
  return INDICATORS.flatMap(({ selector, property, means, place }) => {
    const rule = ruleDeclaring(selector, property);
    if (!rule) throw new Error(`No ${property} in App.css for ${selector}`);
    return place.on.map((name): Pair => {
      const [foreground, under] = shown(resolve(declared(rule, property)!, place, declared(rule, "color")), surface(name), 1);
      return { id: `${theme} indicator ${means} (${selector} ${property}) on ${name}`, foreground, surface: under, need: AA_NON_TEXT };
    });
  });
}

function selectionPairs(theme: Theme): Pair[] {
  const { color, surface, resolve } = palette(theme);
  return Object.entries(SELECTED_TEXT).flatMap(([selector, on]) => on.map((name): Pair => {
    const highlight = over(color("selection"), surface(name));
    // Highlighted text takes the ::selection colour, whatever its own colour is.
    const foreground = resolve(declared(ruleDeclaring("::selection", "color")!, "color")!, { on }, null);
    return { id: `${theme} selected text ${selector} on the highlight over ${name}`, foreground, surface: highlight, need: AA_TEXT };
  }));
}

const allPairs = (theme: Theme) =>
  [...textPairs(theme), ...indicatorPairs(theme), ...selectionPairs(theme)];

const ratio = (pair: Pair) => contrast(pair.foreground, pair.surface);

describe.each(THEMES)("WCAG AA contrast from App.css, %s appearance", (theme) => {
  for (const pair of allPairs(theme)) {
    it(pair.id, () => {
      expect(ratio(pair), pair.id).toBeGreaterThanOrEqual(pair.need);
    });
  }
});

describe("the contrast check itself", () => {
  it("computes the published reference values", () => {
    const [black, white] = [parseColor("#000000"), parseColor("#ffffff")];
    expect(contrast(black, white)).toBeCloseTo(21, 5);
    // #777777 on white is the textbook near-miss.
    expect(contrast(parseColor("#777777"), white)).toBeCloseTo(4.48, 2);
    expect(over(parseColor("rgba(0, 0, 0, 0.5)"), white)).toEqual({ r: 127.5, g: 127.5, b: 127.5, a: 1 });
  });

  it("knows every rule that sets a text colour", () => {
    const hasColor = rules.filter((rule) => rule.media === null && declared(rule, "color"))
      .flatMap((rule) => selectorsOf(rule))
      .filter((selector) => selector !== "from");
    const placed = new Set([
      ...Object.keys(TEXT), ...Object.keys(NO_PAIR_NEEDED),
      ...INDICATORS.filter((item) => item.property === "color").map((item) => item.selector),
    ]);
    expect(hasColor.filter((selector) => !placed.has(selector))).toEqual([]);
  });

  it("knows every rule that draws a line or a filled mark in a token colour", () => {
    const inColor = rules.filter((rule) => rule.media === null && ["border", "border-top", "border-right", "border-bottom", "border-left", "border-color", "border-bottom-color", "outline", "accent-color"]
      .some((property) => /var\(--|currentColor/.test(declared(rule, property) ?? "")))
      .flatMap((rule) => selectorsOf(rule));
    const placed = new Set([...DECORATIVE_LINES, ...INDICATORS.map((item) => item.selector), ...OUTLINED_LIKE_THE_RING]);
    expect(inColor.filter((selector) => !placed.has(selector))).toEqual([]);
  });

  it("uses no fill it does not check text against", () => {
    const tokensAsFills = new Set(rules.filter((rule) => rule.media === null)
      .flatMap((rule) => [...(declared(rule, "background") ?? "").matchAll(/^var\(--([\w-]+)\)$/g)].map((match) => match[1])));
    expect([...tokensAsFills].sort()).toEqual(["accent", "bg", "field", "hover", "row-selected", "row-selected-text", "selection", "sep", "side-selected", "sidebar"]);
  });

});

/** The stylesheet does not decide these, so only the owner's on-screen check can. */
describe("contrast the stylesheet cannot settle", () => {
  it.todo("placeholders in the palette, Notes search and link search: no ::placeholder rule, so WebKit's default colour shows");
  it.todo("unchecked checkbox and radio frames and the slider track: WebKit draws them; accent-color only sets the checked fill");
  it.todo("select menus and the date picker's popover: the system draws them");
  it.todo("the Next Step and Note carets: caret-color is set for Notes only, and a caret blinks");
});
