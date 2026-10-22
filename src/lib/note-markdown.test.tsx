import { describe, expect, it } from "vitest";
import { roundTripNoteMarkdown } from "./note-markdown";

// Canonical Markdown comes back byte-for-byte after passing through the editor
// model. Byte equality alone does not prove a format is recognised (a checklist
// also survives as a bullet with "[ ]" text); the App tests check the rendering.
const CANONICAL: Record<string, string> = {
  headings: "# Plan\n\n## Phase one\n\n### Details",
  emphasis: "Ship **bold** and *italic* text.",
  bullets: "- Draft schema\n- Write tests\n- Review",
  nestedBullets: "- Parent\n    - Child",
  checklist: "- [ ] Write migration\n- [x] Agree table shape",
  link: "Read [the ADR](https://example.com/adr-0003) first.",
  ordered: "1. First\n2. Second",
  quote: "> Quoted line",
  codeBlock: "```\nconst ready = true;\n```",
  codeBlockLanguage: "```ts\nconst ready = true;\n```",
  inlineCode: "Run `pnpm test` first.",
  strikethrough: "~~Old plan~~ replaced.",
  escapedLiterals: "\\*not italic\\* and snake\\_case",
  // Text that only looks like Markdown keeps its escape, so it stays text.
  escapedHeading: "\\# Not a heading",
  escapedBullets: "\\- Not a bullet\n\n\\+ Not either",
  escapedOrdered: "1\\. Not a list\n\n2\\) Not either",
  escapedQuote: "\\> Not a quote",
  escapedRule: "\\---",
  escapedCheckbox: "\\[ \\] Not a task",
  escapedCheckboxInBullet: "- \\[ \\] Literal brackets",
  escapedLink: "\\[draft\\](later) is not a link",
  escapedAfterLineBreak: "First line\n\\# second line",
  escapedIndented: "   \\- indented",
  escapedInBullet: "- \\- inner",
  escapedInQuote: "> \\# inner",
  // ...but only where it would otherwise read as Markdown.
  plainLineStarts: "#tag\n\n-5 degrees\n\n1.5 litres",
  bracketsInInlineCode: "Type `- [ ]` for a task.",
  mixed:
    "# Release notes\n\nSummary with **bold**.\n\n- [x] Schema v5\n- [ ] Editor\n\nSee [docs](https://example.com/docs).",
  // Not edited as rich text, but kept verbatim.
  table: "| a | b |\n|---|---|\n| 1 | 2 |",
  image: "![Diagram](https://example.com/diagram.png)",
  html: "<kbd>Cmd</kbd>",
};

// Equivalent Markdown is normalised to one canonical spelling on save.
const NORMALISED: Record<string, [string, string]> = {
  underscoreEmphasis: ["_under_ and __strong__", "*under* and **strong**"],
  checkedUppercase: ["- [X] Done", "- [x] Done"],
  extraBlankLines: ["one\n\n\n\ntwo", "one\n\ntwo"],
  twoSpaceNesting: ["- Parent\n  - Child", "- Parent\n    - Child"],
  trailingNewline: ["text\n", "text"],
  literalUnderscores: ["snake_case_name", "snake\\_case\\_name"],
  literalBrackets: ["a [b] c and [a]: note", "a \\[b\\] c and \\[a\\]: note"],
  bracketsInLinkText: ["[a [b] c](https://example.com)", "[a \\[b\\] c](https://example.com)"],
};

describe("canonical Note Markdown round-trip", () => {
  for (const [name, markdown] of Object.entries(CANONICAL)) {
    it(`keeps ${name} unchanged`, () => {
      expect(roundTripNoteMarkdown(markdown)).toBe(markdown);
    });
  }

  for (const [name, [input, canonical]] of Object.entries(NORMALISED)) {
    it(`normalises ${name} once and then keeps it`, () => {
      expect(roundTripNoteMarkdown(input)).toBe(canonical);
      expect(roundTripNoteMarkdown(canonical)).toBe(canonical);
    });
  }

  it("keeps an empty Note empty", () => {
    expect(roundTripNoteMarkdown("")).toBe("");
  });
});
