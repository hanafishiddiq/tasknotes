import { useEffect, useRef } from "react";

/** Where typing belongs to the field: its own undo, arrows and letters stay there. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return target.matches("input, textarea, select");
}

export interface ShortcutHandlers {
  /** ⌘K, from anywhere. */
  palette: () => void;
  /** ⌘N, from anywhere. */
  newTask: () => void;
  /** ⌘F, from anywhere. */
  search: () => void;
  /** ⌘, from anywhere. */
  settings: () => void;
  /** ⌘Z outside a text field; returns false when there is nothing to undo. */
  undo: () => boolean;
}

/** The app-wide ⌘ shortcuts. Handlers are read fresh on every key press. */
export function useShortcuts(handlers: ShortcutHandlers) {
  const latest = useRef(handlers);
  latest.current = handlers;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.isComposing) return;
      const key = event.key.toLowerCase();
      const run = latest.current;
      if (key === "k" && !event.shiftKey) run.palette();
      else if (key === "n" && !event.shiftKey) run.newTask();
      else if (key === "f" && !event.shiftKey) run.search();
      else if (key === "," && !event.shiftKey) run.settings();
      else if (key === "z" && !event.shiftKey && !isTypingTarget(event.target)) {
        if (!run.undo()) return;
      } else return;
      event.preventDefault();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
