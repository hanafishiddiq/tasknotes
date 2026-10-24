import { useEffect, useRef, useState } from "react";
import { LoaderCircle, SquarePen } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { noteApi, type Note, type NoteSummary } from "../lib/note-api";
import { saveBeforeQuit } from "../lib/quit";
import NoteEditor, { type NoteEditorHandle } from "./NoteEditor";
import NoteLinkedTasks from "./NoteLinkedTasks";

// Typing settles before an autosave reaches the core.
export const NOTE_AUTOSAVE_MS = 600;

const EDITED = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

/** What the open editor was loaded from or last saved as; `id` is null for an unsaved draft. */
interface Saved {
  id: string | null;
  revision: number;
  markdown: string;
}

const DRAFT: Saved = { id: null, revision: 0, markdown: "" };

function savedFrom(note: Note): Saved {
  return { id: note.id, revision: note.revision, markdown: note.markdown };
}

function summary(note: Note): NoteSummary {
  return { id: note.id, title: note.title, revision: note.revision, updatedAt: note.updatedAt };
}

interface NotesWorkspaceProps {
  hidden: boolean;
  /** A Note to open, asked for from elsewhere; a new count asks again. */
  request?: { noteId: string; count: number } | null;
  onOpenTask: (taskId: string) => void;
}

/** The Notes list beside one open Note, as in Apple Notes. */
export default function NotesWorkspace({ hidden, request = null, onOpenTask }: NotesWorkspaceProps) {
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  // Where the next page of older Notes starts; null once every Note is listed.
  const [moreCursor, setMoreCursor] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [openId, setOpenId] = useState<string | null>(null);
  // The editor remounts only when another Note or a new draft is opened,
  // never when a draft becomes a stored Note, so typing is not interrupted.
  const [editor, setEditor] = useState<{ key: number; markdown: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Set when a save found a newer revision: autosave stops until the person chooses.
  const [conflict, setConflict] = useState(false);
  const conflicted = useRef(false);
  const editorRef = useRef<NoteEditorHandle>(null);
  const saved = useRef<Saved>(DRAFT);
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const inFlight = useRef<Promise<boolean> | null>(null);
  // Set once the person opens a Note or a draft, so the first load does not replace it.
  const pickedByPerson = useRef(false);

  function show(next: Saved) {
    saved.current = next;
    dirty.current = false;
    setOpenId(next.id);
    setEditor((current) => ({ key: (current?.key ?? 0) + 1, markdown: next.markdown }));
  }

  useEffect(() => {
    let active = true;
    noteApi.list()
      .then(async (page) => {
        if (!active) return;
        setNotes(page.items);
        setMoreCursor(page.nextCursor);
        const latest = page.items[0] ? await noteApi.get(page.items[0].id) : null;
        // The person may already have picked a Note or started a new one.
        if (!active || pickedByPerson.current) return;
        show(latest ?? DRAFT);
      })
      .catch((reason) => { if (active) setError(errorMessage(reason)); });
    return () => {
      active = false;
      window.clearTimeout(timer.current);
    };
  }, []);

  useEffect(() => {
    if (request) void openNote(request.noteId);
    // A repeated request for the same Note opens it again only when count changes.
  }, [request?.count]);

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
          ? await noteApi.create(markdown)
          : await noteApi.update(base.id, markdown, base.revision);
        saved.current = savedFrom(note);
        setOpenId(note.id);
        setNotes((current) => [summary(note), ...current.filter((item) => item.id !== note.id)]);
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
      const page = await noteApi.list(moreCursor);
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
      const note = await noteApi.create(markdown);
      saved.current = savedFrom(note);
      conflicted.current = false;
      setConflict(false);
      setOpenId(note.id);
      setNotes((current) => [summary(note), ...current]);
      // Typing that landed while the new Note was being stored is saved next.
      if (dirty.current) edited();
    } catch (reason) {
      dirty.current = true;
      setError("Couldn't save this Note: " + errorMessage(reason));
    } finally {
      setSaving(false);
    }
  }

  async function newNote() {
    pickedByPerson.current = true;
    if (await flush()) show(DRAFT);
  }

  return (
    <div className="notes-workspace" hidden={hidden}>
      <section className="note-list-pane" aria-label="Notes list">
        <header className="note-list-header">
          <h1>Notes</h1>
          <button className="icon-button" type="button" aria-label="New note" onClick={() => void newNote()}>
            <SquarePen size={18} aria-hidden="true" />
          </button>
        </header>
        <ul className="note-list-items" aria-label="Notes">
          {notes.map((note) => (
            <li key={note.id}>
              <button type="button" className="note-list-item"
                aria-current={openId === note.id ? "true" : undefined}
                onClick={() => void openNote(note.id)}>
                <span className="note-list-title">{note.title}</span>
                <span className="note-list-date">{EDITED.format(note.updatedAt)}</span>
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
        <div className="note-save-state" role="status">
          {saving && <><LoaderCircle className="spin" size={13} aria-hidden="true" /> Saving…</>}
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
        {openId !== null && <NoteLinkedTasks noteId={openId} onOpenTask={onOpenTask} />}
        {editor && (
          <NoteEditor key={editor.key} initialMarkdown={editor.markdown} onEdit={edited} editorRef={editorRef} />
        )}
      </section>
    </div>
  );
}
