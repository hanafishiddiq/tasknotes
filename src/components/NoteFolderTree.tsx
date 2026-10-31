import { useEffect, useRef, useState, type CSSProperties, type FormEvent, type KeyboardEvent } from "react";
import {
  Check, ChevronDown, ChevronRight, Folder, FolderInput, FolderPlus, Pencil, Plus, Trash2, X,
} from "lucide-react";
import type { NoteFolder } from "../lib/note-api";
import type { NoteFolders } from "../lib/use-note-folders";
import { alreadyThere, anchorFor, useRowDrag, type RowDrop } from "../lib/use-row-drag";

/** Which inline control is open on a folder row, if any. */
type Editing = { kind: "create" | "rename" | "move"; id: string } | null;

const TOP_LEVEL = "";

interface NoteFolderTreeProps {
  folders: NoteFolders;
  /** The folder whose Notes are showing, or null. */
  currentId: string | null;
  disabled: boolean;
  onOpen: (folder: NoteFolder) => void;
  /** Called after a folder, with everything in it, was moved to Removed. */
  onRemoved: (folder: NoteFolder) => void;
}

/** The Note Folder tree in the sidebar, one bounded page of children per open folder. */
export default function NoteFolderTree({ folders, currentId, disabled, onOpen, onRemoved }: NoteFolderTreeProps) {
  const [editing, setEditing] = useState<Editing>(null);
  const [draft, setDraft] = useState("");
  const [newName, setNewName] = useState("");
  const busy = disabled || folders.busy;
  const tree = useRef<HTMLUListElement>(null);
  // The folder whose inline form just closed, taking its focused field along: its row gets focus instead.
  const closedOn = useRef<string | null>(null);

  useEffect(() => {
    if (editing || !closedOn.current) return;
    tree.current?.querySelector<HTMLElement>(`[data-folder-id="${CSS.escape(closedOn.current)}"]`)?.focus();
    closedOn.current = null;
  }, [editing]);

  const [moved, setMoved] = useState("");
  // The folder a keyboard move carries: its row is rebuilt, which would drop focus.
  const refocus = useRef<string | null>(null);

  useEffect(() => {
    if (!refocus.current) return;
    tree.current?.querySelector<HTMLElement>(`[data-folder-id="${CSS.escape(refocus.current)}"]`)?.focus();
    refocus.current = null;
  }, [folders.byId]);

  const siblingIds = (parentId: string | null) => folders.level(parentId)?.items.map((item) => item.id) ?? [];

  /** Puts the folder there, and if the core refuses, shows what is true now. */
  async function placeFolder(folder: NoteFolder, parentId: string | null, afterId: string | null) {
    if (busy) return;
    if (parentId === folder.parentId && alreadyThere(siblingIds(parentId), folder.id, afterId)) return;
    const after = afterId && folders.byId.get(afterId)?.name;
    const parent = parentId && folders.byId.get(parentId)?.name;
    const where = after ? `after ${after}` : parent ? `to the top of ${parent}` : "to the top";
    if (await folders.place(folder, parentId, afterId)) setMoved(`${folder.name} moved ${where}`);
    else {
      refocus.current = null;
      void folders.refresh();
    }
  }

  async function moveInto(folder: NoteFolder, parent: NoteFolder) {
    if (busy) return;
    if (folder.parentId === parent.id) {
      const others = siblingIds(parent.id).filter((id) => id !== folder.id);
      return placeFolder(folder, parent.id, others[others.length - 1] ?? null);
    }
    if (await folders.move(folder, parent.id)) setMoved(`${folder.name} moved into ${parent.name}`);
    else {
      refocus.current = null;
      void folders.refresh();
    }
  }

  /** Whether `id` is the dragged folder or sits below it: nothing can land inside itself. */
  function insideDragged(dragged: string, id: string) {
    for (let at: string | null | undefined = id; at; at = folders.byId.get(at)?.parentId) {
      if (at === dragged) return true;
    }
    return false;
  }

  const drag = useRowDrag({
    container: tree,
    canNest: true,
    canTarget: (dragged, id) => !insideDragged(dragged, id),
    onDrop(dragged: string, drop: RowDrop) {
      const folder = folders.byId.get(dragged);
      if (!folder) return;
      if (drop.id === null) {
        const top = siblingIds(null).filter((id) => id !== dragged);
        void placeFolder(folder, null, top[top.length - 1] ?? null);
        return;
      }
      const target = folders.byId.get(drop.id);
      if (!target) return;
      if (drop.edge === "into") void moveInto(folder, target);
      // Just below an open folder's row is its first subfolder's place.
      else if (drop.edge === "after" && target.hasChildren && folders.isExpanded(target.id)) void placeFolder(folder, target.id, null);
      else void placeFolder(folder, target.parentId, anchorFor(siblingIds(target.parentId), dragged, target.id, drop.edge));
    },
  });

  /**
   * Alt with the arrows moves the folder itself: Up and Down among its
   * siblings, Right into the folder above it, Left out to just after its parent.
   */
  function moveWithKeys(event: KeyboardEvent, folder: NoteFolder) {
    event.preventDefault();
    const ids = siblingIds(folder.parentId);
    const at = ids.indexOf(folder.id);
    refocus.current = folder.id;
    if (event.key === "ArrowUp" && at > 0) void placeFolder(folder, folder.parentId, ids[at - 2] ?? null);
    else if (event.key === "ArrowDown" && at < ids.length - 1) void placeFolder(folder, folder.parentId, ids[at + 1]);
    else if (event.key === "ArrowRight" && at > 0) void moveInto(folder, folders.byId.get(ids[at - 1])!);
    else if (event.key === "ArrowLeft" && folder.parentId) {
      void placeFolder(folder, folders.byId.get(folder.parentId)?.parentId ?? null, folder.parentId);
    } else refocus.current = null;
  }

  function closeForm() {
    closedOn.current = editing?.id ?? null;
    setEditing(null);
  }

  function start(kind: "create" | "rename" | "move", folder: NoteFolder) {
    setEditing({ kind, id: folder.id });
    setDraft(kind === "rename" ? folder.name : "");
  }

  async function submit(event: FormEvent<HTMLFormElement>, folder: NoteFolder) {
    event.preventDefault();
    if (busy || !draft.trim() || !editing) return;
    const saved = editing.kind === "create"
      ? await folders.create(draft, folder.id)
      : await folders.rename(folder, draft);
    if (saved) closeForm();
  }

  async function moveTo(folder: NoteFolder, destination: string) {
    if (await folders.move(folder, destination === TOP_LEVEL ? null : destination)) closeForm();
  }

  async function createTopLevel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || !newName.trim()) return;
    if (await folders.create(newName, null)) setNewName("");
  }

  /**
   * Arrow keys as in a tree: Up and Down move between the folders shown,
   * Right opens a closed folder or steps into an open one, Left closes an
   * open folder or steps out to its parent.
   */
  function onKeyDown(event: KeyboardEvent<HTMLUListElement>) {
    const target = event.target as HTMLElement;
    const id = target.dataset.folderId;
    const folder = id ? folders.byId.get(id) : undefined;
    if (!folder || !["ArrowDown", "ArrowUp", "ArrowRight", "ArrowLeft"].includes(event.key)) return;
    if (event.altKey) {
      moveWithKeys(event, folder);
      return;
    }
    event.preventDefault();
    const shown = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("button[data-folder-id]")];
    const at = shown.indexOf(target as HTMLButtonElement);
    const open = folder.hasChildren && folders.isExpanded(folder.id);
    if (event.key === "ArrowDown") shown[at + 1]?.focus();
    else if (event.key === "ArrowUp") shown[at - 1]?.focus();
    else if (event.key === "ArrowRight") {
      // An open folder's first subfolder is the next one shown.
      if (open) shown[at + 1]?.focus();
      else if (folder.hasChildren) void folders.toggle(folder);
    } else if (open) void folders.toggle(folder);
    else shown.find((button) => button.dataset.folderId === folder.parentId)?.focus();
  }

  function rows(parentId: string | null, depth: number) {
    const level = folders.level(parentId);
    if (!level) return null;
    return (
      <>
        {level.items.map((folder) => {
          const open = folders.isExpanded(folder.id) && folder.hasChildren;
          const edit = editing?.id === folder.id ? editing.kind : null;
          const dropEdge = drag.drop?.id === folder.id ? drag.drop.edge : undefined;
          // A line just below an open folder's row belongs to its subfolders.
          const dropDepth = dropEdge === "after" && open ? depth + 1 : depth;
          return (
            <li key={folder.id}>
              <div className="folder-row" data-drag-row={folder.id}
                data-dragging={drag.dragging === folder.id || undefined} data-drop-edge={dropEdge}
                style={{ paddingLeft: depth * 14, "--drop-indent": `${dropDepth * 14}px` } as CSSProperties}>
                {folder.hasChildren ? (
                  <button type="button" className="folder-disclosure" disabled={disabled}
                    aria-label={`${open ? "Collapse" : "Expand"} ${folder.name}`} aria-expanded={open}
                    onClick={() => void folders.toggle(folder)}>
                    {open ? <ChevronDown size={13} aria-hidden="true" /> : <ChevronRight size={13} aria-hidden="true" />}
                  </button>
                ) : <span className="folder-disclosure" aria-hidden="true" />}
                {edit === "rename" ? (
                  <form className="folder-form" onSubmit={(event) => void submit(event, folder)}>
                    <input aria-label="Rename folder" value={draft} maxLength={100} disabled={busy} autoFocus
                      onChange={(event) => setDraft(event.target.value)} />
                    <button type="submit" className="folder-action" aria-label="Save folder name"
                      disabled={busy || !draft.trim()}><Check size={14} /></button>
                    <button type="button" className="folder-action" aria-label="Cancel rename"
                      onClick={closeForm}><X size={14} /></button>
                  </form>
                ) : (
                  <>
                    <button type="button" className="folder" disabled={disabled} aria-label={folder.name}
                      data-folder-id={folder.id}
                      aria-current={currentId === folder.id ? "page" : undefined}
                      aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown Alt+ArrowLeft Alt+ArrowRight"
                      onMouseDown={(event) => drag.start(event, folder.id, folder.name)}
                      onClick={() => onOpen(folder)}>
                      <Folder size={17} aria-hidden="true" /><span className="folder-name">{folder.name}</span>
                    </button>
                    <span className="folder-actions">
                      <button type="button" className="folder-action" disabled={busy}
                        aria-label={`New folder in ${folder.name}`} onClick={() => start("create", folder)}>
                        <FolderPlus size={13} />
                      </button>
                      <button type="button" className="folder-action" disabled={busy}
                        aria-label={`Rename ${folder.name}`} onClick={() => start("rename", folder)}>
                        <Pencil size={13} />
                      </button>
                      <button type="button" className="folder-action" disabled={busy}
                        aria-label={`Move ${folder.name}`} onClick={() => start("move", folder)}>
                        <FolderInput size={13} />
                      </button>
                      <button type="button" className="folder-action" disabled={busy}
                        aria-label={`Remove ${folder.name}`}
                        onClick={() => void folders.remove(folder).then((removed) => removed && onRemoved(removed))}>
                        <Trash2 size={13} />
                      </button>
                    </span>
                  </>
                )}
              </div>
              {edit === "create" && (
                <form className="folder-form folder-form--nested" style={{ marginLeft: (depth + 1) * 14 }}
                  onSubmit={(event) => void submit(event, folder)}>
                  <Plus size={16} aria-hidden="true" />
                  <input aria-label={`New folder name in ${folder.name}`} placeholder="New Folder" autoFocus
                    maxLength={100} value={draft} disabled={busy} onChange={(event) => setDraft(event.target.value)} />
                  <button type="button" className="folder-action" aria-label="Cancel new folder"
                    onClick={closeForm}><X size={14} /></button>
                </form>
              )}
              {edit === "move" && (
                <div className="folder-form folder-form--nested" style={{ marginLeft: (depth + 1) * 14 }}>
                  <FolderInput size={16} aria-hidden="true" />
                  <select aria-label={`Move ${folder.name} to`} disabled={busy} defaultValue="" autoFocus
                    onChange={(event) => { if (event.target.value !== "") void moveTo(folder, event.target.value.slice(1)); }}>
                    <option value="" disabled>Move to…</option>
                    {/* A leading marker keeps "Top level" distinct from the empty prompt. */}
                    <option value={"#" + TOP_LEVEL}>Top level</option>
                    {folders.options.map((option) => (
                      <option key={option.id} value={"#" + option.id}>{option.label}</option>
                    ))}
                  </select>
                  <button type="button" className="folder-action" aria-label="Cancel move"
                    onClick={closeForm}><X size={14} /></button>
                </div>
              )}
              {open && <ul className="folder-tree">{rows(folder.id, depth + 1)}</ul>}
            </li>
          );
        })}
        {level.cursor && (
          <li>
            <button className="sidebar-link" type="button" disabled={busy} style={{ marginLeft: depth * 14 }}
              onClick={() => void folders.loadMore(parentId)}>Load more folders</button>
          </li>
        )}
      </>
    );
  }

  return (
    <>
      {folders.error && (
        <div className="sidebar-error" role="alert">
          <span>{folders.error}</span>
          <button type="button" className="folder-action folder-action--visible" aria-label="Dismiss"
            onClick={folders.dismissError}><X size={13} /></button>
        </div>
      )}
      <span className="sr-only" aria-live="polite">{moved}</span>
      <ul ref={tree} className="folder-tree" aria-label="Folders" onKeyDown={onKeyDown}
        data-drop-end={drag.drop?.edge === "end" || undefined}>{rows(null, 0)}</ul>
      <form className="folder-form folder-form--new" onSubmit={(event) => void createTopLevel(event)}>
        <Plus size={16} aria-hidden="true" />
        <input aria-label="New folder name" placeholder="New Folder" maxLength={100}
          value={newName} disabled={busy} onChange={(event) => setNewName(event.target.value)} />
        <button type="submit" className="folder-action" aria-label="Add folder"
          disabled={busy || !newName.trim()}><Check size={14} /></button>
      </form>
    </>
  );
}
