import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import {
  $getNodeByKey, $getRoot, $getSelection, $isLineBreakNode, $isParagraphNode, $isRangeSelection, $isTextNode,
  COMMAND_PRIORITY_HIGH, DRAGOVER_COMMAND, DROP_COMMAND, KEY_ENTER_COMMAND, PASTE_COMMAND, createCommand,
} from "lexical";
import { ImagePlus } from "lucide-react";
import { $createListNode, $isListItemNode, $isListNode } from "@lexical/list";
import { $isHeadingNode } from "@lexical/rich-text";
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
import { $convertFromMarkdownString, $convertToMarkdownString, type Transformer } from "@lexical/markdown";
import { NOTE_NODES, NOTE_TRANSFORMERS } from "../lib/note-markdown";
import { $createTaskImageNode, ImageSourceContext, type ImageSource } from "../lib/task-image-node";
import { TASK_NODES, TASK_TRANSFORMERS } from "../lib/task-notes-markdown";
import { IMAGE_TYPES } from "../lib/task-image-api";

export interface NoteEditorHandle {
  /** Canonical Markdown of what is on screen, exported only when asked. */
  markdown(): string;
  /** Whether the person's cursor is in this editor, in the window they are using. */
  holdsCursor(): boolean;
}

interface NoteEditorProps {
  initialMarkdown: string;
  /** Called after each change to the text, not to the cursor; export stays lazy to keep typing light. */
  onEdit: () => void;
  editorRef: Ref<NoteEditorHandle>;
  /** The text box's name for screen readers; a Note's own by default. */
  label?: string;
  placeholder?: string;
  /** Extra class on the sheet: a Task's notes sit in the Task page, not a Note's column. */
  variant?: "note" | "task";
  /** Images pasted, dropped or chosen go here, and are drawn from here; without it there are none. */
  images?: ImageSource;
  /** False while the text may not change (a save on its way). */
  editable?: boolean;
  /** How many images are still on their way to the core: saving waits for them. */
  onImagesPending?: (count: number) => void;
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

/** The class of the block the core names the Note by. */
const TITLE_CLASS = "note-title";
/** The class of the text before a line break, when the title shares its block with more lines. */
const TITLE_TEXT_CLASS = "note-title-text";

/**
 * Marks the Note's title line so it looks like one: the first line with any
 * text, as the core takes the Note's name from its first non-empty line
 * (owner, 2026-11-18). Only a paragraph or heading takes the look; a first
 * list, quote or code block keeps its own. When that block holds more lines
 * (a single line break, Shift+Return), only the text before the first break
 * is the title. The classes are on the screen only, never in the Markdown.
 */
function TitleLinePlugin() {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    let marked: HTMLElement[] = [];
    const mark = () => {
      const marks = editor.getEditorState().read(() => {
        const first = $getRoot().getChildren().find((block) => block.getTextContent().trim() !== "");
        if (!first || !($isParagraphNode(first) || $isHeadingNode(first))) return [];
        const children = first.getChildren();
        const lineBreak = children.findIndex($isLineBreakNode);
        if (lineBreak < 0) return [{ key: first.getKey(), className: TITLE_CLASS }];
        return children.slice(0, lineBreak).map((child) => ({ key: child.getKey(), className: TITLE_TEXT_CLASS }));
      });
      for (const element of marked) element.classList.remove(TITLE_CLASS, TITLE_TEXT_CLASS);
      marked = [];
      for (const { key, className } of marks) {
        const element = editor.getElementByKey(key);
        if (!element) continue;
        element.classList.add(className);
        marked.push(element);
      }
    };
    mark();
    return editor.registerUpdateListener(mark);
  }, [editor]);
  return null;
}

function MarkdownHandle({ editorRef, transformers }: {
  editorRef: Ref<NoteEditorHandle>;
  transformers: Transformer[];
}) {
  const [editor] = useLexicalComposerContext();
  useImperativeHandle(editorRef, () => ({
    markdown: () => editor.getEditorState().read(() => $convertToMarkdownString(transformers)),
    holdsCursor: () => document.hasFocus() && Boolean(editor.getRootElement()?.contains(document.activeElement)),
  }), [editor, transformers]);
  return null;
}


/**
 * ⌘↵ (Ctrl+Return) saves where the editor sits in a form: it must not also
 * break the line. The key still reaches the form.
 */
function SaveShortcutPlugin() {
  const [editor] = useLexicalComposerContext();
  useEffect(() => editor.registerCommand(KEY_ENTER_COMMAND, (event) => {
    if (!event || !(event.metaKey || event.ctrlKey)) return false;
    event.preventDefault();
    return true;
  }, COMMAND_PRIORITY_HIGH), [editor]);
  return null;
}

/** Puts chosen image files where the cursor is, or at the end. */
const INSERT_IMAGES_COMMAND = createCommand<File[]>("INSERT_IMAGES_COMMAND");

/** "Add image": the same as a paste, for whoever does not know to paste. */
function AddImageButton() {
  const [editor] = useLexicalComposerContext();
  const picker = useRef<HTMLInputElement>(null);
  return (
    <div className="note-image-actions">
      <button type="button" className="text-link note-add-image" onClick={() => picker.current?.click()}>
        <ImagePlus size={13} aria-hidden="true" /> Add image
      </button>
      <input ref={picker} type="file" accept={IMAGE_TYPES.join(",")} multiple hidden aria-label="Choose images"
        onChange={(event) => {
          const files = [...(event.target.files ?? [])];
          event.target.value = "";
          if (files.length > 0) editor.dispatchCommand(INSERT_IMAGES_COMMAND, files);
        }} />
    </div>
  );
}

