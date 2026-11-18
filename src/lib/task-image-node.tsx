import { createContext, useContext, useEffect, useState, type JSX } from "react";
import {
  $applyNodeReplacement, DecoratorNode, type LexicalNode, type NodeKey, type SerializedLexicalNode, type Spread,
} from "lexical";
import type { TextMatchTransformer } from "@lexical/markdown";

/** How a Task's notes refer to an image the core keeps (batch 4). */
export const IMAGE_SCHEME = "tasknotes-image:";
const REFERENCE = String.raw`!\[[^\]\n]*\]\(tasknotes-image:([0-9A-Za-z-]+)\)`;

/** Where the editor fetches and keeps images: the core, through the window. */
export interface ImageSource {
  /** A URL the window can draw the image from. */
  url: (id: string) => Promise<string>;
  /** Keeps a pasted or dropped image; answers its id. */
  add: (file: File) => Promise<string>;
}

export const ImageSourceContext = createContext<ImageSource | null>(null);

type SerializedTaskImageNode = Spread<{ imageId: string }, SerializedLexicalNode>;

function TaskImageView({ imageId }: { imageId: string }) {
  const source = useContext(ImageSourceContext);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    source?.url(imageId)
      .then((found) => { if (active) setUrl(found); })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [source, imageId]);
  if (failed) return <span className="task-image-missing" role="img" aria-label="Image not found">Image not found</span>;
  return url
    ? <img className="task-image" src={url} alt="Image" draggable={false} />
    : <span className="task-image-loading" role="img" aria-label="Loading image" />;
}

/** An image in a Task's notes: inline in the text model, drawn on its own line. */
export class TaskImageNode extends DecoratorNode<JSX.Element> {
  __imageId: string;

  static getType(): string {
    return "task-image";
  }

  static clone(node: TaskImageNode): TaskImageNode {
    return new TaskImageNode(node.__imageId, node.__key);
  }

  static importJSON(json: SerializedTaskImageNode): TaskImageNode {
    return $createTaskImageNode(json.imageId);
  }

  constructor(imageId: string, key?: NodeKey) {
    super(key);
    this.__imageId = imageId;
  }

  exportJSON(): SerializedTaskImageNode {
    return { ...super.exportJSON(), type: "task-image", version: 1, imageId: this.__imageId };
  }

  getImageId(): string {
    return this.getLatest().__imageId;
  }

  createDOM(): HTMLElement {
    const frame = document.createElement("span");
    frame.className = "task-image-frame";
    return frame;
  }

  updateDOM(): false {
    return false;
  }

  isInline(): true {
    return true;
  }

  getTextContent(): string {
    return `![image](${IMAGE_SCHEME}${this.__imageId})`;
  }

  decorate(): JSX.Element {
    return <TaskImageView imageId={this.__imageId} />;
  }
}

export function $createTaskImageNode(imageId: string): TaskImageNode {
  return $applyNodeReplacement(new TaskImageNode(imageId));
}

export function $isTaskImageNode(node: LexicalNode | null | undefined): node is TaskImageNode {
  return node instanceof TaskImageNode;
}

/** `![image](tasknotes-image:ID)` in Markdown ⇄ an image in the editor. */
export const TASK_IMAGE: TextMatchTransformer = {
  dependencies: [TaskImageNode],
  export: (node) => ($isTaskImageNode(node) ? `![image](${IMAGE_SCHEME}${node.getImageId()})` : null),
  importRegExp: new RegExp(REFERENCE),
  regExp: new RegExp(REFERENCE + "$"),
  replace: (textNode, match) => {
    textNode.replace($createTaskImageNode(match[1]));
  },
  trigger: ")",
  type: "text-match",
};

/** A Task's notes as one line of plain text: images left out (the collapsed row). */
export function withoutImages(markdown: string): string {
  return markdown.replace(new RegExp(REFERENCE, "g"), "").replace(/\n{3,}/g, "\n\n").trim();
}
