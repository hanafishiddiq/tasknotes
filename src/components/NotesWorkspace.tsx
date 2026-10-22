import { useEffect, useRef, useState } from "react";
import { LoaderCircle, SquarePen } from "lucide-react";
import { noteApi, type Note, type NoteSummary } from "../lib/note-api";
import NoteEditor, { type NoteEditorHandle } from "./NoteEditor";

// Typing settles before an autosave reaches the core.
export const NOTE_AUTOSAVE_MS = 600;

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong. Please try again.";
}

const EDITED = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

/** What the open editor was loaded from or last saved as; `id` is null for an unsaved draft. */
interface Saved {
  id: string | null;
  revision: number;
  markdown: string;
}

const DRAFT: Saved = { id: null, revision: 0, markdown: "" };

function summary(note: Note): NoteSummary {
  return { id: note.id, title: note.title, revision: note.revision, updatedAt: note.updatedAt };
}

/** The Notes list beside one open Note, as in Apple Notes. */
export default function NotesWorkspace({ hidden }: { hidden: boolean }) {
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);
  // The editor remounts only when another Note or a new draft is opened,
  // never when a draft becomes a stored Note, so typing is not interrupted.
  const [editor, setEditor] = useState<{ key: number; markdown: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editorRef = useRef<NoteEditorHandle>(null);
  const saved = useRef<Saved>(DRAFT);
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const inFlight = useRef<Promise<boolean> | null>(null);
  const chosen = useRef(false);

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
        const latest = page.items[0] ? await noteApi.get(page.items[0].id) : null;
        // The person may already have picked a Note or started a new one.
        if (!active || chosen.current) return;
        show(latest ?? DRAFT);
      })
      .catch((reason) => { if (active) setError(errorMessage(reason)); });
    return () => {
      active = false;
      window.clearTimeout(timer.current);
    };
  }, []);

  function edited() {
    dirty.current = true;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void flush(), NOTE_AUTOSAVE_MS);
  }

  /** Saves pending edits now. Resolves false when the text could not be saved. */
  async function flush(): Promise<boolean> {
    window.clearTimeout(timer.current);
    while (inFlight.current) await inFlight.current;
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
        saved.current = { id: note.id, revision: note.revision, markdown: note.markdown };
        setOpenId(note.id);
        setNotes((current) => [summary(note), ...current.filter((item) => item.id !== note.id)]);
        setError(null);
        return true;
      } catch (reason) {
        // Keep the text on screen and try again on the next edit.
        dirty.current = true;
        setError("Couldn't save this Note: " + errorMessage(reason));
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
    chosen.current = true;
    if (id === saved.current.id || !(await flush())) return;
    try {
      const note = await noteApi.get(id);
      if (!note) {
        setError("This Note could not be found.");
        return;
      }
      show({ id: note.id, revision: note.revision, markdown: note.markdown });
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }

  async function newNote() {
    chosen.current = true;
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
      </section>
      <section className="note-editor-pane" aria-label="Note editor">
        <div className="note-save-state" role="status">
          {saving && <><LoaderCircle className="spin" size={13} aria-hidden="true" /> Saving…</>}
        </div>
        {error && <div role="alert" className="error-banner"><span>{error}</span></div>}
        {editor && (
          <NoteEditor key={editor.key} initialMarkdown={editor.markdown} onEdit={edited} editorRef={editorRef} />
        )}
      </section>
    </div>
  );
}
