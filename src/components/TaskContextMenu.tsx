import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Check, ChevronRight, Trash2 } from "lucide-react";
import type { Priority, StatusChoice, Task } from "../lib/task-api";
import { statusChoiceOf } from "../lib/task-api";
import { PRIORITY_ORDER, PRIORITY_VIEW, statusChoiceLabel, statusChoiceState } from "../lib/task-visuals";

/** Where the Task menu opens: the pointer, or under the row for the keyboard. */
export interface MenuPoint {
  x: number;
  y: number;
}

/** The status choices in the order the owner agreed for the menu (Q20). */
const MENU_STATUS: StatusChoice[] = ["ready", "in_progress", "completed", "wont_do"];
/** Space kept between a sheet and the window's edges. */
const EDGE = 4;
/** The Priority sheet sits this far from its item, and lifts so its first level lines up with it (padding + hairline). */
const SUBMENU_GAP = 2;
const SUBMENU_LIFT = 5;
/** Pointing at another item closes Priority only after this, so a diagonal move toward it keeps it open. */
const SUBMENU_CLOSE_MS = 250;

interface TaskContextMenuProps {
  task: Task;
  at: MenuPoint;
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

/** One line of a sheet. */
type Entry =
  | { kind: "separator" }
  | {
    kind: "choice";
    key: string;
    label: string;
    role: "menuitem" | "menuitemradio";
    checked?: boolean;
    disabled: boolean;
    reason?: string;
    icon?: ReactNode;
    className?: string;
    /** The Priority item: it opens the second sheet instead of choosing. */
    submenu?: boolean;
    run: () => void;
  };
type Choice = Extract<Entry, { kind: "choice" }>;

/** Where a sheet of this size fits in the window, as near the asked point as it can. */
function fitted(x: number, y: number, width: number, height: number) {
  return {
    left: Math.max(EDGE, Math.min(x, window.innerWidth - width - EDGE)),
    top: Math.max(EDGE, Math.min(y, window.innerHeight - height - EDGE)),
  };
}

/**
 * The keys every sheet shares: arrows over the enabled items (wrapping), Home and End.
 * Returns whether it took the key.
 */
function moveFocus(sheet: HTMLElement | null, event: KeyboardEvent<HTMLElement>): boolean {
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return false;
  event.preventDefault();
  const items = [...(sheet?.querySelectorAll<HTMLElement>(":scope > [role^='menuitem']:not([aria-disabled='true'])") ?? [])];
  if (items.length === 0) return true;
  const at = items.indexOf(document.activeElement as HTMLElement);
  const next = event.key === "Home" ? 0
    : event.key === "End" ? items.length - 1
    : event.key === "ArrowDown" ? (at + 1) % items.length
    : (at - 1 + items.length) % items.length;
  items[next].focus();
  return true;
}

/**
 * The Task menu (batch 3, Ticket 03): right-click or Shift+F10 on a Task row.
 * A sheet in the app's own look (Q21), every choice the existing control's
 * change (Q22). It holds focus while open, gives it back after a choice, and
 * closes on a click elsewhere, scrolling, resizing, the window losing focus or
 * any ⌘ shortcut, which then goes on to do its own work.
 */
export default function TaskContextMenu({
  task, at, expanded, changesHeld, onOpenToggle, onStatus, onPriority, onRemove, onClose,
}: TaskContextMenuProps) {
  const sheet = useRef<HTMLDivElement>(null);
  const levelsSheet = useRef<HTMLDivElement>(null);
  const priorityItem = useRef<HTMLButtonElement | null>(null);
  const closeLevelsTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const reasonId = useId();
  const [place, setPlace] = useState({ left: at.x, top: at.y });
  // The Priority sheet: open or not, whether it takes focus, and where it fits once on screen.
  const [levels, setLevels] = useState<{ focus: boolean } | null>(null);
  const [levelsPlace, setLevelsPlace] = useState<{ left: number; top: number } | null>(null);
  const latestClose = useRef(onClose);
  latestClose.current = onClose;

  const current = statusChoiceOf(task);
  const choose = (run: () => void) => () => {
    onClose(true);
    run();
  };
  const entries: Entry[] = [
    { kind: "choice", key: "open", label: expanded ? "Close" : "Open", role: "menuitem", disabled: false, run: choose(onOpenToggle) },
    { kind: "separator" },
    ...MENU_STATUS.map((choice): Entry => {
      const { disabled, reason } = statusChoiceState(task, choice);
      return {
        kind: "choice", key: choice, label: statusChoiceLabel(choice), role: "menuitemradio",
        checked: choice === current, disabled: changesHeld || disabled, reason,
        run: choose(() => { if (choice !== current) onStatus(choice); }),
      };
    }),
    { kind: "separator" },
    { kind: "choice", key: "priority", label: "Priority", role: "menuitem", disabled: changesHeld, submenu: true, run: () => openLevels(true) },
    { kind: "separator" },
    {
      kind: "choice", key: "remove", label: "Move to Removed", role: "menuitem", disabled: changesHeld,
      icon: <Trash2 size={13} aria-hidden="true" />, className: "task-menu-item--remove", run: choose(onRemove),
    },
  ];
  // The same order as the open Task's Priority control.
  const levelEntries: Choice[] = PRIORITY_ORDER.map((priority) => ({
    kind: "choice", key: `priority-${priority}`, label: PRIORITY_VIEW[priority].label, role: "menuitemradio",
    checked: priority === task.priority, disabled: false,
    run: choose(() => { if (priority !== task.priority) onPriority(priority); }),
  }));

  useLayoutEffect(() => {
    const element = sheet.current;
    if (!element) return;
    setPlace(fitted(at.x, at.y, element.offsetWidth, element.offsetHeight));
    element.querySelector<HTMLElement>("[role^='menuitem']:not([aria-disabled='true'])")?.focus();
  }, [at.x, at.y]);

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
      clearTimeout(closeLevelsTimer.current);
      document.removeEventListener("pointerdown", away, true);
      document.removeEventListener("scroll", leave, true);
      window.removeEventListener("resize", leave);
      window.removeEventListener("blur", leave);
    };
  }, []);

  // Placed once it is on screen, with its real size: beside Priority, to the left
  // when there is no room on the right, and never past the window's bottom.
  useLayoutEffect(() => {
    const element = levelsSheet.current;
    const item = priorityItem.current;
    if (!levels || !element || !item) {
      setLevelsPlace(null);
      return;
    }
    const box = item.getBoundingClientRect();
    const width = element.offsetWidth;
    const right = box.right + SUBMENU_GAP;
    const x = right + width + EDGE > window.innerWidth ? box.left - width - SUBMENU_GAP : right;
    setLevelsPlace(fitted(x, box.top - SUBMENU_LIFT, width, element.offsetHeight));
    if (levels.focus) {
      (element.querySelector<HTMLElement>("[aria-checked='true']")
        ?? element.querySelector<HTMLElement>("[role='menuitemradio']"))?.focus();
    }
  }, [levels]);

  function openLevels(focus: boolean) {
    clearTimeout(closeLevelsTimer.current);
    if (!changesHeld) setLevels({ focus });
  }

  function closeLevels() {
    clearTimeout(closeLevelsTimer.current);
    setLevels(null);
    priorityItem.current?.focus();
  }

  /** Any ⌘ or Ctrl shortcut closes the menu and then does its own work. */
  function shortcutCloses(event: KeyboardEvent<HTMLDivElement>): boolean {
    if (!event.metaKey && !event.ctrlKey) return false;
    onClose(false);
    return true;
  }

  function onKey(event: KeyboardEvent<HTMLDivElement>) {
    if (shortcutCloses(event) || moveFocus(sheet.current, event)) return;
    if (event.key === "ArrowRight" && document.activeElement === priorityItem.current) {
      event.preventDefault();
      openLevels(true);
    } else if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      onClose(true);
    }
  }

  function onLevelsKey(event: KeyboardEvent<HTMLDivElement>) {
    if (shortcutCloses(event)) return;
    event.stopPropagation();
    if (moveFocus(levelsSheet.current, event)) return;
    if (event.key === "ArrowLeft" || event.key === "Escape") {
      event.preventDefault();
      closeLevels();
    } else if (event.key === "Tab") {
      event.preventDefault();
      onClose(true);
    }
  }

  const item = (choice: Choice, inLevels: boolean) => (
    <button
      key={choice.key}
      ref={choice.submenu ? priorityItem : undefined}
      type="button"
      role={choice.role}
      tabIndex={-1}
      className={`task-menu-item ${choice.className ?? ""}`}
      aria-checked={choice.role === "menuitemradio" ? Boolean(choice.checked) : undefined}
      aria-disabled={choice.disabled || undefined}
      aria-haspopup={choice.submenu ? "menu" : undefined}
      aria-expanded={choice.submenu ? levels !== null : undefined}
      aria-describedby={choice.reason ? `${reasonId}-${choice.key}` : undefined}
      title={choice.reason}
      onMouseEnter={(event) => {
        if (choice.disabled) return;
        event.currentTarget.focus();
        if (choice.submenu) openLevels(false);
        else if (!inLevels && levels) {
          clearTimeout(closeLevelsTimer.current);
          closeLevelsTimer.current = setTimeout(() => setLevels(null), SUBMENU_CLOSE_MS);
        }
      }}
      onClick={() => { if (!choice.disabled) choice.run(); }}
    >
      <span className="task-menu-mark" aria-hidden="true">
        {choice.checked ? <Check size={13} strokeWidth={2.5} /> : choice.icon}
      </span>
      <span className="task-menu-label">{choice.label}</span>
      {choice.submenu && <ChevronRight size={13} aria-hidden="true" />}
      {choice.reason && <span id={`${reasonId}-${choice.key}`} hidden>{choice.reason}</span>}
    </button>
  );

  return (
    <>
      <div ref={sheet} className="task-menu" role="menu" aria-label={`Actions for “${task.title}”`}
        style={{ left: place.left, top: place.top }} onKeyDown={onKey}
        onContextMenu={(event) => event.preventDefault()}>
        {entries.map((entry, index) => entry.kind === "separator"
          ? <div key={`separator-${index}`} className="task-menu-separator" role="separator" />
          : item(entry, false))}
      </div>
      {levels && (
        <div ref={levelsSheet} className="task-menu" role="menu" aria-label="Priority"
          // Off screen until placed with its real size.
          style={levelsPlace ?? { left: -9999, top: -9999 }} onKeyDown={onLevelsKey}
          onMouseEnter={() => clearTimeout(closeLevelsTimer.current)}
          onContextMenu={(event) => event.preventDefault()}>
          {levelEntries.map((entry) => item(entry, true))}
        </div>
      )}
    </>
  );
}
