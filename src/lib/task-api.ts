import { invoke } from "./invoke";
import type { Style } from "./style";

export type WorkStatus = "ready" | "in_progress" | "completed";
export type Priority = "very_low" | "low" | "medium" | "high" | "very_high";
/** Optional: a Task without one has No Difficulty (`null`). */
export type Difficulty = "very_easy" | "easy" | "medium" | "hard" | "very_hard";
/** "custom" is a List's Custom Order (ADR 0010): only for Inbox or one List. */
export type SortMode = "priority" | "newest" | "difficulty" | "custom";
export type TaskView = { kind: "all" } | { kind: "inbox" } | { kind: "list"; listId: string };

/** The key a view's remembered sort is kept under, as the core names views. */
export function viewKey(view: TaskView): string {
  return view.kind === "list" ? `list:${view.listId}` : view.kind;
}

/** The sort a view opens with: what it remembers, or Priority. */
export function sortOf(sorts: Readonly<Record<string, SortMode>>, view: TaskView): SortMode {
  const sort = sorts[viewKey(view)];
  return sort === "custom" && view.kind === "all" ? "priority" : sort ?? "priority";
}

/**
 * Where a Task goes in its group of its List's Custom Order: directly after a
 * Task (first when null), or directly before one, whose predecessor may sit on
 * another page.
 */
export type TaskPlace = { afterId: string | null } | { beforeId: string };

/** The Tasks of a page with one moved as `place` says; the same array when it cannot be placed. */
export function placeLocally(tasks: Task[], id: string, place: TaskPlace): Task[] {
  const moving = tasks.find((task) => task.id === id);
  if (!moving) return tasks;
  const others = tasks.filter((task) => task.id !== id);
  const at = "beforeId" in place ? others.findIndex((task) => task.id === place.beforeId)
    : place.afterId === null ? others.findIndex((task) => task.status === moving.status)
    : others.findIndex((task) => task.id === place.afterId) + 1;
  if (at < 0 || (at === 0 && "afterId" in place && place.afterId !== null)) return tasks;
  others.splice(at, 0, moving);
  return others;
}

export interface TaskList {
  id: string;
  name: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
  removed: boolean;
  /** Lucide icon name; null (or absent) is the default folder icon. */
  icon?: string | null;
  /** One of the twelve colour keys; null (or absent) is the accent. */
  color?: string | null;
}

export interface TaskListPage {
  items: TaskList[];
  nextCursor: string | null;
}

export interface ListMutation {
  id: string;
  expectedRevision: number;
}


