import { invoke } from "@tauri-apps/api/core";

export type WorkStatus = "ready" | "in_progress" | "completed";
export type Priority = "very_low" | "low" | "medium" | "high" | "very_high";
export type SortMode = "priority" | "newest";

export interface Task {
  id: string;
  title: string;
  description: string;
  status: WorkStatus;
  priority: Priority;
  progress: number;
  nextStep: string;
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

export const TASK_PAGE_SIZE = 50;
export const HISTORY_PAGE_SIZE = 10;

// The desktop talks only to the Tauri command seam. Business rules live in Rust.
export const taskApi = {
  list(
    cursor: string | null = null,
    sort: SortMode = "priority",
  ): Promise<TaskPage> {
    // Omit the default for compatibility with the first Ticket's command contract.
    return invoke(
      "list_tasks",
      sort === "priority"
        ? { limit: TASK_PAGE_SIZE, cursor }
        : { limit: TASK_PAGE_SIZE, cursor, sort },
    );
  },

  create(input: TaskDraft): Promise<Task> {
    return invoke("create_task", { input });
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
