import { createEditor } from "lexical";
import { $convertFromMarkdownString, $convertToMarkdownString } from "@lexical/markdown";
import { NOTE_NODES, NOTE_TRANSFORMERS } from "./note-markdown";
import { TASK_IMAGE, TaskImageNode, withoutImages } from "./task-image-node";

/** What a Task's notes read and write: a Note's Markdown, and images (ADR 0012). */
export const TASK_TRANSFORMERS = [TASK_IMAGE, ...NOTE_TRANSFORMERS];
export const TASK_NODES = [...NOTE_NODES, TaskImageNode];

/**
 * A Task's notes as the editor writes them back. Notes written as plain text
 * (by the old field, the CLI or MCP) come back with Markdown escapes, such as
 * `snake\_case`; a draft equal to this is no change.
 */
export function roundTripTaskNotes(markdown: string): string {
  const editor = createEditor({
    namespace: "task-notes-round-trip",
    nodes: TASK_NODES,
    onError: (error) => {
      throw error;
    },
  });
  editor.update(() => $convertFromMarkdownString(markdown, TASK_TRANSFORMERS), { discrete: true });
  return editor.getEditorState().read(() => $convertToMarkdownString(TASK_TRANSFORMERS));
}

/** A Task's notes on its collapsed row: no images, no Markdown escapes. */
export function notesPreview(markdown: string): string {
  return withoutImages(markdown).replace(/\\([\\`*_{}[\]()#+\-.!|>~])/g, "$1");
}
