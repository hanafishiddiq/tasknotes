import { hidesCompleted, type Task, type TaskFilters } from "./task-api";

/**
 * What the list shows while its view hides Completed (batch 4): Completed
 * Tasks leave it, a Task completed here at once, except one holding unsaved
 * text: a re-read keeps such a Task on screen so the person's words never
 * vanish, and completing it elsewhere must not undo that.
 */
export function visibleTasks(tasks: Task[], hiding: boolean, holdsEdits: (taskId: string) => boolean): Task[] {
  return hiding ? tasks.filter((task) => task.status !== "completed" || holdsEdits(task.id)) : tasks;
}

/** What the row under the list offers: show the hidden Completed Tasks, hide them again, or nothing. */
export function completedRowFor({ filters, shownHere, count, tasks, loading, lastPage }: {
  filters: TaskFilters;
  /** The person showed Completed in this view. */
  shownHere: boolean;
  /** How many the core counts hidden for this very query; null until that count lands. */
  count: number | null;
  /** The Tasks on this page. */
  tasks: Task[];
  loading: boolean;
  lastPage: boolean;
}): "show" | "hide" | null {
  if (loading || !lastPage) return null;
  if (hidesCompleted(filters)) return count !== null && count > 0 ? "show" : null;
  // Shown here, and nothing (a search, a status filter, one Task) decides it instead.
  const wouldHide = hidesCompleted({ ...filters, hideCompleted: true });
  return shownHere && wouldHide && tasks.some((task) => task.status === "completed") ? "hide" : null;
}
