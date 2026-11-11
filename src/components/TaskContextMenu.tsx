import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Check, ChevronRight, Trash2 } from "lucide-react";
import type { Priority, StatusChoice, Task } from "../lib/task-api";
import { statusChoiceOf } from "../lib/task-api";
import { PRIORITY_ORDER, PRIORITY_VIEW, statusChoiceLabel, statusChoiceState } from "../lib/task-visuals";

/** The status choices in the order the owner agreed for the menu (Q20). */
const MENU_STATUS: StatusChoice[] = ["ready", "in_progress", "completed", "wont_do"];
/** Space kept between the menu and the window's edges. */
const EDGE = 4;

interface TaskContextMenuProps {
  task: Task;
  /** Where it opens: the pointer, or the row for the keyboard. */
  x: number;
  y: number;
  expanded: boolean;
  /** The Task has unsaved edits, or a save is on its way: choices that change it wait. */
  changesHeld: boolean;
  onOpenToggle: () => void;
  onStatus: (choice: StatusChoice) => void;
  onPriority: (priority: Priority) => void;
  onRemove: () => void;
  /** Closes it; `refocus` puts focus back where it was when the menu opened. */
  onClose: (refocus: boolean) => void;
}

interface Choice {
  key: string;
  label: string;
  role: "menuitem" | "menuitemradio";
  checked?: boolean;
  disabled: boolean;
  reason?: string;
  icon?: ReactNode;
  className?: string;
  run: () => void;
}

/** Where a sheet of this size fits in the window, as near the asked point as it can. */
function fitted(x: number, y: number, width: number, height: number) {
  return {
    left: Math.max(EDGE, Math.min(x, window.innerWidth - width - EDGE)),
    top: Math.max(EDGE, Math.min(y, window.innerHeight - height - EDGE)),
  };
}

/** Moves focus among the enabled items of a sheet, wrapping at the ends. */
function moveFocus(sheet: HTMLElement | null, key: string) {
  const items = [...(sheet?.querySelectorAll<HTMLElement>(":scope > [role^='menuitem']:not([aria-disabled='true'])") ?? [])];
  if (items.length === 0) return;
  const at = items.indexOf(document.activeElement as HTMLElement);
  const next = key === "Home" ? 0
    : key === "End" ? items.length - 1
    : key === "ArrowDown" ? (at + 1) % items.length
    : (at - 1 + items.length) % items.length;
  items[next].focus();
}

/**
 * The Task menu (batch 3, Ticket 03): right-click or Shift+F10 on a Task row.
 * A sheet in the app's own look (Q21), every choice the existing control's
 * change (Q22). It holds focus while open and closes on a choice, a click
 * elsewhere, scrolling, resizing or the window losing focus.
 */
