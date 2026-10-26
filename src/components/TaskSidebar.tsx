import { useState, type FormEvent, type ReactNode } from "react";
import {
  Check, Folder, Inbox, ListChecks, Moon, NotebookText, Pencil,
  Plus, Settings, Sun, Trash2, X,
} from "lucide-react";
import type { TaskList, TaskView } from "../lib/task-api";
import type { Theme } from "../lib/use-theme";

interface TaskSidebarProps {
  view: TaskView;
  /** The Notes workspace is showing instead of a Task view. */
  notesActive: boolean;
  /** The Removed view is showing. */
  removedActive: boolean;
  onOpenRemoved: () => void;
  settingsActive: boolean;
  onOpenSettings: () => void;
  /** Notes are showing every Note rather than one folder. */
  allNotesCurrent: boolean;
  /** The Note Folder tree, once Notes have been opened. */
  noteFolders?: ReactNode;
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
  onRenameList: (list: TaskList, name: string) => Promise<boolean>;
  onRemoveList: (list: TaskList) => Promise<void>;
  onLoadMoreLists: () => Promise<void>;
}

export default function TaskSidebar({
  view, notesActive, removedActive, onOpenRemoved, settingsActive, onOpenSettings, allNotesCurrent, noteFolders, lists, listError, shownCount, hasMoreTasks, hasMoreLists, busy,
  theme, onToggleTheme, onNavigate, onOpenNotes, onCreateList, onRenameList, onRemoveList,
  onLoadMoreLists,
}: TaskSidebarProps) {
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || !name.trim()) return;
    if (await onCreateList(name)) setName("");
  }

  async function rename(event: FormEvent<HTMLFormElement>, list: TaskList) {
    event.preventDefault();
    if (busy || !renameDraft.trim()) return;
    if (await onRenameList(list, renameDraft)) setRenaming(null);
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
      </nav>

      <div className="sidebar-heading">Lists</div>
      {listError && <p className="sidebar-error" role="status">{listError}</p>}
      <nav aria-label="Lists" className="sidebar-nav">
        {lists.filter((list) => !list.removed).map((list) => (
          <div className="folder-row" key={list.id}>
            {renaming === list.id ? (
              <form className="folder-form" onSubmit={(event) => void rename(event, list)}>
                <Folder size={17} aria-hidden="true" />
                <input aria-label="Rename list" value={renameDraft}
                  maxLength={100} disabled={busy} autoFocus
                  onChange={(event) => setRenameDraft(event.target.value)} />
                <button type="submit" className="folder-action"
                  aria-label="Save list name" disabled={busy || !renameDraft.trim()}>
                  <Check size={14} />
                </button>
                <button type="button" className="folder-action"
                  aria-label="Cancel rename" onClick={() => setRenaming(null)}><X size={14} /></button>
              </form>
            ) : (
              <>
                <button type="button" disabled={busy}
                  aria-label={list.name}
                  aria-current={isCurrent({ kind: "list", listId: list.id }) ? "page" : undefined}
                  className="folder"
                  onClick={() => onNavigate({ kind: "list", listId: list.id })}>
                  <Folder size={17} aria-hidden="true" /><span className="folder-name">{list.name}</span>
                  {count({ kind: "list", listId: list.id })}
                </button>
                <span className="folder-actions">
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
        ))}
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
      <button type="button" className="folder folder--quiet" disabled={busy}
        aria-label="Settings" aria-current={settingsActive ? "page" : undefined} onClick={onOpenSettings}>
        <Settings size={16} aria-hidden="true" /><span className="folder-name">Settings</span>
      </button>
      <button type="button" className="folder folder--quiet" onClick={onToggleTheme}
        aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}>
        {theme === "dark" ? <Sun size={16} aria-hidden="true" /> : <Moon size={16} aria-hidden="true" />}
        <span className="folder-name">{theme === "dark" ? "Light appearance" : "Dark appearance"}</span>
      </button>
    </aside>
  );
}
