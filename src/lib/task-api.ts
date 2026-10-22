import { invoke } from "@tauri-apps/api/core";

export type WorkStatus = "ready" | "in_progress" | "completed";
export type Priority = "very_low" | "low" | "medium" | "high" | "very_high";
export type SortMode = "priority" | "newest";
export type TaskView = { kind: "all" } | { kind: "inbox" } | { kind: "list"; listId: string };

export interface TaskList {
  id: string;
  name: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
  removed: boolean;
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
  progress: number;
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
  progress?: number;
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
  status: WorkStatus | "";
  priority: Priority | "";
  tag: string;
}

export const NO_FILTERS: TaskFilters = { text: "", status: "", priority: "", tag: "" };

export const TASK_PAGE_SIZE = 50;
export const HISTORY_PAGE_SIZE = 10;

function queryArgs(view: TaskView, filters: TaskFilters) {
  const text = filters.text.trim();
  const tag = filters.tag.trim();
  return {
    ...(view.kind === "all" ? {} : { view }),
    ...(text ? { text } : {}),
    ...(filters.status ? { status: filters.status } : {}),
    ...(filters.priority ? { priority: filters.priority } : {}),
    ...(tag ? { tag } : {}),
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
    const page = await invoke<TaskPage>("list_tasks", {
      limit: 1,
      cursor: null,
      taskId,
      ...queryArgs(view, filters),
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

  setListRemoved(input: ListMutation & { removed: boolean }): Promise<TaskList> {
    return invoke("set_list_removed", { input });
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
