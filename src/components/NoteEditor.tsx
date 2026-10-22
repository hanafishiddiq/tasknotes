import { useImperativeHandle, type Ref } from "react";
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
  heading: { h1: "note-h1", h2: "note-h2", h3: "note-h3" },
  text: { bold: "note-bold", italic: "note-italic" },
  // Distinct from the Note list pane's classes: a shared name once hid the bullets.
  list: {
    ul: "note-ul",
    listitem: "note-li",
    listitemChecked: "note-check note-check--done",
    listitemUnchecked: "note-check",
    nested: { listitem: "note-li-nested" },
  },
  link: "note-link",
};

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
      <OnChangePlugin onChange={() => onEdit()} />
      <MarkdownHandle editorRef={editorRef} />
    </LexicalComposer>
  );
}
