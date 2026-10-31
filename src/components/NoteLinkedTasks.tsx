import { useEffect, useRef, useState } from "react";
import { Link2, SquareCheck } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { linkApi } from "../lib/link-api";
import { NO_FILTERS, taskApi, type Task } from "../lib/task-api";
import { STATUS_VIEW } from "../lib/task-visuals";
import { useLinks } from "../lib/use-links";
import LinkedList from "./LinkedList";

// Typing settles before a search reaches the core, as in the Task search.
const SEARCH_DELAY_MS = 250;

interface NoteLinkedTasksProps {
  noteId: string;
  /** False while the Task view covers the Notes workspace. */
  shown: boolean;
  onOpenTask: (taskId: string) => void;
}

/** The Tasks linked to the open Note, above its text. */
export default function NoteLinkedTasks({ noteId, shown, onOpenTask }: NoteLinkedTasksProps) {
  const links = useLinks(
    noteId,
    shown,
    (cursor) => linkApi.tasksOfNote(noteId, cursor),
  );
  const [picking, setPicking] = useState(false);
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<Task[]>([]);
  const opener = useRef<HTMLButtonElement>(null);
  const wasPicking = useRef(picking);

  // The button that opened the picker comes back when it closes: focus follows. (The search field takes it on opening.)
  useEffect(() => {
    if (!picking && wasPicking.current) opener.current?.focus();
    wasPicking.current = picking;
  }, [picking]);

  useEffect(() => { setPicking(false); }, [noteId]);

  // Finding a Task reuses the core Task search, so it matches what the Task view finds.
  useEffect(() => {
    if (!picking) return;
    let active = true;
    const timer = window.setTimeout(() => {
      taskApi.list(null, "priority", { kind: "all" }, { ...NO_FILTERS, text: query })
        .then((page) => { if (active) setFound(page.items); })
        .catch((reason) => { if (active) links.setError("Couldn't search Tasks: " + errorMessage(reason)); });
    }, SEARCH_DELAY_MS);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [picking, query]);

  async function link(taskId: string) {
    if (await links.change(() => linkApi.link(taskId, noteId), "Couldn't link this Task: ")) {
      setPicking(false);
      setQuery("");
    }
  }

  const linked = new Set(links.items.map((item) => item.taskId));
  const offered = found.filter((task) => !linked.has(task.id));

  return (
    <LinkedList noun="Tasks" links={links}
      icon={<SquareCheck size={13} aria-hidden="true" />}
      detail={(item) => <span className="linked-status">{STATUS_VIEW[item.status].label}</span>}
      onOpen={(item) => onOpenTask(item.taskId)}>
      {!picking ? (
        <button ref={opener} type="button" className="text-link linked-add" onClick={() => setPicking(true)}>
          <Link2 size={13} aria-hidden="true" /> Link a Task
        </button>
      ) : (
        <div className="linked-picker">
          <input type="search" className="linked-search" aria-label="Find a Task to link"
            placeholder="Search Tasks" maxLength={200} autoFocus value={query}
            onChange={(event) => setQuery(event.target.value)} />
          <ul className="linked-items" aria-label="Tasks to link">
            {offered.map((task) => (
              <li key={task.id} className="linked-item">
                <button type="button" className="linked-open" disabled={links.busy}
                  onClick={() => void link(task.id)}>
                  <span className="linked-title">{task.title}</span>
                  <span className="linked-status">{STATUS_VIEW[task.status].label}</span>
                </button>
              </li>
            ))}
          </ul>
          <div className="linked-picker-actions">
            <button type="button" className="text-link" onClick={() => setPicking(false)}>Cancel</button>
          </div>
        </div>
      )}
    </LinkedList>
  );
}
