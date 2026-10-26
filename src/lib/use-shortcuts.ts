import { useEffect, useRef } from "react";
import { SHORTCUTS, type ShortcutAction } from "./shortcuts";

/** Where typing belongs to the field: its own undo, arrows and letters stay there. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return target.matches("input, textarea, select");
}

/**
 * One handler per action in SHORTCUTS. Returning false means "nothing to do",
 * and the key press is left alone (⌘Z with nothing to undo).
 */
export type ShortcutHandlers = Record<ShortcutAction, () => boolean | void>;

/** The app-wide ⌘ shortcuts from SHORTCUTS. Handlers are read fresh on every key press. */
export function useShortcuts(handlers: ShortcutHandlers) {
  const latest = useRef(handlers);
  latest.current = handlers;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey || event.isComposing) return;
      const key = event.key.toLowerCase();
      const command = SHORTCUTS.find((shortcut) => shortcut.command?.key === key)?.command;
      if (!command || (command.outsideFields && isTypingTarget(event.target))) return;
      if (latest.current[command.action]() === false) return;
      event.preventDefault();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
