import { useEffect, useState } from "react";
import { FileText, Link2, X } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { linkApi, type LinkedNote } from "../lib/link-api";
import { noteApi, type NoteSummary } from "../lib/note-api";

interface TaskLinkedNotesProps {
  taskId: string;
  onOpenNote: (noteId: string) => void;
}

/** The Notes linked to one Task, on its expanded page. */
export default function TaskLinkedNotes({ taskId, onOpenNote }: TaskLinkedNotesProps) {
  const [items, setItems] = useState<LinkedNote[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Recent Notes offered for linking; null while the picker is closed.
  const [choices, setChoices] = useState<NoteSummary[] | null>(null);
  const [choicesCursor, setChoicesCursor] = useState<string | null>(null);
  // The link just ended, kept so Undo can bring it back.
  const [undoable, setUndoable] = useState<LinkedNote | null>(null);

  async function refresh() {
    const page = await linkApi.notesOfTask(taskId);
    setItems(page.items);
  }

  useEffect(() => {
    let active = true;
    linkApi.notesOfTask(taskId)
      .then((page) => { if (active) setItems(page.items); })
      .catch((reason) => { if (active) setError("Couldn't load linked Notes: " + errorMessage(reason)); });
    return () => { active = false; };
  }, [taskId]);

  async function openPicker() {
    setError(null);
    try {
      const page = await noteApi.list();
      setChoices(page.items);
      setChoicesCursor(page.nextCursor);
    } catch (reason) {
      setError("Couldn't load Notes: " + errorMessage(reason));
    }
  }

  async function moreChoices() {
    if (choicesCursor === null) return;
    try {
      const page = await noteApi.list(choicesCursor);
      setChoices((current) => [...(current ?? []), ...page.items]);
      setChoicesCursor(page.nextCursor);
    } catch (reason) {
      setError("Couldn't load more Notes: " + errorMessage(reason));
    }
  }

  async function link(noteId: string) {
    setBusy(true);
    setError(null);
    try {
      await linkApi.link(taskId, noteId);
      setChoices(null);
      await refresh();
    } catch (reason) {
      setError("Couldn't link this Note: " + errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function unlink(item: LinkedNote) {
    setBusy(true);
    setError(null);
    try {
      await linkApi.unlink(item.linkId);
      setUndoable(item);
      await refresh();
    } catch (reason) {
      setError("Couldn't unlink this Note: " + errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function undo() {
    if (!undoable) return;
    setBusy(true);
    setError(null);
    try {
      // Linking the pair again restores the same link; nothing else was changed.
      await linkApi.link(taskId, undoable.noteId);
      setUndoable(null);
      await refresh();
    } catch (reason) {
      setError("Couldn't undo: " + errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  const linked = new Set(items.map((item) => item.noteId));
  const offered = choices?.filter((note) => !linked.has(note.id)) ?? [];

  return (
    <section className="linked-section" aria-label="Notes linked to this Task">
      <h4 className="linked-heading">Notes</h4>
      {error && <p role="alert" className="inline-error">{error}</p>}
      <ul className="linked-items" aria-label="Linked Notes">
        {items.map((item) => (
          <li key={item.linkId} className="linked-item">
            <button type="button" className="linked-open" onClick={() => onOpenNote(item.noteId)}>
              <FileText size={13} aria-hidden="true" />
              <span className="linked-title">{item.title}</span>
            </button>
            <button type="button" className="linked-unlink" aria-label={`Unlink ${item.title}`}
              disabled={busy} onClick={() => void unlink(item)}>
              <X size={13} aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
      {undoable && (
        <div role="status" aria-label="Unlinked" className="linked-undo">
          <span>Unlinked “{undoable.title}”. The Note itself is unchanged.</span>
          <button type="button" className="text-link" aria-label="Undo unlink" disabled={busy}
            onClick={() => void undo()}>Undo</button>
        </div>
      )}
      {choices === null ? (
        <button type="button" className="text-link linked-add" onClick={() => void openPicker()}>
          <Link2 size={13} aria-hidden="true" /> Link a Note
        </button>
      ) : (
        <div className="linked-picker">
          {offered.length === 0 && choicesCursor === null && (
            <p className="linked-empty">No other Notes to link.</p>
          )}
          <ul className="linked-items" aria-label="Notes to link">
            {offered.map((note) => (
              <li key={note.id} className="linked-item">
                <button type="button" className="linked-open" disabled={busy} onClick={() => void link(note.id)}>
                  {note.title}
                </button>
              </li>
            ))}
          </ul>
          <div className="linked-picker-actions">
            {choicesCursor !== null && (
              <button type="button" className="text-link" onClick={() => void moreChoices()}>Load more Notes</button>
            )}
            <button type="button" className="text-link" onClick={() => setChoices(null)}>Cancel</button>
          </div>
        </div>
      )}
    </section>
  );
}