/** The image files a paste or drop carries. */
function imageFiles(data: DataTransfer | null): File[] {
  return [...(data?.files ?? [])].filter((file) => IMAGE_TYPES.includes(file.type));
}

/**
 * Images come in by paste or drop: each is kept by the core first, then put
 * where the cursor is, on a line of its own. A refusal (too large, not an
 * image) is said under the text.
 */
function ImagePlugin({ source, onError, onPending }: {
  source: ImageSource;
  onError: (message: string | null) => void;
  onPending?: (count: number) => void;
}) {
  const [editor] = useLexicalComposerContext();
  const pending = useRef(0);
  useEffect(() => {
    const insert = async (files: File[]) => {
      onError(null);
      pending.current += files.length;
      onPending?.(pending.current);
      for (const file of files) {
        try {
          const id = await source.add(file);
          editor.update(() => {
            const image = $createTaskImageNode(id);
            const selection = $getSelection();
            if ($isRangeSelection(selection)) selection.insertNodes([image]);
            else $getRoot().selectEnd().insertNodes([image]);
          });
        } catch (reason) {
          onError("Couldn't add the image: " + (reason instanceof Error ? reason.message : String(reason)));
        } finally {
          pending.current -= 1;
          onPending?.(pending.current);
        }
      }
    };
    const take = (data: DataTransfer | null, event: Event) => {
      const files = imageFiles(data);
      if (files.length === 0) return false;
      event.preventDefault();
      void insert(files);
      return true;
    };
    const stopPaste = editor.registerCommand(PASTE_COMMAND,
      (event) => take("clipboardData" in event ? event.clipboardData : null, event), COMMAND_PRIORITY_HIGH);
    const stopDrop = editor.registerCommand(DROP_COMMAND,
      (event) => take(event.dataTransfer, event), COMMAND_PRIORITY_HIGH);
    const stopOver = editor.registerCommand(DRAGOVER_COMMAND, (event) => {
      if (![...(event.dataTransfer?.items ?? [])].some((item) => item.kind === "file")) return false;
      event.preventDefault();
      return true;
    }, COMMAND_PRIORITY_HIGH);
    const stopInsert = editor.registerCommand(INSERT_IMAGES_COMMAND, (files) => {
      void insert(files.filter((file) => IMAGE_TYPES.includes(file.type)));
      return true;
    }, COMMAND_PRIORITY_HIGH);
    return () => { stopPaste(); stopDrop(); stopOver(); stopInsert(); };
  }, [editor, source, onError, onPending]);
  return null;
}

function EditablePlugin({ editable }: { editable: boolean }) {
  const [editor] = useLexicalComposerContext();
  useEffect(() => editor.setEditable(editable), [editor, editable]);
  return null;
}

/** Rich-text editing over canonical Markdown (ADR-0003). Remount with a new key per Note. */
export default function NoteEditor({
  initialMarkdown, onEdit, editorRef, label = "Note text", placeholder = "Start writing", variant = "note", images,
  editable = true, onImagesPending,
}: NoteEditorProps) {
  const transformers = images ? TASK_TRANSFORMERS : NOTE_TRANSFORMERS;
  const [imageError, setImageError] = useState<string | null>(null);
  return (
    <LexicalComposer initialConfig={{
      namespace: variant === "task" ? "tasknotes-task-notes" : "tasknotes-note",
      nodes: images ? TASK_NODES : NOTE_NODES,
      theme: THEME,
      editable,
      editorState: () => $convertFromMarkdownString(initialMarkdown, transformers),
      onError: (error) => { throw error; },
    }}>
      <ImageSourceContext.Provider value={images ?? null}>
        <div className={`note-editor${variant === "task" ? " note-editor--task" : ""}`}
          // Links are kept, never followed: the webview must not leave the app.
          onClickCapture={(event) => {
            if ((event.target as HTMLElement).closest("a")) event.preventDefault();
          }}>
          <div className="note-sheet">
            <RichTextPlugin
              contentEditable={<ContentEditable className="note-content" aria-label={label} />}
              placeholder={<div className="note-placeholder">{placeholder}</div>}
              ErrorBoundary={LexicalErrorBoundary}
            />
          </div>
          {images && <AddImageButton />}
          {imageError && <p role="alert" className="note-image-error">{imageError}</p>}
        </div>
      </ImageSourceContext.Provider>
      <HistoryPlugin />
      <ListPlugin />
      <CheckListPlugin />
      <LinkPlugin />
      <MarkdownShortcutPlugin transformers={transformers} />
      <ChecklistShortcutPlugin />
      <OnChangePlugin ignoreSelectionChange onChange={() => onEdit()} />
      {variant === "note" && <TitleLinePlugin />}
      <SaveShortcutPlugin />
      <EditablePlugin editable={editable} />
      {images && <ImagePlugin source={images} onError={setImageError} onPending={onImagesPending} />}
      <MarkdownHandle editorRef={editorRef} transformers={transformers} />
    </LexicalComposer>
  );
}
