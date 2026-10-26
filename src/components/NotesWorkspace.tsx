import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Bot, BotOff, Folder as FolderIcon, LoaderCircle, Pin, PinOff, Search, SquarePen, Trash2 } from "lucide-react";
import { errorMessage, isFolderRemoved } from "../lib/error-message";
import { ALL_NOTES, noteApi, type Note, type NoteScope, type NoteSummary } from "../lib/note-api";
import type { FolderOption } from "../lib/use-note-folders";
import { saveBeforeQuit } from "../lib/quit";
import { removedApi } from "../lib/removed-api";
import NoteEditor, { type NoteEditorHandle } from "./NoteEditor";
import NoteSnippet from "./NoteSnippet";
import NoteLinkedTasks from "./NoteLinkedTasks";

// Typing settles before an autosave reaches the core.
export const NOTE_AUTOSAVE_MS = 600;
// Typing settles before a search reaches the core.
const NOTE_SEARCH_MS = 250;

const EDITED = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

/**
 * What the open editor was loaded from or last saved as. For an unsaved draft
 * `id` is null and `folderId` is the folder the Note will be created in once
 * its first character is typed.
 */
interface Saved {
  id: string | null;
  revision: number;
  markdown: string;
  folderId: string | null;
  pinned: boolean;
  excludeFromAi: boolean;
}

function draftIn(scope: NoteScope): Saved {
  return {
    id: null, revision: 0, markdown: "", pinned: false, excludeFromAi: false,
    folderId: scope.kind === "folder" ? scope.folderId : null,
  };
}

function savedFrom(note: Note): Saved {
  return {
    id: note.id, revision: note.revision, markdown: note.markdown, folderId: note.folderId, pinned: note.pinned,
    excludeFromAi: note.excludeFromAi,
  };
}

function summary(note: Note): NoteSummary {
  return {
    id: note.id, title: note.title, revision: note.revision, updatedAt: note.updatedAt,
    folderId: note.folderId, pinned: note.pinned, snippet: null,
  };
}

const scopeKey = (scope: NoteScope) => scope.kind === "folder" ? scope.folderId : "";

/**
 * Asked for from elsewhere: global search shows its search here, optionally
 * opening one match; a Task's linked Note opens with an empty search.
 */
export interface NotesRequest {
  /** A new number for every request, so the same request can be made twice. */
  seq: number;
  search: string;
  openId: string | null;
}

interface NotesWorkspaceProps {
  hidden: boolean;
  /** Every Note, or the Notes directly in one folder. */
  scope: NoteScope;
  /** Heading for the list: "Notes" or the folder's name. */
  scopeName: string;
  /** Loaded folders, for moving the open Note. */
  folders: FolderOption[];
  request: NotesRequest | null;
  onOpenTask: (taskId: string) => void;
  /** The list asks again whenever this changes, e.g. after a restore. */
  refreshKey: number;
  /** Called after the open Note was moved to Removed, with the Note as it is now. */
  onRemoved: (note: Note) => void;
  /** Called when the core says the open folder is now hidden by Removed. */
  onScopeHidden: () => void;
}

/** Arrow keys move focus between the Notes in the list. */
function moveFocus(event: KeyboardEvent<HTMLUListElement>) {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  const items = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("button.note-list-item")];
  const at = items.indexOf(document.activeElement as HTMLButtonElement);
  if (at === -1) return;
  event.preventDefault();
  items[Math.min(items.length - 1, Math.max(0, at + (event.key === "ArrowDown" ? 1 : -1)))].focus();
}

/** An edited Note goes to the top of its group: pinned Notes stay above the rest. */
function placeEdited(list: NoteSummary[], item: NoteSummary): NoteSummary[] {
  const rest = list.filter((other) => other.id !== item.id);
  const firstUnpinned = rest.findIndex((other) => !other.pinned);
  const at = item.pinned ? 0 : firstUnpinned === -1 ? rest.length : firstUnpinned;
  return [...rest.slice(0, at), item, ...rest.slice(at)];
}

