import {
  $isLineBreakNode,
  $isParagraphNode,
  $isTextNode,
  createEditor,
  type Klass,
  type LexicalNode,
  type TextNode,
} from "lexical";
import { $isQuoteNode, HeadingNode, QuoteNode } from "@lexical/rich-text";
import { $isListItemNode, ListItemNode, ListNode } from "@lexical/list";
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
  type TextMatchTransformer,
  type Transformer,
} from "@lexical/markdown";

/** Lexical's link, except that an escaped `\[` opens no link. */
const NOTE_LINK: TextMatchTransformer = {
  ...LINK,
  importRegExp: new RegExp(String.raw`(?<=(?:^|[^\\])(?:\\\\)*)` + LINK.importRegExp!.source),
};

/** Markers that start a heading, quote, list or rule at the start of a line. */
const LINE_START = /^(\s*)(#(?=#{0,5}(?:\s|$))|>|[-+](?=\s|$)|-(?=[ \t]*-[ \t]*-[- \t]*$))/;
const ORDERED_START = /^(\s*\d{1,9})([.)])(?=\s|$)/;

function startsLine(node: TextNode): boolean {
  const before = node.getPreviousSibling();
  if (before) return $isLineBreakNode(before);
  const parent = node.getParent();
  return $isParagraphNode(parent) || $isQuoteNode(parent) || $isListItemNode(parent);
}

/**
 * Lexical saves only the escapes for `*`, `_`, backticks, `~` and `\`, so
 * literal text such as `# not a heading` or `[ ] not a task` would come back
 * as formatting on the next open. Brackets and line-start markers in plain
 * text keep a backslash too, so text stays text.
 */
const LITERAL_TEXT: TextMatchTransformer = {
  type: "text-match",
  dependencies: [],
  regExp: /$^/,
  export: (node, _exportChildren, exportFormat) => {
    if (!$isTextNode(node) || node.hasFormat("code")) return null;
    const text = exportFormat(node, node.getTextContent()).replace(/[[\]]/g, "\\$&");
    if (!startsLine(node)) return text;
    return text.replace(LINE_START, "$1\\$2").replace(ORDERED_START, "$1\\$2");
  },
};

/**
 * The ADR-0003 subset plus the common CommonMark blocks and marks (owner
 * decision 2026-10-22), so Markdown written elsewhere keeps its meaning.
 * Order matters: a checklist is tried before a bullet, and the star forms
 * come first so they are the canonical export. Literal text keeps the
 * escapes that stop it reading as Markdown.
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
  NOTE_LINK,
  LITERAL_TEXT,
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
