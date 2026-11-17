import { invoke } from "./invoke";

/** One item of a Task's checklist (batch 4): its text and whether it is done. */
export interface Subtask {
  id: string;
  taskId: string;
  text: string;
  done: boolean;
  /** Taken off the checklist; restoring brings it back in its place. */
  removed: boolean;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export const subtaskApi = {
  list: (taskId: string) => invoke<Subtask[]>("list_subtasks", { taskId }),
  add: (taskId: string, text: string) => invoke<Subtask>("add_subtask", { input: { taskId, text } }),
  setDone: (item: Subtask, done: boolean) =>
    invoke<Subtask>("update_subtask", { input: { id: item.id, expectedRevision: item.revision, done } }),
  setText: (item: Subtask, text: string) =>
    invoke<Subtask>("update_subtask", { input: { id: item.id, expectedRevision: item.revision, text } }),
  move: (item: Subtask, index: number) =>
    invoke<Subtask>("move_subtask", { input: { id: item.id, expectedRevision: item.revision, index } }),
  remove: (item: Subtask) => invoke<Subtask>("remove_subtask", { id: item.id, expectedRevision: item.revision }),
  /** `item` as its removal left it: the restore names that revision. */
  restore: (item: Subtask) => invoke<Subtask>("restore_subtask", { id: item.id, expectedRevision: item.revision }),
};
