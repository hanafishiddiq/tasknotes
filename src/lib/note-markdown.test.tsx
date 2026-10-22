import { describe, expect, it } from "vitest";
import { roundTripNoteMarkdown } from "./note-markdown";

// Canonical Markdown for the supported subset (ADR-0003). Each case must come back
// byte-for-byte after passing through the rich-text editor model.
const CANONICAL: Record<string, string> = {
  headings: "# Plan\n\n## Phase one\n\n### Details",
  emphasis: "Ship **bold** and *italic* text.",
  bullets: "- Draft schema\n- Write tests\n- Review",
  checklist: "- [ ] Write migration\n- [x] Agree table shape",
  link: "Read [the ADR](https://example.com/adr-0003) first.",
  mixed:
    "# Release notes\n\nSummary with **bold**.\n\n- [x] Schema v5\n- [ ] Editor\n\nSee [docs](https://example.com/docs).",
};

describe("canonical Note Markdown round-trip", () => {
  for (const [name, markdown] of Object.entries(CANONICAL)) {
    it(`keeps ${name} unchanged`, () => {
      expect(roundTripNoteMarkdown(markdown)).toBe(markdown);
    });
  }

  it("keeps an empty Note empty", () => {
    expect(roundTripNoteMarkdown("")).toBe("");
  });
});
