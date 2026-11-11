import {
  Fragment, useEffect, useRef, useState, type CSSProperties, type FormEvent, type KeyboardEvent, type ReactNode,
} from "react";
import {
  Check, ChevronDown, ChevronRight, ClipboardCheck, History, Inbox, ListChecks, ListPlus, Moon, NotebookText, Palette,
  Pencil, Plus, Settings, Sun, Trash2, X,
} from "lucide-react";
import StyleIcon from "./StyleIcon";
import StylePicker from "./StylePicker";
import type { Style } from "../lib/style";
import type { TaskList, TaskView } from "../lib/task-api";
import { readExpanded, writeExpanded } from "../lib/expanded";
import { MAX_LIST_DEPTH, listTree, parentOf } from "../lib/list-tree";
import { alreadyThere, anchorFor, useRowDrag, type RowDrop } from "../lib/use-row-drag";
import type { Theme } from "../lib/use-theme";

interface TaskSidebarProps {
  view: TaskView;
  /** The Notes workspace is showing instead of a Task view. */
  notesActive: boolean;
  /** The Removed view is showing. */
  removedActive: boolean;
  onOpenRemoved: () => void;
  /** Bulk Changes waiting for the person; the entry shows while any wait. */
  pendingApprovals: number;
  approvalsActive: boolean;
  onOpenApprovals: () => void;
  settingsActive: boolean;
  onOpenSettings: () => void;
  /** The Pomodoro page (the daily history) is showing. */
  pomodoroHistoryActive: boolean;
  onOpenPomodoroHistory: () => void;
  /** Notes are showing every Note rather than one folder. */
  allNotesCurrent: boolean;
  /** The Note Folder tree, once Notes have been opened. */
  noteFolders?: ReactNode;
  /**
   * The draggable right edge. Rendered inside the sidebar so it belongs to its landmark;
   * App.css positions it against the shell, so it does not scroll with the sidebar.
   */
  resizer?: ReactNode;
  /** The Pomodoro control, at the foot above Settings. */
  pomodoro?: ReactNode;
  lists: TaskList[];
  listError: string | null;
  /** Tasks on the current page, or null when the count is not known. */
  shownCount: number | null;
  hasMoreTasks: boolean;
  hasMoreLists: boolean;
  busy: boolean;
  theme: Theme;
  onToggleTheme: () => void;
  onNavigate: (view: TaskView) => void;
  onOpenNotes: () => void;
  onCreateList: (name: string) => Promise<boolean>;
  /** Makes an empty List inside `parent`, at the bottom there; the new List, or null when refused. */
  onCreateSubList: (parent: TaskList, name: string) => Promise<TaskList | null>;
  onRenameList: (list: TaskList, name: string) => Promise<boolean>;
  /** Puts the List directly after a sibling, or first when `afterId` is null; false when refused. */
  onReorderList: (list: TaskList, afterId: string | null) => Promise<boolean>;
  /** Puts the List under another parent (null: the top level), after `afterId` there or first; false when refused. */
  onPlaceList: (list: TaskList, parentId: string | null, afterId: string | null) => Promise<boolean>;
  /** Moves the List into another, at the bottom there; false when refused. */
  onMoveList: (list: TaskList, parentId: string) => Promise<boolean>;
  /** Sets the List's icon and colour; false when refused. */
  onStyleList: (list: TaskList, style: Style) => Promise<boolean>;
  onRemoveList: (list: TaskList) => Promise<void>;
  onLoadMoreLists: () => Promise<void>;
}

