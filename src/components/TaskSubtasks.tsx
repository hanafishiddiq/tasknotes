import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { SUBTASK_TEXT_MAX, subtaskApi, type Subtask } from "../lib/subtask-api";

interface TaskSubtasksProps {
  taskId: string;
  /** While the Task has other unsaved edits or a change on its way. */
  disabled: boolean;
  /** How many items are done, of how many, whenever the checklist changes. */
  onCounts: (done: number, total: number) => void;
}

/** An item's name for a screen reader: its first line, kept short. */
function nameOf(item: Subtask): string {
  const line = item.text.split("\n")[0];
  return line.length > 120 ? `${line.slice(0, 120)}…` : line;
}

/** Grows a text box to its content, so a long item is read whole. */
function fit(box: HTMLTextAreaElement | null) {
  if (!box) return;
  box.style.height = "auto";
  box.style.height = `${box.scrollHeight}px`;
}

/**
 * A Task's checklist on its open page (batch 4): tick items done, edit them
 * in place (Return saves, Shift+Return breaks the line), move one with
 * Option and the arrows, remove one with Undo, and add more at the end. The
 * Task's own progress is never changed here.
 */
export default function TaskSubtasks({ taskId, disabled, onCounts }: TaskSubtasksProps) {
  const [items, setItems] = useState<Subtask[] | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [adding, setAdding] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [removed, setRemoved] = useState<Subtask | null>(null);
  const boxes = useRef(new Map<string, HTMLTextAreaElement>());
  const keepFocus = useRef<string | null>(null);
  const counts = useRef(onCounts);
  counts.current = onCounts;

  const load = useCallback(async () => {
    try {
      setItems(await subtaskApi.list(taskId));
    } catch (reason) {
      setError("Couldn't load the subtasks: " + errorMessage(reason));
    }
  }, [taskId]);
  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    if (items) counts.current(items.filter((item) => item.done).length, items.length);
  }, [items]);

  useLayoutEffect(() => {
    for (const box of boxes.current.values()) fit(box);
    if (keepFocus.current) {
      boxes.current.get(keepFocus.current)?.focus();
      keepFocus.current = null;
    }
  }, [items]);

  /** One change through the core; when it is refused, say why and show the checklist as it is now. */
  async function run(change: () => Promise<void>, failure: string) {
    setBusy(true);
    setError(null);
    try {
      await change();
    } catch (reason) {
      setError(failure + errorMessage(reason));
      await load();
    } finally {
      setBusy(false);
    }
  }
  const replace = (next: Subtask) =>
    setItems((current) => current?.map((item) => (item.id === next.id ? next : item)) ?? current);

  const add = () => {
    const text = adding.trim();
    if (!text) return;
    void run(async () => {
      const added = await subtaskApi.add(taskId, text);
      setItems((current) => [...(current ?? []), added]);
      setAdding("");
    }, "Couldn't add the subtask: ");
  };

  const saveText = (item: Subtask) => {
    const draft = drafts[item.id];
    if (draft === undefined) return;
    setDrafts(({ [item.id]: _, ...rest }) => rest);
    if (draft.trim() === item.text || !draft.trim()) return;
    void run(async () => replace(await subtaskApi.setText(item, draft.trim())), "Couldn't save the subtask: ");
  };

  const move = (item: Subtask, index: number) => {
    if (!items || index < 0 || index >= items.length) return;
    keepFocus.current = item.id;
    void run(async () => {
      const moved = await subtaskApi.move(item, index);
      keepFocus.current = item.id;
      setItems((current) => {
        if (!current) return current;
        const rest = current.filter((each) => each.id !== item.id);
        rest.splice(index, 0, moved);
        return rest;
      });
    }, "Couldn't move the subtask: ");
  };

  const onTextKey = (event: KeyboardEvent<HTMLTextAreaElement>, item: Subtask, index: number) => {
    if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      saveText(item);
      move(item, index + (event.key === "ArrowUp" ? -1 : 1));
    } else if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      event.currentTarget.blur();
    } else if (event.key === "Escape" && drafts[item.id] !== undefined) {
      event.preventDefault();
      setDrafts(({ [item.id]: _, ...rest }) => rest);
    }
  };

  const off = disabled || busy;
  return (
    <div className="subtasks" role="group" aria-label="Subtasks">
      <h3 className="linked-heading">Subtasks</h3>
      {error && <p role="alert" className="subtasks-error">{error}</p>}
      {items && items.length > 0 && (
        <ul className="subtask-items">
          {items.map((item, index) => (
            <li key={item.id} className={`subtask${item.done ? " subtask--done" : ""}`}>
              <input type="checkbox" className="subtask-check" aria-label={nameOf(item)} checked={item.done}
                disabled={off}
                onChange={(event) => {
                  const done = event.target.checked;
                  void run(async () => replace(await subtaskApi.setDone(item, done)), "Couldn't change the subtask: ");
                }} />
              <textarea className="subtask-text" aria-label="Subtask text" rows={1} maxLength={SUBTASK_TEXT_MAX}
                aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
                ref={(box) => { if (box) boxes.current.set(item.id, box); else boxes.current.delete(item.id); }}
                value={drafts[item.id] ?? item.text} disabled={disabled}
                onChange={(event) => {
                  fit(event.target);
                  const text = event.target.value;
                  setDrafts((current) => ({ ...current, [item.id]: text }));
                }}
                onBlur={() => saveText(item)}
                onKeyDown={(event) => onTextKey(event, item, index)} />
              <button type="button" className="subtask-remove" aria-label={`Remove “${nameOf(item)}”`}
                title="Remove" disabled={off}
                onClick={() => void run(async () => {
                  const gone = await subtaskApi.remove(item);
                  setItems((current) => current?.filter((each) => each.id !== item.id) ?? current);
                  setRemoved(gone);
                }, "Couldn't remove the subtask: ")}>
                <X size={13} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      {removed && (
        <p className="linked-undo" role="status">
          <span>“{nameOf(removed)}” removed</span>
          <button type="button" className="text-link" disabled={off}
            onClick={() => void run(async () => {
              await subtaskApi.restore(removed);
              setRemoved(null);
              await load();
            }, "Couldn't bring the subtask back: ")}>Undo</button>
        </p>
      )}
      <input className="subtask-add" aria-label="Add subtask" placeholder="Add subtask" maxLength={SUBTASK_TEXT_MAX}
        value={adding} disabled={disabled}
        onChange={(event) => setAdding(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.nativeEvent.isComposing) {
            event.preventDefault();
            if (!busy) add();
          }
        }} />
    </div>
  );
}
