import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Check, Search } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { NO_FILTERS, taskApi, type Task, type TaskList } from "../lib/task-api";
import PomodoroSheet from "./PomodoroSheet";

/** Typing waits this long before the core is asked, so each key does not cost two reads. */
const SEARCH_DELAY_MS = 150;

/** One choice: a Task, or the focus without one. */
type Option = { id: string | null; title: string; status: "in_progress" | "ready" | "none"; list?: string };

interface PomodoroTaskPickerProps {
  anchor: HTMLElement;
  chosen: string | null;
  lists: TaskList[];
  onChoose: (taskId: string | null, title: string | null) => void;
  onClose: (refocus: boolean) => void;
}

/** The Task's List as the sidebar names it; Inbox without one. */
function listName(task: Task, lists: TaskList[]): string {
  return task.listId ? lists.find((list) => list.id === task.listId)?.name ?? "" : "Inbox";
}

/**
 * The Task a focus is for (batch 3, Q28): "No Task" and every unfinished Task
 * in every List, In Progress first, then Ready, each with its List. The core
 * reads and searches them, so the sheet stays quick with thousands of Tasks;
 * past the first page of each group, typing narrows them. Focus stays in the
 * search field: the arrows move the highlight, Return chooses.
 */
export default function PomodoroTaskPicker({ anchor, chosen, lists, onChoose, onClose }: PomodoroTaskPickerProps) {
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<{ inProgress: Task[]; ready: Task[]; more: boolean } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const listId = useId();
  const optionId = useId();
  const field = useRef<HTMLInputElement>(null);

  useEffect(() => { field.current?.focus(); }, []);

  useEffect(() => {
    let current = true;
    const text = query.trim();
    const read = setTimeout(() => {
      Promise.all(["in_progress", "ready"].map((status) => taskApi.list(null, "priority", { kind: "all" },
        { ...NO_FILTERS, text, status: status as "in_progress" | "ready" })))
        .then(([inProgress, ready]) => {
          if (!current) return;
          setFound({
            inProgress: inProgress.items, ready: ready.items,
            more: inProgress.nextCursor !== null || ready.nextCursor !== null,
          });
          setError(null);
        })
        .catch((reason) => { if (current) setError("Couldn't load the Tasks: " + errorMessage(reason)); });
    }, text ? SEARCH_DELAY_MS : 0);
    return () => {
      current = false;
      clearTimeout(read);
    };
  }, [query]);

  const showNone = !query.trim() || "no task".includes(query.trim().toLowerCase());
  const toOption = (task: Task): Option => ({
    id: task.id, title: task.title, status: task.status === "in_progress" ? "in_progress" : "ready",
    list: listName(task, lists),
  });
  const groups: [string, Option[]][] = found
    ? [["In Progress", found.inProgress.map(toOption)], ["Ready", found.ready.map(toOption)]]
    : [];
  const options: Option[] = [
    ...(showNone ? [{ id: null, title: "No Task", status: "none" } as Option] : []),
    ...groups.flatMap(([, group]) => group),
  ];
  // The highlight starts on the chosen Task, or the first choice.
  const chosenIndex = options.findIndex((option) => option.id === chosen);
  const highlighted = options.length === 0 ? -1
    : Math.min(active ?? Math.max(chosenIndex, 0), options.length - 1);

  function choose(option: Option) {
    onChoose(option.id, option.id === null ? null : option.title);
    onClose(true);
  }

  function onKey(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (options.length === 0) return;
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActive((highlighted + step + options.length) % options.length);
    } else if (event.key === "Home" || event.key === "End") {
      if (options.length === 0) return;
      event.preventDefault();
      setActive(event.key === "Home" ? 0 : options.length - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (highlighted >= 0) choose(options[highlighted]);
    } else if (event.key === "Tab") {
      event.preventDefault();
      onClose(true);
    }
  }

  let index = 0;
  const option = (choice: Option) => {
    const at = index++;
    return (
      <div key={choice.id ?? "none"} id={`${optionId}-${at}`} role="option" className="pomo-option"
        aria-selected={choice.id === chosen} data-active={at === highlighted}
        onPointerMove={() => { if (at !== highlighted) setActive(at); }}
        onClick={() => choose(choice)}>
        <span className={`pomo-option-ring pomo-option-ring--${choice.status}`} aria-hidden="true" />
        <span className="pomo-option-title">{choice.title}</span>
        {choice.id === chosen
          ? <Check className="pomo-option-tick" size={13} strokeWidth={2.5} aria-label="Chosen" />
          : choice.list && <span className="pomo-option-list">{choice.list}</span>}
      </div>
    );
  };

  return (
    <PomodoroSheet anchor={anchor} align="center" label="Task for the focus" className="pomo-picker" onClose={onClose}>
      <label className="pomo-picker-search">
        <Search size={13} aria-hidden="true" />
        <input ref={field} type="text" placeholder="Search Tasks" value={query} aria-label="Search Tasks"
          role="combobox" aria-expanded="true" aria-controls={listId} autoComplete="off" spellCheck={false}
          aria-activedescendant={highlighted >= 0 ? `${optionId}-${highlighted}` : undefined}
          onChange={(event) => { setQuery(event.target.value); setActive(null); }} onKeyDown={onKey} />
      </label>
      <div id={listId} role="listbox" aria-label="Unfinished Tasks" className="pomo-picker-list">
        {showNone && option(options[0])}
        {groups.map(([name, group]) => group.length > 0 && (
          <div key={name} role="group" aria-label={name}>
            <div className="pomo-picker-head" aria-hidden="true">{name}</div>
            {group.map(option)}
          </div>
        ))}
        {found && options.length === 0 && <p className="pomo-picker-note">No unfinished Task matches</p>}
      </div>
      {found?.more && <p className="pomo-picker-note">Type to find more Tasks</p>}
      {error && <p className="pomo-picker-error" role="alert">{error}</p>}
    </PomodoroSheet>
  );
}