export default function TaskSidebar({
  view, notesActive, removedActive, onOpenRemoved, pendingApprovals, approvalsActive, onOpenApprovals, settingsActive, onOpenSettings, pomodoroHistoryActive, onOpenPomodoroHistory, allNotesCurrent, noteFolders, resizer, pomodoro, lists, listError, shownCount, hasMoreTasks, hasMoreLists, busy,
  theme, onToggleTheme, onNavigate, onOpenNotes, onCreateList, onCreateSubList, onRenameList, onReorderList,
  onPlaceList, onMoveList,
  onStyleList, onRemoveList,
  onLoadMoreLists,
}: TaskSidebarProps) {
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [styling, setStyling] = useState<string | null>(null);
  const listsNav = useRef<HTMLElement>(null);
  // The List whose rename just closed, taking its focused field along: its row gets focus instead.
  const renamed = useRef<string | null>(null);

  useEffect(() => {
    if (renaming || !renamed.current) return;
    listsNav.current?.querySelector<HTMLElement>(`[data-list-id="${CSS.escape(renamed.current)}"]`)?.focus();
    renamed.current = null;
  }, [renaming]);

  function closeRename() {
    renamed.current = renaming;
    setRenaming(null);
  }

  // The List whose picker just closed: its row gets focus back.
  const styled = useRef<string | null>(null);
  useEffect(() => {
    if (styling || !styled.current) return;
    listsNav.current?.querySelector<HTMLElement>(`[data-list-id="${CSS.escape(styled.current)}"]`)?.focus();
    styled.current = null;
  }, [styling]);

  function closeStyle() {
    styled.current = styling;
    setStyling(null);
  }

  async function saveStyle(list: TaskList, style: Style) {
    const saved = await onStyleList(list, style);
    if (saved) closeStyle();
    return saved;
  }

  const tree = listTree(lists);
  // Lists start collapsed, as Note Folders do; the open ones are remembered.
  const [expanded, setExpanded] = useState<Set<string>>(() => readExpanded(EXPANDED_LISTS_KEY));
  const isOpen = (id: string) => expanded.has(id);
  function setOpen(id: string, open: boolean) {
    setExpanded((before) => {
      if (open === before.has(id)) return before;
      const next = new Set(before);
      if (open) next.add(id);
      else next.delete(id);
      writeExpanded(EXPANDED_LISTS_KEY, next);
      return next;
    });
  }
  /** Opens or closes a List; its Lists may be on the next page, which opening then loads. */
  function toggle(id: string) {
    const open = !isOpen(id);
    setOpen(id, open);
    if (open && tree.children(id).length === 0 && hasMoreLists) void onLoadMoreLists();
  }
  // A List shows while every List around it is open.
  const rows = tree.shown.filter((list) => tree.up(list.id).slice(1).every((around) => isOpen(around.id)));
  const rowIds = rows.map((list) => list.id);
  const [moved, setMoved] = useState("");
  // The List a keyboard move carries, or one just made: its row would drop or lack focus.
  const refocus = useRef<string | null>(null);
  // The List a new List is being named inside, and that name.
  const [creatingIn, setCreatingIn] = useState<string | null>(null);
  const [subName, setSubName] = useState("");

  useEffect(() => {
    if (!refocus.current) return;
    listsNav.current?.querySelector<HTMLElement>(`[data-list-id="${CSS.escape(refocus.current)}"]`)?.focus();
    refocus.current = null;
  }, [lists, expanded, creatingIn]);

  // One move at a time: a second would name the revision the first is replacing.
  const moving = useRef(false);

  /** Puts the List there: a reorder among its siblings, or a placement under another parent. */
  async function placeList(list: TaskList, parentId: string | null, afterId: string | null) {
    const sameParent = parentOf(list) === parentId;
    if (busy || moving.current || (sameParent && alreadyThere(tree.children(parentId), list.id, afterId))) return;
    moving.current = true;
    const after = afterId && tree.byId.get(afterId)?.name;
    const parent = parentId && tree.byId.get(parentId)?.name;
    // Open where it lands in the same render as the move, so its row, and focus, show at once.
    if (parentId) setOpen(parentId, true);
    const placed = sameParent ? await onReorderList(list, afterId) : await onPlaceList(list, parentId, afterId);
    moving.current = false;
    if (placed) {
      setMoved(after ? `${list.name} moved after ${after}` : parent ? `${list.name} moved to the top of ${parent}` : `${list.name} moved to the top`);
    } else refocus.current = null;
  }

  async function moveInto(list: TaskList, parent: TaskList) {
    if (parentOf(list) === parent.id) {
      const others = tree.children(parent.id).filter((id) => id !== list.id);
      return placeList(list, parent.id, others[others.length - 1] ?? null);
    }
    if (busy || moving.current) return;
    moving.current = true;
    setOpen(parent.id, true);
    const placed = await onMoveList(list, parent.id);
    moving.current = false;
    if (placed) {
      setMoved(`${list.name} moved into ${parent.name}`);
    } else refocus.current = null;
  }

  const drag = useRowDrag({
    container: listsNav,
    canNest: true,
    // Nothing lands inside itself.
    canTarget: (dragged, id) => !tree.within(dragged, id),
    onDrop(dragged: string, drop: RowDrop) {
      const list = tree.byId.get(dragged);
      if (!list) return;
      if (drop.id === null) {
        const top = tree.children(null).filter((id) => id !== dragged);
        void placeList(list, null, top[top.length - 1] ?? null);
        return;
      }
      const target = tree.byId.get(drop.id);
      if (!target) return;
      if (drop.edge === "into") void moveInto(list, target);
      // Just below an open List's row is its first child's place.
      else if (drop.edge === "after" && tree.children(target.id).length > 0 && isOpen(target.id)) {
        void placeList(list, target.id, null);
      } else {
        const anchor = anchorFor(tree.children(parentOf(target)), dragged, target.id, drop.edge);
        if (anchor !== undefined) void placeList(list, parentOf(target), anchor);
      }
    },
  });

  /**
   * Alt with the arrows moves the List itself, as with Note Folders: Up and
   * Down among its siblings, Right into the List above it, Left out to just
   * after the List it is in.
   */
  function moveWithKeys(event: KeyboardEvent, list: TaskList) {
    if (!event.altKey || !["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault();
    if (busy || moving.current) return;
    const parentId = parentOf(list);
    const ids = tree.children(parentId);
    const at = ids.indexOf(list.id);
    refocus.current = list.id;
    if (event.key === "ArrowUp" && at > 0) void placeList(list, parentId, ids[at - 2] ?? null);
    else if (event.key === "ArrowDown" && at >= 0 && at < ids.length - 1) void placeList(list, parentId, ids[at + 1]);
    else if (event.key === "ArrowRight" && at > 0) void moveInto(list, tree.byId.get(ids[at - 1])!);
    else if (event.key === "ArrowLeft" && parentId) {
      const parent = tree.byId.get(parentId);
      if (parent) void placeList(list, parentOf(parent), parent.id);
      else refocus.current = null;
    } else refocus.current = null;
  }

  const dropEdge = (id: string) => {
    if (drag.drop?.id === id) return drag.drop.edge;
    return drag.drop?.edge === "end" && id === rowIds[rowIds.length - 1] ? "after" : undefined;
  };

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || !name.trim()) return;
    if (await onCreateList(name)) setName("");
  }

  function startSubList(list: TaskList) {
    setCreatingIn(list.id);
    setSubName("");
  }

  /** Back to the row action that opened the field. */
  function cancelSubList(parentId: string) {
    setCreatingIn(null);
    listsNav.current?.querySelector<HTMLElement>(`[data-new-list-in="${CSS.escape(parentId)}"]`)?.focus();
  }

  async function createSubList(event: FormEvent<HTMLFormElement>, parent: TaskList) {
    event.preventDefault();
    if (busy || !subName.trim()) return;
    const created = await onCreateSubList(parent, subName);
    if (!created) return;
    // The new List's row takes focus once its parent shows open.
    refocus.current = created.id;
    setCreatingIn(null);
    setOpen(parent.id, true);
  }

  async function rename(event: FormEvent<HTMLFormElement>, list: TaskList) {
    event.preventDefault();
    if (busy || !renameDraft.trim()) return;
    if (await onRenameList(list, renameDraft)) closeRename();
  }

  // While Notes are open, no Task view is the current place.
  const isCurrent = (place: TaskView) => !notesActive && place.kind === view.kind &&
    (place.kind !== "list" || (view.kind === "list" && view.listId === place.listId));
  // The page holds only the active view's Tasks, so only that folder gets a count.
  const count = (place: TaskView) => isCurrent(place) && shownCount !== null && (
    <span className="folder-count">{shownCount}{hasMoreTasks ? "+" : ""}</span>
  );

  return (
    <aside className="sidebar">
      <div className="sidebar-heading">Tasknotes</div>
      <nav aria-label="Workspace" className="sidebar-nav">
        <button type="button" disabled={busy}
          aria-label="All tasks"
          className="folder"
          aria-current={isCurrent({ kind: "all" }) ? "page" : undefined}
          onClick={() => onNavigate({ kind: "all" })}>
          <ListChecks size={17} aria-hidden="true" />
          <span className="folder-name">All tasks</span>
          {count({ kind: "all" })}
        </button>
        <button type="button" disabled={busy}
          aria-label="Inbox"
          className="folder"
          aria-current={isCurrent({ kind: "inbox" }) ? "page" : undefined}
          onClick={() => onNavigate({ kind: "inbox" })}>
          <Inbox size={17} aria-hidden="true" /><span className="folder-name">Inbox</span>
          {count({ kind: "inbox" })}
        </button>
        <button type="button" disabled={busy}
          aria-label="Removed"
          className="folder"
          aria-current={removedActive ? "page" : undefined}
          onClick={onOpenRemoved}>
          <Trash2 size={17} aria-hidden="true" /><span className="folder-name">Removed</span>
        </button>
        {(pendingApprovals > 0 || approvalsActive) && (
          <button type="button" disabled={busy}
            aria-label={`Waiting for approval, ${pendingApprovals}`}
            className="folder"
            aria-current={approvalsActive ? "page" : undefined}
            onClick={onOpenApprovals}>
            <ClipboardCheck size={17} aria-hidden="true" />
            <span className="folder-name">Waiting for approval</span>
            {pendingApprovals > 0 && <span className="folder-count">{pendingApprovals}</span>}
          </button>
        )}
      </nav>

      <div className="sidebar-heading">Lists</div>
      <span className="sr-only" aria-live="polite">{moved}</span>
      {listError && <p className="sidebar-error" role="status">{listError}</p>}
      <nav ref={listsNav} aria-label="Lists" className="sidebar-nav">
        {rows.map((list) => {
          const depth = tree.depth(list.id);
          const hasChildren = tree.hasChildren(list.id);
          const open = hasChildren && isOpen(list.id);
          const edge = dropEdge(list.id);
          // A line just below an open List's row belongs to the Lists inside it.
          const dropDepth = edge === "after" && open ? depth + 1 : depth;
          return (
          <Fragment key={list.id}>
          <div className="folder-row" data-drag-row={list.id}
            data-dragging={drag.dragging === list.id || undefined} data-drop-edge={edge}
            style={depth > 0 || edge ? { paddingLeft: depth * 14, "--drop-indent": `${dropDepth * 14}px` } as CSSProperties : undefined}>
            {hasChildren && (
              <button type="button" className="folder-disclosure" disabled={busy}
                aria-label={`${open ? "Collapse" : "Expand"} ${list.name}`} aria-expanded={open}
                onClick={() => toggle(list.id)}>
                {open ? <ChevronDown size={13} aria-hidden="true" /> : <ChevronRight size={13} aria-hidden="true" />}
              </button>
            )}
            {renaming === list.id ? (
              <form className="folder-form" onSubmit={(event) => void rename(event, list)}>
                <StyleIcon icon={list.icon} color={list.color} />
                <input aria-label="Rename list" value={renameDraft}
                  maxLength={100} disabled={busy} autoFocus
                  onChange={(event) => setRenameDraft(event.target.value)} />
                <button type="submit" className="folder-action"
                  aria-label="Save list name" disabled={busy || !renameDraft.trim()}>
                  <Check size={14} />
                </button>
                <button type="button" className="folder-action"
                  aria-label="Cancel rename" onClick={closeRename}><X size={14} /></button>
              </form>
            ) : (
              <>
                <button type="button" disabled={busy}
                  aria-label={list.name}
                  data-list-id={list.id}
                  aria-current={isCurrent({ kind: "list", listId: list.id }) ? "page" : undefined}
                  aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown Alt+ArrowLeft Alt+ArrowRight"
                  className="folder"
                  onMouseDown={(event) => drag.start(event, list.id)}
                  onKeyDown={(event) => moveWithKeys(event, list)}
                  onClick={() => onNavigate({ kind: "list", listId: list.id })}>
                  <StyleIcon icon={list.icon} color={list.color} /><span className="folder-name">{list.name}</span>
                  {count({ kind: "list", listId: list.id })}
                </button>
                <span className="folder-actions">
                  <button type="button" className="folder-action" disabled={busy}
                    aria-label={`Icon and colour for ${list.name}`} aria-expanded={styling === list.id}
                    onClick={() => setStyling(styling === list.id ? null : list.id)}>
                    <Palette size={13} />
                  </button>
                  {depth + 1 < MAX_LIST_DEPTH && (
                    <button type="button" className="folder-action" disabled={busy}
                      aria-label={`New List inside ${list.name}`} data-new-list-in={list.id}
                      onClick={() => startSubList(list)}>
                      <ListPlus size={13} />
                    </button>
                  )}
                  <button type="button" className="folder-action" disabled={busy}
                    aria-label={`Rename ${list.name}`}
                    onClick={() => { setRenaming(list.id); setRenameDraft(list.name); }}>
                    <Pencil size={13} />
                  </button>
                  <button type="button" className="folder-action" disabled={busy}
                    aria-label={`Remove ${list.name}`} onClick={() => void onRemoveList(list)}>
                    <Trash2 size={13} />
                  </button>
                </span>
              </>
            )}
          </div>
          {creatingIn === list.id && (
            <form className="folder-form folder-form--nested" style={{ marginLeft: (depth + 1) * 14 }}
              onSubmit={(event) => void createSubList(event, list)}>
              <Plus size={16} aria-hidden="true" />
              <input aria-label={`New List name in ${list.name}`} placeholder="New List" autoFocus
                maxLength={100} value={subName} disabled={busy} onChange={(event) => setSubName(event.target.value)}
                onKeyDown={(event) => { if (event.key === "Escape") cancelSubList(list.id); }} />
              <button type="button" className="folder-action" aria-label="Cancel new List"
                onClick={() => cancelSubList(list.id)}><X size={14} /></button>
            </form>
          )}
          {styling === list.id && (
            <StylePicker name={list.name} style={{ icon: list.icon ?? null, color: list.color ?? null }}
              busy={busy} onSave={(style) => saveStyle(list, style)} onCancel={closeStyle} />
          )}
          </Fragment>
          );
        })}
        {hasMoreLists && (
          <button className="sidebar-link" type="button" disabled={busy}
            onClick={() => void onLoadMoreLists()}>Load more lists</button>
        )}
      </nav>
      <form className="folder-form folder-form--new" onSubmit={(event) => void create(event)}>
        <Plus size={16} aria-hidden="true" />
        <input aria-label="New list name" placeholder="New List" maxLength={100}
          value={name} disabled={busy} onChange={(event) => setName(event.target.value)} />
        <button type="submit" className="folder-action" aria-label="Add list"
          disabled={busy || !name.trim()}><Check size={14} /></button>
      </form>

      <div className="sidebar-heading">Notes</div>
      <nav aria-label="Note folders" className="sidebar-nav">
        <button type="button" disabled={busy}
          aria-label="All notes"
          className="folder"
          aria-current={allNotesCurrent ? "page" : undefined}
          onClick={onOpenNotes}>
          <NotebookText size={17} aria-hidden="true" /><span className="folder-name">All notes</span>
        </button>
        {noteFolders}
      </nav>

      <div className="sidebar-spacer" />
      {pomodoro}
      <button type="button" className="folder folder--quiet" disabled={busy}
        aria-label="Pomodoro" aria-current={pomodoroHistoryActive ? "page" : undefined}
        onClick={onOpenPomodoroHistory}>
        <History size={16} aria-hidden="true" /><span className="folder-name">Pomodoro</span>
      </button>
      <button type="button" className="folder folder--quiet" disabled={busy}
        aria-label="Settings" aria-current={settingsActive ? "page" : undefined} onClick={onOpenSettings}>
        <Settings size={16} aria-hidden="true" /><span className="folder-name">Settings</span>
      </button>
      <button type="button" className="folder folder--quiet" onClick={onToggleTheme}
        aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}>
        {theme === "dark" ? <Sun size={16} aria-hidden="true" /> : <Moon size={16} aria-hidden="true" />}
        <span className="folder-name">{theme === "dark" ? "Light appearance" : "Dark appearance"}</span>
      </button>
      {resizer}
    </aside>
  );
}

/** Which Lists the person opened, kept as Note Folders keep theirs (`EXPANDED_FOLDERS_KEY`). */
export const EXPANDED_LISTS_KEY = "tasknotes.lists.expanded";
