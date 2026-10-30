/// <reference types="node" />
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import axe from "axe-core";

/**
 * Rules switched off because jsdom has no layout, paint or canvas, so their
 * answer would be noise rather than evidence. Each is checked some other way,
 * or belongs to the owner's on-screen checklist, as its reason says.
 */
export const RULES_JSDOM_CANNOT_JUDGE: Record<string, string> = {
  "color-contrast": "needs painted colours; a11y-contrast.test.tsx computes them from App.css",
  "color-contrast-enhanced": "needs painted colours, and AAA is not the target",
  "link-in-text-block": "compares painted colours of a link and the text around it; on-screen check",
  "target-size": "needs on-screen geometry; on-screen check",
  "scrollable-region-focusable": "needs scroll sizes, which are all zero; on-screen check",
  "css-orientation-lock": "reads applied media queries; a desktop window has no orientation lock",
  "p-as-heading": "compares rendered font size and weight; on-screen check",
  "label-content-name-mismatch": "measures glyphs on a canvas, which jsdom lacks; VoiceOver and Voice Control check",
};

/** Off by default in axe, and answerable without layout, so asked for here. */
const EXTRA_RULES = ["focus-order-semantics"];

/** Gives the test document what the real page has: index.html's lang, title and viewport. */
export function adoptIndexHtml() {
  const page = new DOMParser().parseFromString(
    readFileSync(resolve(__dirname, "../../index.html"), "utf8"), "text/html",
  );
  document.documentElement.lang = page.documentElement.lang;
  document.title = page.title;
  document.head.querySelector("meta[name=viewport]")?.remove();
  document.head.append(document.importNode(page.querySelector("meta[name=viewport]")!, true));
  // jsdom has no hit testing. Axe only uses it to spot an overlay by its size;
  // a real [role=dialog] is found without it. Without this, page-level rules
  // such as landmark-one-main crash and come back "incomplete", untested.
  document.elementFromPoint = () => null;
}

/** "rule (impact): target, target": short enough to read in a failed assertion. */
function summaries(results: axe.Result[]): string[] {
  return results.map((result) =>
    `${result.id} (${result.impact ?? "no impact"}): ${result.nodes.map((node) => node.target.join(" ")).join(", ")}`);
}

/**
 * Axe yields between rules, so a re-render during a run (a debounce firing, a late
 * answer) would have it judge half of each state. An answer only counts if nothing
 * changed while it was being worked out.
 */
async function axeOnStillPage() {
  for (let attempt = 0; attempt < 5; attempt++) {
    const changes = new MutationObserver(() => undefined);
    changes.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
    const results = await axe.run(document, {
      rules: {
        ...Object.fromEntries(Object.keys(RULES_JSDOM_CANNOT_JUDGE).map((rule) => [rule, { enabled: false }])),
        ...Object.fromEntries(EXTRA_RULES.map((rule) => [rule, { enabled: true }])),
      },
      resultTypes: ["violations", "incomplete"],
    });
    const changed = changes.takeRecords().length > 0;
    changes.disconnect();
    if (!changed) return results;
  }
  throw new Error("The page kept changing, so axe never saw one state of it");
}

/** Runs axe over the whole document, so portals and page-level rules are included. */
export async function runAxe() {
  const results = await axeOnStillPage();
  const crashed = results.incomplete.filter((result) =>
    result.nodes.some((node) => [...node.any, ...node.all, ...node.none].some((check) => check.id === "error-occurred")));
  return {
    violations: results.violations.map((result) => result.id),
    /** What axe could not decide. Treated as a finding until someone has looked. */
    incomplete: results.incomplete.filter((result) => !crashed.includes(result)).map((result) => result.id),
    /** Rules that threw, so they said nothing: a hole in the check, never a pass. */
    crashed: crashed.map((result) => result.id),
    details: summaries([...results.violations, ...results.incomplete]),
  };
}
