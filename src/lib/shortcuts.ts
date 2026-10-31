/** What an app-wide ⌘ shortcut does; App supplies one handler per action. */
export type ShortcutAction = "palette" | "newTask" | "search" | "settings" | "undo";

export interface Shortcut {
  /** As Settings shows it. */
  keys: string;
  label: string;
  /** For a ⌘ shortcut handled app-wide: the key pressed with ⌘ and what it does. */
  command?: {
    key: string;
    action: ShortcutAction;
    /** Inside a text field the key belongs to the field (⌘Z undoes typing). */
    outsideFields?: boolean;
  };
}

/** The keyboard shortcuts: Settings lists them, useShortcuts handles the ⌘ ones. */
export const SHORTCUTS: Shortcut[] = [
  { keys: "⌘K", label: "Command palette", command: { key: "k", action: "palette" } },
  { keys: "⌘N", label: "New task", command: { key: "n", action: "newTask" } },
  { keys: "⌘F", label: "Search tasks and notes", command: { key: "f", action: "search" } },
  {
    keys: "⌘Z", label: "Undo the latest move to Removed",
    command: { key: "z", action: "undo", outsideFields: true },
  },
  { keys: "⌘,", label: "Settings", command: { key: ",", action: "settings" } },
  { keys: "↑ ↓", label: "Move between tasks" },
  { keys: "Return", label: "Open or close the task" },
  { keys: "Esc", label: "Close the open task" },
];

/** How Settings and notices name the New task shortcut. */
export const newTaskKeys = SHORTCUTS.find((shortcut) => shortcut.command?.action === "newTask")!.keys;
