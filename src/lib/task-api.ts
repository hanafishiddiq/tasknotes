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

export const TASK_PAGE_SIZE = 50;

// The desktop talks only to the Tauri command seam. Business rules live in Rust.
export const taskApi = {
  list(cursor: string | null = null, sort: SortMode = "priority"): Promise<TaskPage> {
    // Omit the default for compatibility with the first Ticket's command contract.
    return invoke("list_tasks", sort === "priority"
      ? { limit: TASK_PAGE_SIZE, cursor }
      : { limit: TASK_PAGE_SIZE, cursor, sort });
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
};
