import { invoke } from "@tauri-apps/api/core";

export type WorkStatus = "ready" | "in_progress" | "completed";
export type Priority = "very_low" | "low" | "medium" | "high" | "very_high";

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

export const TASK_PAGE_SIZE = 50;

// The desktop talks only to the Tauri command seam. Business rules live in Rust.
export const taskApi = {
  list(cursor: string | null = null): Promise<TaskPage> {
    return invoke("list_tasks", { limit: TASK_PAGE_SIZE, cursor });
  },

  create(input: TaskDraft): Promise<Task> {
    return invoke("create_task", { input });
  },

  update(input: TaskEdit): Promise<Task> {
    return invoke("update_task", { input });
  },
};
