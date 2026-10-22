import { useEffect, useImperativeHandle, type Ref } from "react";
import { $getNodeByKey, $getSelection, $isRangeSelection, $isTextNode } from "lexical";
import { $createListNode, $isListItemNode, $isListNode } from "@lexical/list";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { CheckListPlugin } from "@lexical/react/LexicalCheckListPlugin";
import { LinkPlugin } from "@lexical/react/LexicalLinkPlugin";
import { MarkdownShortcutPlugin } from "@lexical/react/LexicalMarkdownShortcutPlugin";
import { OnChangePlugin } from "@lexical/react/LexicalOnChangePlugin";
import { $convertFromMarkdownString, $convertToMarkdownString } from "@lexical/markdown";
import { NOTE_NODES, NOTE_TRANSFORMERS } from "../lib/note-markdown";

export interface NoteEditorHandle {
  /** Canonical Markdown of what is on screen, exported only when asked. */
  markdown(): string;
}

interface NoteEditorProps {
  initialMarkdown: string;
  /** Called after every content change; export stays lazy to keep typing light. */
  onEdit: () => void;
  editorRef: Ref<NoteEditorHandle>;
}

const THEME = {
  paragraph: "note-paragraph",
  heading: { h1: "note-h1", h2: "note-h2", h3: "note-h3", h4: "note-h4", h5: "note-h4", h6: "note-h4" },
  text: {
    bold: "note-bold",
    italic: "note-italic",
    strikethrough: "note-strike",
    code: "note-code-inline",
  },
  quote: "note-quote",
  code: "note-code",
  // Distinct from the Note list pane's classes: a shared name once hid the bullets.
  list: {
    ul: "note-ul",
    ol: "note-ol",
    listitem: "note-li",
    listitemChecked: "note-check note-check--done",
    listitemUnchecked: "note-check",
    nested: { listitem: "note-li-nested" },
  },
  link: "note-link",
};

const CHECKBOX_MARKER = /^\[( |x|X)?\] $/;

/**
 * Markdown shortcuts only run in top-level blocks, so after "- " has made a
 * bullet, a typed "[ ] " stays literal text and only becomes a checkbox when the
 * Note is reopened. This finishes the shortcut inside a bullet item so typing
 * and reopening agree. It reacts to typing only, never to opening a Note.
 */
function ChecklistShortcutPlugin() {
  const [editor] = useLexicalComposerContext();
  useEffect(() => editor.registerUpdateListener(({ tags, dirtyLeaves, editorState }) => {
    if (tags.has("historic") || tags.has("collaboration") || editor.isComposing()) return;
    const typed = editorState.read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection) || !selection.isCollapsed() || selection.anchor.type !== "text") return null;
      const { key, offset } = selection.anchor;
      if (!dirtyLeaves.has(key)) return null;
      const node = $getNodeByKey(key);
      const item = node?.getParent();
      const list = item?.getParent();
      if (!$isTextNode(node) || !$isListItemNode(item) || item.getFirstChild() !== node ||
          !$isListNode(list) || list.getListType() !== "bullet") return null;
      const marker = CHECKBOX_MARKER.exec(node.getTextContent().slice(0, offset));
      return marker ? { key, length: offset, checked: marker[1]?.toLowerCase() === "x" } : null;
    });
    if (!typed) return;
    editor.update(() => {
      const node = $getNodeByKey(typed.key);
      const item = node?.getParent();
      const list = item?.getParent();
      if (!$isTextNode(node) || !$isListItemNode(item) || !$isListNode(list)) return;
      node.spliceText(0, typed.length, "", true);
      // The item and anything typed after it become a checklist; earlier bullets stay bullets.
      const later = item.getNextSiblings();
      const checklist = $createListNode("check");
      list.insertAfter(checklist);
      checklist.append(item);
      if (later.length > 0) {
        const rest = $createListNode("bullet");
        checklist.insertAfter(rest);
        rest.append(...later);
      }
      if (list.getChildrenSize() === 0) list.remove();
      item.setChecked(typed.checked);
    });
  }), [editor]);
  return null;
}

function MarkdownHandle({ editorRef }: { editorRef: Ref<NoteEditorHandle> }) {
  const [editor] = useLexicalComposerContext();
  useImperativeHandle(editorRef, () => ({
    markdown: () => editor.getEditorState().read(() => $convertToMarkdownString(NOTE_TRANSFORMERS)),
  }), [editor]);
  return null;
}

/** Rich-text editing over canonical Markdown (ADR-0003). Remount with a new key per Note. */
export default function NoteEditor({ initialMarkdown, onEdit, editorRef }: NoteEditorProps) {
  return (
    <LexicalComposer initialConfig={{
      namespace: "tasknotes-note",
      nodes: NOTE_NODES,
      theme: THEME,
      editorState: () => $convertFromMarkdownString(initialMarkdown, NOTE_TRANSFORMERS),
      onError: (error) => { throw error; },
    }}>
      <div className="note-editor"
        // Links are kept, never followed: the webview must not leave the app.
        onClickCapture={(event) => {
          if ((event.target as HTMLElement).closest("a")) event.preventDefault();
        }}>
        <div className="note-sheet">
          <RichTextPlugin
            contentEditable={<ContentEditable className="note-content" aria-label="Note text" />}
            placeholder={<div className="note-placeholder">Start writing</div>}
            ErrorBoundary={LexicalErrorBoundary}
          />
        </div>
      </div>
      <HistoryPlugin />
      <ListPlugin />
      <CheckListPlugin />
      <LinkPlugin />
      <MarkdownShortcutPlugin transformers={NOTE_TRANSFORMERS} />
      <ChecklistShortcutPlugin />
      <OnChangePlugin onChange={() => onEdit()} />
      <MarkdownHandle editorRef={editorRef} />
    </LexicalComposer>
  );
}
