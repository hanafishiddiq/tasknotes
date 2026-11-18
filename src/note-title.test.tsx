import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import NoteEditor, { type NoteEditorHandle } from "./components/NoteEditor";
import { declared, rules, selectorsOf } from "./test/stylesheet";

/**
 * A Note's first line is its title (the core names the Note by it), so the
 * editor shows it as one: bold and larger, like the Note's own heading
 * (owner, 2026-11-18, batch 4). Only the look changes: the Markdown stays as
 * written. A first line that is a list, quote or code keeps its own look.
 */
afterEach(cleanup);

function open(markdown: string) {
  const handle = createRef<NoteEditorHandle>();
  render(<NoteEditor initialMarkdown={markdown} onEdit={() => {}} editorRef={handle} />);
  return { text: screen.getByRole("textbox", { name: "Note text" }), handle };
}
/** The text shown as the title: a whole block, or the text before its first line break. */
const titles = (text: HTMLElement) => {
  const block = [...text.querySelectorAll(".note-title")].map((each) => each.textContent);
  const parts = [...text.querySelectorAll(".note-title-text")].map((each) => each.textContent).join("");
  return parts ? [...block, parts] : block;
};

describe("the first line of a Note", () => {
  it("shows as the title, and only that line", async () => {
    const { text, handle } = open("Meeting with supplier. Discuss price\n\nBring the samples");
    await waitFor(() => expect(titles(text)).toEqual(["Meeting with supplier. Discuss price"]));
    // The Markdown is what was written: no heading marker is added.
    expect(handle.current!.markdown()).toBe("Meeting with supplier. Discuss price\n\nBring the samples");
  });

  it("is only the first line when more lines share its paragraph", async () => {
    const { text, handle } = open("Title **here**\nmore details on the next line");
    await waitFor(() => expect(titles(text)).toEqual(["Title here"]));
    expect(text.querySelector(".note-title")).toBeNull();
    expect(handle.current!.markdown()).toBe("Title **here**\nmore details on the next line");
  });

  it("skips empty lines, as the Note's name does", async () => {
    const { text } = open("\n\n\nGroceries\n\nMilk");
    await waitFor(() => expect(titles(text)).toEqual(["Groceries"]));
  });

  it("keeps a heading as the title, whatever its level", async () => {
    const { text } = open("## Plans\n\nBody");
    await waitFor(() => expect(titles(text)).toEqual(["Plans"]));
  });

  it("leaves a first line that is a list, quote or code as it is", async () => {
    for (const markdown of ["- Milk\n- Eggs\n\nAfter", "> Quoted\n\nAfter", "```\ncode\n```\n\nAfter"]) {
      const { text } = open(markdown);
      await act(async () => {});
      expect(titles(text), markdown).toEqual([]);
      cleanup();
    }
  });

  it("follows the typing: the line typed first becomes the title", async () => {
    const user = userEvent.setup();
    const { text } = open("");
    await user.click(text);
    await user.keyboard("Trip ideas{Enter}Beach");
    await waitFor(() => expect(titles(text)).toEqual(["Trip ideas"]));
  });

  it("is bold and as large as a Note's own heading", () => {
    const rule = rules.find((each) => each.media === null && selectorsOf(each).includes(".note-content .note-title"));
    const h1 = rules.find((each) => each.media === null && selectorsOf(each).includes(".note-h1") && declared(each, "font-size") !== null);
    expect(rule && selectorsOf(rule)).toContain(".note-content .note-title-text");
    expect(rule && declared(rule, "font-weight")).toBe("700");
    expect(rule && declared(rule, "font-size")).toBe(h1 && declared(h1, "font-size"));
  });
});