export interface Task {
  id: string;
  title: string;
  description: string;
  status: WorkStatus;
  priority: Priority;
  difficulty: Difficulty | null;
  progress: number;
  /** A Completed Task set aside rather than finished (ADR 0008); keeps its progress. */
  wontDo: boolean;
  /** While Won't Do: the work status reopening returns it to. Absent otherwise. */
  wontDoFrom?: Exclude<WorkStatus, "completed">;
  nextStep: string;
  listId: string | null;
  dueDate: string | null;
  tags: string[];
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface TaskPage {
  items: Task[];
  nextCursor: string | null;
}

export interface TaskDraft {
  title: string;
  description: string;
}

export interface TaskEdit extends TaskDraft {
  id: string;
  expectedRevision: number;
}

export type TaskChange = {
  status?: WorkStatus;
  priority?: Priority;
  /** `null` clears it to No Difficulty. */
  difficulty?: Difficulty | null;
  progress?: number;
  /** `true` marks it Won't Do; `false` clears it and reopens the Task. */
  wontDo?: boolean;
};

export type ChangeTaskInput = TaskChange & {
  id: string;
  expectedRevision: number;
};

export interface ProgressDraft {
  status?: WorkStatus;
  progress?: number;
  nextStep?: string;
  note?: string;
}

export type RecordProgressInput = ProgressDraft & {
  id: string;
  expectedRevision: number;
};

export interface ProgressEntry {
  sequence: number;
  id: string;
  taskId: string;
  previousStatus: WorkStatus;
  status: WorkStatus;
  previousProgress: number;
  progress: number;
  previousWontDo: boolean;
  wontDo: boolean;
  note: string;
  nextStep: string;
  createdAt: number;
}

export interface ProgressHistoryPage {
  items: ProgressEntry[];
  nextCursor: number | null;
}

/** Search text and filters applied on top of the sidebar view. Empty means Any. */
export interface TaskFilters {
  text: string;
  /** "wont_do" keeps only Won't Do Tasks, which are Completed. */
  status: StatusChoice | "";
  priority: Priority | "";
  /** "" is Any difficulty (no filter); "none" keeps only Tasks with No Difficulty. */
  difficulty: Difficulty | "none" | "";
  tag: string;
  /** Show only this Task, as when it is opened from a Note it is linked to. */
  taskId?: string;
}

export const NO_FILTERS: TaskFilters = {
  text: "", status: "", priority: "", difficulty: "", tag: "",
};

export const TASK_PAGE_SIZE = 50;
export const HISTORY_PAGE_SIZE = 10;

/**
 * A status control's value: one of the three work statuses, or Won't Do,
 * which the core keeps as a resolution of Completed (ADR 0008), not a
 * fourth status. Every select and filter maps it through these two helpers.
 */
export type StatusChoice = WorkStatus | "wont_do";

/** The choice a Task shows in a status control. */
export function statusChoiceOf(task: Pick<Task, "status" | "wontDo">): StatusChoice {
  return task.wontDo ? "wont_do" : task.status;
}

/** What the core is sent for a choice, as a change or as a list filter. */
export function statusChoiceArgs(choice: StatusChoice): { wontDo: true } | { status: WorkStatus } {
  return choice === "wont_do" ? { wontDo: true } : { status: choice };
}

/**
 * What a click on a Task's round check does (Ticket 16): complete a Ready or
 * In Progress Task, reopen a Completed one, or clear Won't Do. Each is the
 * status control's own change; the core decides the progress that follows.
 */
export type CheckAction = "complete" | "reopen" | "clear_wont_do";

/** The check's action for a Task as it shows now. */
export function checkActionOf(task: Pick<Task, "status" | "wontDo">): CheckAction {
  if (task.wontDo) return "clear_wont_do";
  return task.status === "completed" ? "reopen" : "complete";
}

/** What the core is sent for a check action. Reopening restores the progress the Task had before. */
export function checkActionArgs(action: CheckAction): TaskChange {
  return action === "clear_wont_do" ? { wontDo: false }
    : action === "reopen" ? { status: "in_progress" }
    : { status: "completed" };
}

function queryArgs(view: TaskView, filters: TaskFilters) {
  const text = filters.text.trim();
  const tag = filters.tag.trim();
  return {
    ...(view.kind === "all" ? {} : { view }),
    ...(text ? { text } : {}),
    ...(filters.status ? statusChoiceArgs(filters.status) : {}),
    ...(filters.priority ? { priority: filters.priority } : {}),
    ...(filters.difficulty ? { difficulty: filters.difficulty } : {}),
    ...(tag ? { tag } : {}),
    ...(filters.taskId ? { taskId: filters.taskId } : {}),
  };
}

// The desktop talks only to the Tauri command seam. Business rules live in Rust.
export const taskApi = {
  list(
    cursor: string | null = null,
    sort: SortMode = "priority",
    view: TaskView = { kind: "all" },
    filters: TaskFilters = NO_FILTERS,
  ): Promise<TaskPage> {
    // Preserve the Ticket 01–03 default Tauri command contract: send only
    // what differs from the defaults.
    return invoke("list_tasks", {
      limit: TASK_PAGE_SIZE,
      cursor,
      ...(sort === "priority" ? {} : { sort }),
      ...queryArgs(view, filters),
    });
  },

  /** Whether one Task passes the view, search and filters, as the core decides. */
  async matches(taskId: string, view: TaskView, filters: TaskFilters): Promise<boolean> {
    if (filters.taskId && filters.taskId !== taskId) return false;
    const page = await invoke<TaskPage>("list_tasks", {
      limit: 1,
      cursor: null,
      ...queryArgs(view, filters),
      taskId,
    });
    return page.items.length > 0;
  },

  create(input: TaskDraft, listId?: string): Promise<Task> {
    return invoke("create_task", { input, ...(listId ? { listId } : {}) });
  },

  lists(cursor: string | null = null): Promise<TaskListPage> {
    return invoke("list_lists", { limit: 100, cursor, includeRemoved: true });
  },

  createList(name: string): Promise<TaskList> {
    return invoke("create_list", { input: { name } });
  },

  renameList(input: ListMutation & { name: string }): Promise<TaskList> {
    return invoke("rename_list", { input });
  },

  /** Puts the List directly after `afterId`, or first when it is null. */
  reorderList(input: ListMutation & { afterId: string | null }): Promise<TaskList> {
    return invoke("reorder_list", { input });
  },

  /** Sets both the icon and the colour; null clears one to the default look. */
  setListStyle(input: ListMutation & Style): Promise<TaskList> {
    return invoke("set_list_style", { input });
  },

  setListRemoved(input: ListMutation & { removed: boolean }): Promise<TaskList> {
    return invoke("set_list_removed", { input });
  },

  /**
   * Places a Task in its group of its List's Custom Order. `shownSort` is the
   * sort on screen: another than Custom starts the Custom Order from it and
   * switches the List to Custom.
   */
  reorder(input: ListMutation & TaskPlace & { shownSort: SortMode }): Promise<Task> {
    return invoke("reorder_task", { input });
  },

  /** The sort each view remembers; none when the core cannot say. */
  async viewSorts(): Promise<Record<string, SortMode>> {
    const sorts = await invoke<{ view: TaskView; sort: SortMode }[] | null>("view_sorts");
    return Object.fromEntries((Array.isArray(sorts) ? sorts : []).map(({ view, sort }) => [viewKey(view), sort]));
  },

  /** Remembers the sort the person chose for a view. */
  setViewSort(view: TaskView, sort: SortMode): Promise<void> {
    return invoke("set_view_sort", { input: { view, sort } });
  },

  move(input: ListMutation & { listId: string | null }): Promise<Task> {
    return invoke("move_task", { input });
  },

  details(input: ListMutation & { tags: string[]; dueDate: string | null }): Promise<Task> {
    return invoke("update_task_details", { input });
  },

  update(input: TaskEdit): Promise<Task> {
    return invoke("update_task", { input });
  },

  change(input: ChangeTaskInput): Promise<Task> {
    return invoke("change_task", { input });
  },

  recordProgress(input: RecordProgressInput): Promise<Task> {
    return invoke("record_progress", { input });
  },

  history(
    taskId: string,
    before: number | null = null,
  ): Promise<ProgressHistoryPage> {
    return invoke("list_progress_entries", {
      taskId,
      limit: HISTORY_PAGE_SIZE,
      before,
    });
  },
};
