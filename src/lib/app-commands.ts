import type { Command } from "../components/CommandPalette";
import type { TaskList, TaskView } from "./task-api";
import { APPEARANCES, APPEARANCE_LABELS, type ThemePreference } from "./use-theme";

/** What the ⌘K palette can reach and do; App supplies the actions. */
export interface CommandSources {
  lists: TaskList[];
  /** The title Undo would bring back next, if any. */
  undoTitle: string | null;
  appearance: ThemePreference;
  navigate: (view: TaskView) => void;
  openNotes: () => void;
  openRemoved: () => void;
  openSettings: () => void;
  focusInTasks: (field: "capture" | "search") => void;
  undo: () => void;
  setAppearance: (next: ThemePreference) => void;
}

/** The palette's places and actions, in the order it lists them. */
export function buildCommands(from: CommandSources): Command[] {
  return [
    { id: "go-all", label: "All tasks", group: "Go to", run: () => from.navigate({ kind: "all" }) },
    { id: "go-inbox", label: "Inbox", group: "Go to", run: () => from.navigate({ kind: "inbox" }) },
    ...from.lists.filter((list) => !list.removed).map((list): Command => ({
      id: `go-list-${list.id}`, label: list.name, group: "Go to", keywords: "list",
      run: () => from.navigate({ kind: "list", listId: list.id }),
    })),
    { id: "go-notes", label: "All notes", group: "Go to", run: from.openNotes },
    { id: "go-removed", label: "Removed", group: "Go to", keywords: "trash restore", run: from.openRemoved },
    { id: "go-settings", label: "Settings", group: "Go to", keywords: "preferences", run: from.openSettings },
    {
      id: "new-task", label: "New task", group: "Actions", keywords: "add create capture",
      run: () => from.focusInTasks("capture"),
    },
    { id: "search", label: "Search tasks and notes", group: "Actions", keywords: "find", run: () => from.focusInTasks("search") },
    ...(from.undoTitle !== null ? [{
      id: "undo", label: `Undo moving “${from.undoTitle}” to Removed`, group: "Actions", keywords: "restore",
      run: from.undo,
    }] : []),
    ...APPEARANCES.filter((value) => value !== from.appearance).map((value): Command => ({
      id: `appearance-${value}`, label: `Appearance: ${APPEARANCE_LABELS[value]}`, group: "Actions",
      keywords: "theme mode", run: () => from.setAppearance(value),
    })),
  ];
}
