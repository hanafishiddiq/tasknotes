import { useEffect, useRef, useState } from "react";
import { FileText, Link2 } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { linkApi } from "../lib/link-api";
import { noteApi, type NoteSummary } from "../lib/note-api";
import { useLinks } from "../lib/use-links";
import LinkedList from "./LinkedList";

interface TaskLinkedNotesProps {
  taskId: string;
  /** False while the Notes workspace covers the Task view. */
  shown: boolean;
  onOpenNote: (noteId: string) => void;
}

/** The Notes linked to one Task, on its expanded page. */
export default function TaskLinkedNotes({ taskId, shown, onOpenNote }: TaskLinkedNotesProps) {
  const links = useLinks(
    taskId,
    shown,
    (cursor) => linkApi.notesOfTask(taskId, cursor),
  );
  // Recent Notes offered for linking; null while the picker is closed.
  const [choices, setChoices] = useState<NoteSummary[] | null>(null);
  const [choicesCursor, setChoicesCursor] = useState<string | null>(null);
  const opener = useRef<HTMLButtonElement>(null);
  const picker = useRef<HTMLDivElement>(null);
  const picking = choices !== null;
  const wasPicking = useRef(picking);

  // The button that opened the picker goes when it opens, and comes back when it closes: focus follows.
  useEffect(() => {
    if (picking && !wasPicking.current) picker.current?.querySelector("button")?.focus();
    if (!picking && wasPicking.current) opener.current?.focus();
    wasPicking.current = picking;
  }, [picking]);

  async function loadChoices(cursor: string | null) {
    links.setError(null);
    try {
      const page = await noteApi.list(cursor);
      setChoices((current) => [...(cursor === null ? [] : current ?? []), ...page.items]);
      setChoicesCursor(page.nextCursor);
    } catch (reason) {
      links.setError("Couldn't load Notes: " + errorMessage(reason));
    }
  }

  async function link(noteId: string) {
    if (await links.change(() => linkApi.link(taskId, noteId), "Couldn't link this Note: ")) setChoices(null);
  }

  const linked = new Set(links.items.map((item) => item.noteId));
  const offered = choices?.filter((note) => !linked.has(note.id)) ?? [];

  return (
    <LinkedList noun="Notes" links={links}
      icon={<FileText size={13} aria-hidden="true" />}
      onOpen={(item) => onOpenNote(item.noteId)}>
      {!picking ? (
        <button ref={opener} type="button" className="text-link linked-add" onClick={() => void loadChoices(null)}>
          <Link2 size={13} aria-hidden="true" /> Link a Note
        </button>
      ) : (
        <div ref={picker} className="linked-picker">
          {offered.length === 0 && choicesCursor === null && (
            <p className="linked-empty">No other Notes to link.</p>
          )}
          <ul className="linked-items" aria-label="Notes to link">
            {offered.map((note) => (
              <li key={note.id} className="linked-item">
                <button type="button" className="linked-open" disabled={links.busy}
                  onClick={() => void link(note.id)}>
                  <span className="linked-title">{note.title}</span>
                </button>
              </li>
            ))}
          </ul>
          <div className="linked-picker-actions">
            {choicesCursor !== null && (
              <button type="button" className="text-link" onClick={() => void loadChoices(choicesCursor)}>
                Load more Notes
              </button>
            )}
            <button type="button" className="text-link" onClick={() => setChoices(null)}>Cancel</button>
          </div>
        </div>
      )}
    </LinkedList>
  );
}
