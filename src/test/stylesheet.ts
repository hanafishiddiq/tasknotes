/// <reference types="node" />
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export type Theme = "dark" | "light";
export interface Color { r: number; g: number; b: number; a: number }
export interface Rule { selector: string; body: string; media: string | null }

/** The real stylesheet, without comments. Vitest hands CSS imports back empty. */
export const css = readFileSync(resolve(__dirname, "../App.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** Flat list of rules with the @media block (if any) each sits in. */
export const rules: Rule[] = (() => {
  const found: Rule[] = [];
  let media: string | null = null;
  let selector = "";
  let start = 0;
  for (let i = 0; i < css.length; i++) {
    if (css[i] === "{") {
      const head = css.slice(start, i).trim();
      if (head.startsWith("@")) media = head;
      else selector = head;
      start = i + 1;
    } else if (css[i] === "}") {
      if (selector) found.push({ selector, body: css.slice(start, i), media });
      else media = null;
      selector = "";
      start = i + 1;
    }
  }
  return found;
})();

export function selectorsOf(rule: Rule): string[] {
  return rule.selector.split(",").map((part) => part.replace(/\s+/g, " ").trim());
}

/** A declaration's value in a rule's body, or null. `color` never matches `accent-color`. */
export function declared(rule: Rule, property: string): string | null {
  return new RegExp(`(?:^|[;\\s])${property}:\\s*([^;]+);`).exec(rule.body)?.[1].trim() ?? null;
}

/** The last rule outside any @media with this exact selector that declares the property. */
export function ruleDeclaring(selector: string, property: string): Rule | null {
  const matching = rules.filter((rule) => rule.media === null && selectorsOf(rule).includes(selector) && declared(rule, property));
  return matching[matching.length - 1] ?? null;
}

const tokensIn = (selector: string) => Object.fromEntries(
  [...rules.find((rule) => rule.selector === selector)!.body.matchAll(/--([\w-]+):\s*([^;]+);/g)]
    .map(([, name, value]) => [name, value.trim()]),
);

/** Dark is the default on :root; light overrides it. */
export function tokens(theme: Theme): Record<string, string> {
  const dark = tokensIn(":root");
  return theme === "dark" ? dark : { ...dark, ...tokensIn(':root[data-theme="light"]') };
}

export function parseColor(value: string): Color {
  const hex = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(value);
  if (hex) return { r: parseInt(hex[1], 16), g: parseInt(hex[2], 16), b: parseInt(hex[3], 16), a: 1 };
  const rgb = /^rgba?\(([^)]+)\)$/.exec(value);
  if (rgb) {
    const [r, g, b, a = 1] = rgb[1].split(",").map(Number);
    return { r, g, b, a };
  }
  throw new Error(`Not a colour this check understands: ${value}`);
}

/** A translucent colour as it shows over an opaque one. */
export function over(top: Color, under: Color): Color {
  const mix = (a: number, b: number) => a * top.a + b * (1 - top.a);
  return { r: mix(top.r, under.r), g: mix(top.g, under.g), b: mix(top.b, under.b), a: 1 };
}

/** `color-mix(in srgb, A n%, B)` for opaque colours. */
export function mixed(a: Color, percent: number, b: Color): Color {
  return over({ ...a, a: percent / 100 }, b);
}

/** CSS `brightness()` multiplies in sRGB, clamped. */
export function brightened(color: Color, factor: number): Color {
  const lift = (channel: number) => Math.min(255, channel * factor);
  return { r: lift(color.r), g: lift(color.g), b: lift(color.b), a: color.a };
}

function luminance({ r, g, b }: Color): number {
  const [lr, lg, lb] = [r, g, b].map((channel) => {
    const c = channel / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
}

/** The WCAG 2 contrast ratio of two opaque colours. */
export function contrast(a: Color, b: Color): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