export default function TaskContextMenu({
  task, x, y, expanded, changesHeld, onOpenToggle, onStatus, onPriority, onRemove, onClose,
}: TaskContextMenuProps) {
  const sheet = useRef<HTMLDivElement>(null);
  const levelsSheet = useRef<HTMLDivElement>(null);
  const priorityItem = useRef<HTMLButtonElement>(null);
  const reasonId = useId();
  const [place, setPlace] = useState({ left: x, top: y });
  // The Priority submenu: where it sits, and whether it takes focus when it opens.
  const [levels, setLevels] = useState<{ left: number; top: number; focus: boolean } | null>(null);
  const latestClose = useRef(onClose);
  latestClose.current = onClose;

  const current = statusChoiceOf(task);
  const choose = (run: () => void) => () => {
    onClose(false);
    run();
  };
  const choices: (Choice | "separator" | "priority")[] = [
    {
      key: "open", label: expanded ? "Close" : "Open", role: "menuitem", disabled: false, run: choose(onOpenToggle),
    },
    "separator",
    ...MENU_STATUS.map((choice): Choice => {
      const { disabled, reason } = statusChoiceState(task, choice);
      return {
        key: choice, label: statusChoiceLabel(choice), role: "menuitemradio", checked: choice === current,
        disabled: changesHeld || disabled, reason,
        run: choose(() => { if (choice !== current) onStatus(choice); }),
      };
    }),
    "separator",
    "priority",
    "separator",
    {
      key: "remove", label: "Move to Removed", role: "menuitem", disabled: changesHeld,
      icon: <Trash2 size={13} aria-hidden="true" />, className: "task-menu-item--remove", run: choose(onRemove),
    },
  ];

  useLayoutEffect(() => {
    const element = sheet.current;
    if (!element) return;
    setPlace(fitted(x, y, element.offsetWidth, element.offsetHeight));
    element.querySelector<HTMLElement>("[role^='menuitem']:not([aria-disabled='true'])")?.focus();
  }, [x, y]);

  useEffect(() => {
    const inside = (target: EventTarget | null) =>
      target instanceof Node && (sheet.current?.contains(target) || levelsSheet.current?.contains(target));
    const away = (event: Event) => { if (!inside(event.target)) latestClose.current(false); };
    const leave = () => latestClose.current(false);
    document.addEventListener("pointerdown", away, true);
    document.addEventListener("scroll", leave, true);
    window.addEventListener("resize", leave);
    window.addEventListener("blur", leave);
    return () => {
      document.removeEventListener("pointerdown", away, true);
      document.removeEventListener("scroll", leave, true);
      window.removeEventListener("resize", leave);
      window.removeEventListener("blur", leave);
    };
  }, []);

  useLayoutEffect(() => {
    if (!levels) return;
    const element = levelsSheet.current;
    if (!element) return;
    if (levels.focus) {
      (element.querySelector<HTMLElement>("[aria-checked='true']")
        ?? element.querySelector<HTMLElement>("[role='menuitemradio']"))?.focus();
    }
  }, [levels]);

  function openLevels(focus: boolean) {
    if (changesHeld) return;
    const item = priorityItem.current;
    if (!item) return;
    const box = item.getBoundingClientRect();
    const width = levelsSheet.current?.offsetWidth || 160;
    const right = box.right + 2;
    const left = right + width + EDGE > window.innerWidth ? Math.max(EDGE, box.left - width - 2) : right;
    setLevels({ left, top: Math.max(EDGE, box.top - 5), focus });
  }

  function closeLevels() {
    setLevels(null);
    priorityItem.current?.focus();
  }

  function onKey(event: KeyboardEvent<HTMLDivElement>) {
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      moveFocus(sheet.current, event.key);
    } else if (event.key === "ArrowRight" && document.activeElement === priorityItem.current) {
      event.preventDefault();
      openLevels(true);
    } else if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      onClose(true);
    }
  }

  function onLevelsKey(event: KeyboardEvent<HTMLDivElement>) {
    event.stopPropagation();
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      moveFocus(levelsSheet.current, event.key);
    } else if (event.key === "ArrowLeft" || event.key === "Escape") {
      event.preventDefault();
      closeLevels();
    } else if (event.key === "Tab") {
      event.preventDefault();
      onClose(true);
    }
  }

  // Pointing at an item of the main sheet closes Priority; its own items keep it.
  const item = (choice: Choice, inLevels = false) => (
    <button
      key={choice.key}
      type="button"
      role={choice.role}
      tabIndex={-1}
      className={`task-menu-item ${choice.className ?? ""}`}
      aria-checked={choice.role === "menuitemradio" ? Boolean(choice.checked) : undefined}
      aria-disabled={choice.disabled || undefined}
      aria-describedby={choice.reason ? `${reasonId}-${choice.key}` : undefined}
      title={choice.reason}
      onMouseEnter={(event) => {
        if (!inLevels) setLevels(null);
        if (!choice.disabled) event.currentTarget.focus();
      }}
      onClick={() => { if (!choice.disabled) choice.run(); }}
    >
      <span className="task-menu-mark" aria-hidden="true">
        {choice.checked ? <Check size={13} strokeWidth={2.5} /> : choice.icon}
      </span>
      <span className="task-menu-label">{choice.label}</span>
      {choice.reason && <span id={`${reasonId}-${choice.key}`} hidden>{choice.reason}</span>}
    </button>
  );

  return (
    <>
      <div ref={sheet} className="task-menu" role="menu" aria-label={`Actions for “${task.title}”`}
        style={{ left: place.left, top: place.top }} onKeyDown={onKey}
        onContextMenu={(event) => event.preventDefault()}>
        {choices.map((choice, index) => {
          if (choice === "separator") return <div key={`separator-${index}`} className="task-menu-separator" role="separator" />;
          if (choice !== "priority") return item(choice);
          return (
            <button
              key="priority"
              ref={priorityItem}
              type="button"
              role="menuitem"
              tabIndex={-1}
              className="task-menu-item"
              aria-haspopup="menu"
              aria-expanded={levels !== null}
              aria-disabled={changesHeld || undefined}
              onMouseEnter={(event) => {
                if (changesHeld) return;
                event.currentTarget.focus();
                openLevels(false);
              }}
              onClick={() => openLevels(true)}
            >
              <span className="task-menu-mark" aria-hidden="true" />
              <span className="task-menu-label">Priority</span>
              <ChevronRight className="task-menu-more" size={13} aria-hidden="true" />
            </button>
          );
        })}
      </div>
      {levels && (
        <div ref={levelsSheet} className="task-menu" role="menu" aria-label="Priority"
          style={{ left: levels.left, top: levels.top }} onKeyDown={onLevelsKey}
          onContextMenu={(event) => event.preventDefault()}>
          {[...PRIORITY_ORDER].reverse().map((priority) => item({
            key: `priority-${priority}`, label: PRIORITY_VIEW[priority].label, role: "menuitemradio",
            checked: priority === task.priority, disabled: false,
            run: choose(() => { if (priority !== task.priority) onPriority(priority); }),
          }, true))}
        </div>
      )}
    </>
  );
}