/** The Notes list beside one open Note, as in Apple Notes. */
export default function NotesWorkspace({
  hidden, scope, scopeName, folders, request, onOpenTask, refreshKey, onRemoved, onScopeHidden,
}: NotesWorkspaceProps) {
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  // Where the next page of older Notes starts; null once every Note is listed.
  const [moreCursor, setMoreCursor] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [openId, setOpenId] = useState<string | null>(null);
  // Where the open Note lives, whether it is pinned and excluded from AI, as the core last said.
  const [placement, setPlacement] = useState<{ folderId: string | null; pinned: boolean; excludeFromAi: boolean }>(
    { folderId: null, pinned: false, excludeFromAi: false },
  );
  const [placing, setPlacing] = useState(false);
  const [searchDraft, setSearchDraft] = useState("");
  // The search the list shows; empty means the scope's Notes.
  const [searchText, setSearchText] = useState("");
  const searchRef = useRef("");
  // Ignore an older list answer once a newer search or scope was asked for.
  const listGeneration = useRef(0);
  // The editor remounts only when another Note or a new draft is opened,
  // never when a draft becomes a stored Note, so typing is not interrupted.
  const [editor, setEditor] = useState<{ key: number; markdown: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Set when a save found a newer revision: autosave stops until the person chooses.
  const [conflict, setConflict] = useState(false);
  const conflicted = useRef(false);
  const editorRef = useRef<NoteEditorHandle>(null);
  const saved = useRef<Saved>(draftIn(scope));
  const scopeRef = useRef(scope);
  scopeRef.current = scope;
  const loadedScope = useRef<string | null>(null);
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const inFlight = useRef<Promise<boolean> | null>(null);
  // Set once the person opens a Note or a draft, so the first load does not replace it.
  const pickedByPerson = useRef(false);

  /** A search looks in every folder; otherwise the list is the scope's Notes. */
  function listPage(cursor: string | null) {
    const text = searchRef.current.trim();
    return text ? noteApi.list(cursor, ALL_NOTES, text) : noteApi.list(cursor, scopeRef.current);
  }

  function show(next: Saved) {
    saved.current = next;
    dirty.current = false;
    setOpenId(next.id);
    setPlacement({ folderId: next.folderId, pinned: next.pinned, excludeFromAi: next.excludeFromAi });
    setEditor((current) => ({ key: (current?.key ?? 0) + 1, markdown: next.markdown }));
  }

  // Loads the list for the scope; a later scope opens its newest Note, once the
  // open Note's pending text is saved.
  useEffect(() => {
    let active = true;
    const first = loadedScope.current === null;
    loadedScope.current = scopeKey(scope);
    // Opening a folder ends any search, as in Apple Notes.
    searchRef.current = "";
    setSearchDraft("");
    setSearchText("");
    const generation = ++listGeneration.current;
    (async () => {
      const left = first || (await flush());
      const page = await noteApi.list(null, scope);
      if (!active || generation !== listGeneration.current) return;
      setNotes(page.items);
      setMoreCursor(page.nextCursor);
      if (!left || (first && pickedByPerson.current)) return;
      const latest = page.items[0] ? await noteApi.get(page.items[0].id) : null;
      // The person may already have picked a Note or started a new one.
      if (!active || (first && pickedByPerson.current)) return;
      show(latest ? savedFrom(latest) : draftIn(scope));
    })().catch((reason) => { if (active) setError(errorMessage(reason)); });
    return () => { active = false; };
    // A scope is identified by its folder id, not by object identity.
  }, [scopeKey(scope)]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  useEffect(() => {
    const delay = window.setTimeout(() => setSearchText(searchDraft), NOTE_SEARCH_MS);
    return () => window.clearTimeout(delay);
  }, [searchDraft]);

  // A new search replaces the list; the open Note stays open.
  useEffect(() => {
    if (searchText === searchRef.current) return;
    searchRef.current = searchText;
    const generation = ++listGeneration.current;
    listPage(null)
      .then((page) => {
        if (generation !== listGeneration.current) return;
        setNotes(page.items);
        setMoreCursor(page.nextCursor);
        setError(null);
      })
      .catch((reason) => {
        if (generation === listGeneration.current) setError("Search couldn't run: " + errorMessage(reason));
      });
  }, [searchText]);

  // A request from global search or a Task's linked Note: show its search and
  // open its Note. Declared after the scope and search effects on purpose:
  // effects run in order, so when a request also changes the scope (or mounts
  // this component), its list and open Note replace theirs.
  useEffect(() => {
    if (!request) return;
    searchRef.current = request.search;
    setSearchDraft(request.search);
    setSearchText(request.search);
    const generation = ++listGeneration.current;
    listPage(null)
      .then(async (page) => {
        if (generation !== listGeneration.current) return;
        setNotes(page.items);
        setMoreCursor(page.nextCursor);
        // Without a chosen Note, the first match opens unless something is already open.
        const target = request.openId ??
          (saved.current.id === null && !dirty.current ? page.items[0]?.id : undefined);
        if (target) await openNote(target);
      })
      .catch((reason) => {
        if (generation === listGeneration.current) setError("Search couldn't run: " + errorMessage(reason));
      });
  }, [request?.seq]);

  // A restore elsewhere can bring Notes back into this list.
  const seenRefresh = useRef(refreshKey);
  useEffect(() => {
    if (refreshKey === seenRefresh.current) return;
    seenRefresh.current = refreshKey;
    refreshList().catch((reason) => {
      // The open folder, or one above it, went to Removed: the core decides that.
      if (scopeRef.current.kind === "folder" && isFolderRemoved(reason)) onScopeHidden();
      else setError("Couldn't refresh the Notes list: " + errorMessage(reason));
    });
  }, [refreshKey]);

  // flush reads only refs, so the first render's copy stays current.
  useEffect(() => saveBeforeQuit(flush), []);

  function edited() {
    dirty.current = true;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void flush(), NOTE_AUTOSAVE_MS);
  }

  /** Saves pending edits now. Resolves false when the text could not be saved. */
  async function flush(): Promise<boolean> {
    window.clearTimeout(timer.current);
    while (inFlight.current) await inFlight.current;
    if (conflicted.current) return false;
    if (!dirty.current) return true;
    const markdown = editorRef.current?.markdown() ?? "";
    const base = saved.current;
    dirty.current = false;
    // An untouched draft is never stored, so abandoned drafts leave nothing behind.
    if (markdown === base.markdown || (base.id === null && !markdown.trim())) return true;
    const run = (async () => {
      setSaving(true);
      try {
        const note = base.id === null
          ? await noteApi.create(markdown, base.folderId)
          : await noteApi.update(base.id, markdown, base.revision);
        saved.current = savedFrom(note);
        setOpenId(note.id);
        setNotes((current) => placeEdited(current, summary(note)));
        setError(null);
        return true;
      } catch (reason) {
        // Keep the text on screen and try again on the next edit.
        dirty.current = true;
        const latest = base.id === null ? null : await noteApi.get(base.id).catch(() => null);
        if (latest && latest.revision !== base.revision) {
          conflicted.current = true;
          setConflict(true);
        } else {
          setError("Couldn't save this Note: " + errorMessage(reason));
        }
        return false;
      } finally {
        setSaving(false);
      }
    })();
    inFlight.current = run;
    try {
      return await run;
    } finally {
      inFlight.current = null;
    }
  }

  async function openNote(id: string) {
    pickedByPerson.current = true;
    if (id === saved.current.id || !(await flush())) return;
    try {
      const note = await noteApi.get(id);
      if (!note) {
        setError("This Note could not be found.");
        return;
      }
      show(savedFrom(note));
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }

  async function loadMore() {
    if (moreCursor === null) return;
    setLoadingMore(true);
    try {
      const generation = listGeneration.current;
      const page = await listPage(moreCursor);
      if (generation !== listGeneration.current) return;
      // Keyset pages: a Note saved meanwhile is newer than the cursor, so it cannot repeat.
      setNotes((current) => [...current, ...page.items]);
      setMoreCursor(page.nextCursor);
    } catch (reason) {
      setError("Couldn't load more Notes: " + errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  /** Drops the unsaved text and opens the version saved elsewhere. */
  async function loadLatest() {
    const id = saved.current.id;
    if (id === null) return;
    try {
      const note = await noteApi.get(id);
      if (!note) {
        setError("This Note could not be found.");
        return;
      }
      conflicted.current = false;
      setConflict(false);
      setNotes((current) => current.map((item) => (item.id === note.id ? summary(note) : item)));
      show(savedFrom(note));
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }

  /** Stores the text on screen as its own Note; the other version stays as it is. */
  async function saveMineAsNew() {
    const markdown = editorRef.current?.markdown() ?? "";
    dirty.current = false;
    setSaving(true);
    try {
      const note = await noteApi.create(markdown, saved.current.folderId);
      saved.current = savedFrom(note);
      conflicted.current = false;
      setConflict(false);
      setOpenId(note.id);
      setNotes((current) => placeEdited(current, summary(note)));
      // Typing that landed while the new Note was being stored is saved next.
      if (dirty.current) edited();
    } catch (reason) {
      dirty.current = true;
      setError("Couldn't save this Note: " + errorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  /** The first page again, in the core's order, after the open Note was placed elsewhere. */
  async function refreshList() {
    const generation = ++listGeneration.current;
    const page = await listPage(null);
    if (generation !== listGeneration.current) return;
    setNotes(page.items);
    setMoreCursor(page.nextCursor);
  }

  /** Pins, moves or excludes the open Note from AI. None of these changes its text or revision. */
  async function place(change: (id: string) => Promise<Note>) {
    const id = saved.current.id;
    if (id === null || placing) return;
    setPlacing(true);
    try {
      const note = await change(id);
      saved.current = {
        ...saved.current, folderId: note.folderId, pinned: note.pinned, excludeFromAi: note.excludeFromAi,
      };
      setPlacement({ folderId: note.folderId, pinned: note.pinned, excludeFromAi: note.excludeFromAi });
      setError(null);
      await refreshList().catch((reason) => setError("Couldn't refresh the Notes list: " + errorMessage(reason)));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setPlacing(false);
    }
  }

  /** Moves the open Note to Removed, once its pending text is saved, and opens the next one. */
  async function removeOpen() {
    if (saved.current.id === null || placing || !(await flush())) return;
    const { id, revision } = saved.current;
    if (id === null) return;
    setPlacing(true);
    try {
      const removed = await removedApi.remove<Note>("note", { id, revision });
      const rest = notes.filter((item) => item.id !== id);
      setNotes(rest);
      setError(null);
      onRemoved(removed);
      const next = rest[0] ? await noteApi.get(rest[0].id) : null;
      show(next ? savedFrom(next) : draftIn(scopeRef.current));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setPlacing(false);
    }
  }

  async function newNote() {
    pickedByPerson.current = true;
    if (await flush()) show(draftIn(scopeRef.current));
  }

  return (
    <div className="notes-workspace" hidden={hidden}>
      <section className="note-list-pane" aria-label="Notes list">
        <header className="note-list-header">
          <h1>{scopeName}</h1>
          <button className="icon-button" type="button" aria-label="New note" onClick={() => void newNote()}>
            <SquarePen size={18} aria-hidden="true" />
          </button>
        </header>
        <label className="note-search">
          <Search size={14} aria-hidden="true" />
          <input type="search" aria-label="Search notes" placeholder="Search all notes" maxLength={200}
            value={searchDraft} onChange={(event) => setSearchDraft(event.target.value)} />
        </label>
        {searchText.trim() !== "" && notes.length === 0 && <p className="note-list-empty">No matching notes</p>}
        <ul className="note-list-items" aria-label="Notes" onKeyDown={moveFocus}>
          {notes.map((note, index) => (
            <li key={note.id}>
              {note.pinned && index === 0 && <h2 className="note-list-heading">Pinned</h2>}
              {!note.pinned && index > 0 && notes[index - 1].pinned &&
                <h2 className="note-list-heading">Notes</h2>}
              <button type="button" className="note-list-item"
                aria-current={openId === note.id ? "true" : undefined}
                onClick={() => void openNote(note.id)}>
                <span className="note-list-title">{note.title}</span>
                <span className="note-list-date">{EDITED.format(note.updatedAt)}</span>
                {note.snippet && <NoteSnippet text={note.snippet} />}
              </button>
            </li>
          ))}
        </ul>
        {moreCursor !== null && (
          <button type="button" className="note-list-more" disabled={loadingMore} onClick={() => void loadMore()}>
            {loadingMore ? "Loading..." : "Load more Notes"}
          </button>
        )}
      </section>
      <section className="note-editor-pane" aria-label="Note editor">
        <div className="note-toolbar">
          <label className="note-folder-picker">
            <FolderIcon size={14} aria-hidden="true" />
            <select aria-label="Folder" value={placement.folderId ?? ""}
              disabled={openId === null || placing}
              onChange={(event) => {
                const folderId = event.target.value || null;
                void place((id) => noteApi.move(id, folderId));
              }}>
              <option value="">No folder</option>
              {folders.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
              {/* A folder not loaded in the sidebar still shows as the current one. */}
              {placement.folderId !== null && !folders.some((option) => option.id === placement.folderId) &&
                <option value={placement.folderId}>Current folder</option>}
            </select>
          </label>
          <button className="icon-button" type="button" disabled={openId === null || placing}
            aria-label={placement.pinned ? "Unpin note" : "Pin note"} aria-pressed={placement.pinned}
            onClick={() => { const pinned = !placement.pinned; void place((id) => noteApi.setPinned(id, pinned)); }}>
            {placement.pinned ? <PinOff size={16} aria-hidden="true" /> : <Pin size={16} aria-hidden="true" />}
          </button>
          <button className="icon-button" type="button" disabled={openId === null || placing}
            aria-label="Exclude from AI" aria-pressed={placement.excludeFromAi}
            title={placement.excludeFromAi ? "Hidden from AI tools" : "Visible to AI tools"}
            onClick={() => {
              const excludeFromAi = !placement.excludeFromAi;
              void place((id) => noteApi.setExcludeFromAi(id, excludeFromAi));
            }}>
            {placement.excludeFromAi ? <BotOff size={16} aria-hidden="true" /> : <Bot size={16} aria-hidden="true" />}
          </button>
          <button className="icon-button" type="button" disabled={openId === null || placing}
            aria-label="Move note to Removed" onClick={() => void removeOpen()}>
            <Trash2 size={16} aria-hidden="true" />
          </button>
          <div className="note-save-state" role="status">
            {saving && <><LoaderCircle className="spin" size={13} aria-hidden="true" /> Saving…</>}
          </div>
        </div>
        {error && <div role="alert" className="error-banner"><span>{error}</span></div>}
        {conflict && (
          <div role="alert" className="notice-banner">
            <span>This Note was changed somewhere else. Your text is still here and was not saved over it.</span>
            <span className="notice-actions">
              <button className="button button-small" type="button" disabled={saving}
                onClick={() => void loadLatest()}>
                Load latest
              </button>
              <button className="button button-small" type="button" disabled={saving}
                onClick={() => void saveMineAsNew()}>
                Save mine as a new Note
              </button>
            </span>
          </div>
        )}
        {/* A draft has nothing to link until its first character stores it. */}
        {openId !== null && <NoteLinkedTasks noteId={openId} shown={!hidden} onOpenTask={onOpenTask} />}
        {editor && (
          <NoteEditor key={editor.key} initialMarkdown={editor.markdown} onEdit={edited} editorRef={editorRef} />
        )}
      </section>
    </div>
  );
}
