import type { Task, TaskList, UnfinishedCounts } from "../lib/task-api";

/**
 * What the core's `unfinished_counts` answers for these synthetic Tasks: Ready
 * and In Progress, outside Lists in Removed, a List counting only its own.
 * The rule itself is the core's and tested in Rust; window tests share this
 * one stand-in for it.
 */
export function countUnfinished(tasks: readonly Task[], lists: readonly TaskList[] = []): UnfinishedCounts {
  const hidden = new Set(lists.filter((list) => list.removed).map((list) => list.id));
  const open = tasks.filter((task) => task.status !== "completed" && !(task.listId && hidden.has(task.listId)));
  const byList: Record<string, number> = {};
  for (const task of open) if (task.listId) byList[task.listId] = (byList[task.listId] ?? 0) + 1;
  return { all: open.length, inbox: open.filter((task) => !task.listId).length, lists: byList };
}
