import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { subtaskApi, type Subtask } from "../lib/subtask-api";

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

/** Return without Shift, outside an input method's composition. */
const isPlainReturn = (event: KeyboardEvent) =>
  event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing;

/**
 * A Task's checklist on its open page (batch 4): tick items done, edit them
 * in place (Return saves, Shift+Return breaks the line), move one with
 * Option and the arrows, remove one with Undo, and add more at the end. The
 * Task's own progress is never changed here.
 *
 * Changes go to the core one after the other, each naming the item's newest
 * revision as the change before it left it: an edit saved on leaving the
 * text and the click that left it never refuse each other.
 */
export default function TaskSubtasks({ taskId, disabled, onCounts }: TaskSubtasksProps) {
  const [items, setItemsState] = useState<Subtask[] | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [adding, setAdding] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [removed, setRemoved] = useState<Subtask | null>(null);
  const group = useRef<HTMLDivElement>(null);
  const boxes = useRef(new Map<string, HTMLTextAreaElement>());
  const addBox = useRef<HTMLTextAreaElement>(null);
  const keepFocus = useRef<string | null>(null);
  /** The checklist as the core last answered, read when a queued change runs. */
  const latest = useRef<Subtask[] | null>(null);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const counts = useRef(onCounts);
  counts.current = onCounts;

  const setItems = useCallback((next: (current: Subtask[] | null) => Subtask[] | null) => {
    latest.current = next(latest.current);
    setItemsState(latest.current);
  }, []);
  const now = (id: string) => latest.current?.find((each) => each.id === id);

  const load = useCallback(async () => {
    try {
      const list = await subtaskApi.list(taskId);
      setItems(() => list);
    } catch (reason) {
      setError("Couldn't load the subtasks: " + errorMessage(reason));
    }
  }, [taskId, setItems]);
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

  // A narrower page wraps long items onto more lines: they grow to match.
  useEffect(() => {
    if (typeof ResizeObserver === "undefined" || !group.current) return;
    const observer = new ResizeObserver(() => {
      for (const box of boxes.current.values()) fit(box);
      fit(addBox.current);
    });
    observer.observe(group.current);
    return () => observer.disconnect();
  }, []);

  /**
   * Queues one change for the core. When it is refused, say why and show the
   * checklist as it is now; `undo` puts back what the window showed early.
   */
  function run(change: () => Promise<void>, failure: string, undo?: () => void) {
    queue.current = queue.current.then(async () => {
      setError(null);
      try {
        await change();
      } catch (reason) {
        undo?.();
        setError(failure + errorMessage(reason));
        await load();
      }
    });
  }
  const replace = (next: Subtask) =>
    setItems((current) => current?.map((item) => (item.id === next.id ? next : item)) ?? current);

  const add = () => {
    const text = adding.trim();
    if (!text) return;
    // The field empties at once, so the next item can be typed while this one saves.
    setAdding("");
    run(async () => {
      const added = await subtaskApi.add(taskId, text);
      setItems((current) => [...(current ?? []), added]);
    }, "Couldn't add the subtask: ", () => setAdding((typed) => typed || text));
  };

  const saveText = (id: string) => {
    const draft = drafts[id];
    if (draft === undefined) return;
    setDrafts(({ [id]: _, ...rest }) => rest);
    run(async () => {
      const item = now(id);
      if (!item || !draft.trim() || draft.trim() === item.text) return;
      replace(await subtaskApi.setText(item, draft.trim()));
    }, "Couldn't save the subtask: ");
  };

  const setDone = (id: string, done: boolean) =>
    run(async () => {
      const item = now(id);
      if (item && item.done !== done) replace(await subtaskApi.setDone(item, done));
    }, "Couldn't change the subtask: ");

  const move = (id: string, by: -1 | 1) =>
    run(async () => {
      const list = latest.current ?? [];
      const from = list.findIndex((each) => each.id === id);
      const to = from + by;
      if (from < 0 || to < 0 || to >= list.length) return;
      const moved = await subtaskApi.move(list[from], to);
      keepFocus.current = id;
      setItems((current) => {
        if (!current) return current;
        const rest = current.filter((each) => each.id !== id);
        rest.splice(to, 0, moved);
        return rest;
      });
    }, "Couldn't move the subtask: ");

  const remove = (id: string) =>
    run(async () => {
      const item = now(id);
      if (!item) return;
      const gone = await subtaskApi.remove(item);
      setItems((current) => current?.filter((each) => each.id !== id) ?? current);
      setRemoved(gone);
    }, "Couldn't remove the subtask: ");

  const restore = (gone: Subtask) =>
    run(async () => {
      await subtaskApi.restore(gone);
      setRemoved(null);
      await load();
    }, "Couldn't bring the subtask back: ");

  const onTextKey = (event: KeyboardEvent<HTMLTextAreaElement>, item: Subtask) => {
    if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      saveText(item.id);
      move(item.id, event.key === "ArrowUp" ? -1 : 1);
    } else if (isPlainReturn(event)) {
      event.preventDefault();
      event.currentTarget.blur();
    } else if (event.key === "Escape" && drafts[item.id] !== undefined) {
      event.preventDefault();
      setDrafts(({ [item.id]: _, ...rest }) => rest);
    }
  };

  return (
    <div ref={group} className="subtasks" role="group" aria-label="Subtasks">
      <h3 className="linked-heading">Subtasks</h3>
      {error && <p role="alert" className="subtasks-error">{error}</p>}
      {items && items.length > 0 && (
        <ul className="subtask-items">
          {items.map((item) => (
            <li key={item.id} className={`subtask${item.done ? " subtask--done" : ""}`}>
              <input type="checkbox" className="subtask-check" aria-label={nameOf(item)} checked={item.done}
                disabled={disabled}
                onChange={(event) => setDone(item.id, event.target.checked)} />
              <textarea className="subtask-text" aria-label="Subtask text" rows={1}
                aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
                ref={(box) => { if (box) boxes.current.set(item.id, box); else boxes.current.delete(item.id); }}
                value={drafts[item.id] ?? item.text} disabled={disabled}
                onChange={(event) => {
                  fit(event.target);
                  const text = event.target.value;
                  setDrafts((current) => ({ ...current, [item.id]: text }));
                }}
                onBlur={() => saveText(item.id)}
                onKeyDown={(event) => onTextKey(event, item)} />
              <button type="button" className="subtask-remove" aria-label={`Remove “${nameOf(item)}”`}
                title="Remove" disabled={disabled} onClick={() => remove(item.id)}>
                <X size={13} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      {removed && (
        <p className="linked-undo" role="status">
          <span>“{nameOf(removed)}” removed</span>
          <button type="button" className="text-link" disabled={disabled}
            onClick={() => restore(removed)}>Undo</button>
        </p>
      )}
      <textarea ref={addBox} className="subtask-add" aria-label="Add subtask" placeholder="Add subtask" rows={1}
        value={adding} disabled={disabled}
        onChange={(event) => {
          fit(event.target);
          setAdding(event.target.value);
        }}
        onKeyDown={(event) => {
          if (isPlainReturn(event)) {
            event.preventDefault();
            add();
          }
        }} />
    </div>
  );
}
