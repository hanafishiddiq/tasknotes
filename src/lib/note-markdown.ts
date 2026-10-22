import { createEditor, type Klass, type LexicalNode } from "lexical";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListItemNode, ListNode } from "@lexical/list";
import { LinkNode } from "@lexical/link";
import { CodeNode } from "@lexical/code";
import {
  $convertFromMarkdownString,
  $convertToMarkdownString,
  BOLD_ITALIC_STAR,
  BOLD_ITALIC_UNDERSCORE,
  BOLD_STAR,
  BOLD_UNDERSCORE,
  CHECK_LIST,
  CODE,
  HEADING,
  INLINE_CODE,
  ITALIC_STAR,
  ITALIC_UNDERSCORE,
  LINK,
  ORDERED_LIST,
  QUOTE,
  STRIKETHROUGH,
  UNORDERED_LIST,
  type Transformer,
} from "@lexical/markdown";

/**
 * The ADR-0003 subset plus the common CommonMark blocks and marks (owner
 * decision 2026-10-22), so Markdown written elsewhere keeps its meaning.
 * Order matters: a checklist is tried before a bullet, and the star forms
 * come first so they are the canonical export.
 */
export const NOTE_TRANSFORMERS: Transformer[] = [
  HEADING,
  QUOTE,
  CODE,
  CHECK_LIST,
  UNORDERED_LIST,
  ORDERED_LIST,
  INLINE_CODE,
  BOLD_ITALIC_STAR,
  BOLD_ITALIC_UNDERSCORE,
  BOLD_STAR,
  BOLD_UNDERSCORE,
  STRIKETHROUGH,
  ITALIC_STAR,
  ITALIC_UNDERSCORE,
  LINK,
];

export const NOTE_NODES: Klass<LexicalNode>[] = [
  HeadingNode,
  QuoteNode,
  ListNode,
  ListItemNode,
  LinkNode,
  CodeNode,
];

/** Import Markdown into the editor model and export it again. */
export function roundTripNoteMarkdown(markdown: string): string {
  const editor = createEditor({
    namespace: "note-round-trip",
    nodes: NOTE_NODES,
    onError: (error) => {
      throw error;
    },
  });
  editor.update(
    () => $convertFromMarkdownString(markdown, NOTE_TRANSFORMERS),
    { discrete: true },
  );
  return editor.getEditorState().read(() => $convertToMarkdownString(NOTE_TRANSFORMERS));
}
