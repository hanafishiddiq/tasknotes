import { useState, type FormEvent } from "react";
import {
  CheckCircle2, Folder, Inbox, LockKeyhole, Pencil,
  Plus, RotateCcw, Trash2,
} from "lucide-react";
import type { TaskList, TaskView } from "../lib/task-api";

interface TaskSidebarProps {
  view: TaskView;
  lists: TaskList[];
  listError: string | null;
  shownCount: number;
  hasMoreTasks: boolean;
  hasMoreLists: boolean;
  busy: boolean;
  onNavigate: (view: TaskView) => void;
  onCreateList: (name: string) => Promise<boolean>;
  onRenameList: (list: TaskList, name: string) => Promise<void>;
  onRemoveList: (list: TaskList) => Promise<void>;
  onRestoreList: (list: TaskList) => Promise<void>;
  onLoadMoreLists: () => Promise<void>;
}

export default function TaskSidebar({
  view, lists, listError, shownCount, hasMoreTasks, hasMoreLists, busy,
  onNavigate, onCreateList, onRenameList, onRemoveList,
  onRestoreList, onLoadMoreLists,
}: TaskSidebarProps) {
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [removedVisible, setRemovedVisible] = useState(false);

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || !name.trim()) return;
    if (await onCreateList(name)) setName("");
  }

  async function rename(event: FormEvent<HTMLFormElement>, list: TaskList) {
    event.preventDefault();
    if (busy || !renameDraft.trim()) return;
    await onRenameList(list, renameDraft);
    setRenaming(null);
  }

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark"><CheckCircle2 size={22} strokeWidth={2.3} /></div>
        <div><strong>tasknotes</strong><span>Make room for progress.</span></div>
      </div>
      <div className="sidebar-section-title">WORKSPACE</div>
      <nav aria-label="Workspace" className="workspace-nav">
        <button type="button" disabled={busy}
          aria-label="All tasks"
          className={`nav-item ${view.kind === "all" ? "nav-item--active" : ""}`}
          aria-current={view.kind === "all" ? "page" : undefined}
          onClick={() => onNavigate({ kind: "all" })}>
          <CheckCircle2 size={18} strokeWidth={1.8} aria-hidden="true" />
          <span>All tasks</span>
          <span className="nav-count">{shownCount}{hasMoreTasks ? "+" : ""}</span>
        </button>
        <button type="button" disabled={busy}
          className={`nav-item ${view.kind === "inbox" ? "nav-item--active" : ""}`}
          aria-current={view.kind === "inbox" ? "page" : undefined}
          onClick={() => onNavigate({ kind: "inbox" })}>
          <Inbox size={18} aria-hidden="true" /><span>Inbox</span>
        </button>
      </nav>

      <div className="sidebar-section-title sidebar-list-title">LISTS</div>
      {listError && <p className="sidebar-list-error" role="status">{listError}</p>}
      <form className="sidebar-create-form" onSubmit={(event) => void create(event)}>
        <input aria-label="New list name" placeholder="New list..." maxLength={100}
          value={name} disabled={busy} onChange={(event) => setName(event.target.value)} />
        <button type="submit" className="sidebar-icon-button" aria-label="Add list"
          disabled={busy || !name.trim()}><Plus size={17} /></button>
      </form>
      <nav aria-label="Lists" className="lists-nav">
        {lists.filter((list) => !list.removed).map((list) => (
          <div className="sidebar-list-row" key={list.id}>
            {renaming === list.id ? (
              <form className="sidebar-rename-form" onSubmit={(event) => void rename(event, list)}>
                <input aria-label="Rename list" value={renameDraft}
                  maxLength={100} disabled={busy}
                  onChange={(event) => setRenameDraft(event.target.value)} />
                <button type="submit" className="sidebar-icon-button"
                  aria-label="Save list name" disabled={busy || !renameDraft.trim()}>
                  <CheckCircle2 size={15} />
                </button>
                <button type="button" className="sidebar-icon-button"
                  aria-label="Cancel rename" onClick={() => setRenaming(null)}>×</button>
              </form>
            ) : (
              <>
                <button type="button" disabled={busy}
                  aria-current={view.kind === "list" && view.listId === list.id ? "page" : undefined}
                  className={`nav-item nav-list-name ${view.kind === "list" && view.listId === list.id ? "nav-item--active" : ""}`}
                  onClick={() => onNavigate({ kind: "list", listId: list.id })}>
                  <Folder size={16} aria-hidden="true" /><span>{list.name}</span>
                </button>
                <button type="button" className="sidebar-icon-button" disabled={busy}
                  aria-label={`Rename ${list.name}`}
                  onClick={() => { setRenaming(list.id); setRenameDraft(list.name); }}>
                  <Pencil size={14} />
                </button>
                <button type="button" className="sidebar-icon-button" disabled={busy}
                  aria-label={`Remove ${list.name}`} onClick={() => void onRemoveList(list)}>
                  <Trash2 size={14} />
                </button>
              </>
            )}
          </div>
        ))}
        {hasMoreLists && (
          <button className="sidebar-load-more" type="button" disabled={busy}
            onClick={() => void onLoadMoreLists()}>Load more lists</button>
        )}
      </nav>
      {lists.some((list) => list.removed) && (
        <div className="removed-list-area">
          <button type="button" className="sidebar-load-more"
            onClick={() => setRemovedVisible((current) => !current)}>
            {removedVisible ? "Hide removed lists" : "Show removed lists"}
          </button>
          {removedVisible && lists.filter((list) => list.removed).map((list) => (
            <div key={list.id} className="sidebar-list-row">
              <span>{list.name}</span>
              <button type="button" className="sidebar-icon-button" disabled={busy}
                aria-label={`Restore ${list.name}`} onClick={() => void onRestoreList(list)}>
                <RotateCcw size={15} />
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="sidebar-spacer" />
      <div className="local-note">
        <span className="local-note-icon"><LockKeyhole size={15} /></span>
        <div><strong>Private by design</strong><span>Your tasks live on this Mac.</span></div>
      </div>
      <span className="sidebar-version">TASKNOTES · EARLY PREVIEW</span>
    </aside>
  );
}
