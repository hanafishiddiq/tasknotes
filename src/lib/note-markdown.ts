import { createEditor, type Klass, type LexicalNode } from "lexical";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListItemNode, ListNode } from "@lexical/list";
import { LinkNode } from "@lexical/link";
import { CodeNode } from "@lexical/code";
import {
  $convertFromMarkdownString,
  $convertToMarkdownString,
  BOLD_STAR,
  CHECK_LIST,
  HEADING,
  ITALIC_STAR,
  LINK,
  UNORDERED_LIST,
  type Transformer,
} from "@lexical/markdown";

/** The supported Note formatting subset from ADR-0003. */
export const NOTE_TRANSFORMERS: Transformer[] = [
  HEADING,
  CHECK_LIST,
  UNORDERED_LIST,
  BOLD_STAR,
  ITALIC_STAR,
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
