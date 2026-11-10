import type { TaskList } from "./task-api";

/**
 * Lists as the core reads them (ADR 0009): one flat page order in which a
 * List comes right after the List it sits in, siblings in manual order. The
 * sidebar keeps that order and works out the tree from `parentId`; whether a
 * List holds Lists not loaded yet is the core's `hasChildren`.
 */

/** The List a List sits in, or null at the top level. */
export const parentOf = (list: TaskList): string | null => list.parentId ?? null;

/** Far above the five levels the core allows: a damaged tree never loops. */
const WALK_LIMIT = 64;

export interface ListTree {
  byId: Map<string, TaskList>;
  /** Lists a person sees: not in Removed, nor inside a List in Removed. */
  shown: TaskList[];
  /** Ids of the shown Lists directly inside `parentId` (null: the top level), in order. */
  children: (parentId: string | null) => string[];
  /** The List and every loaded List around it, nearest first. */
  up: (id: string) => TaskList[];
  /** How many Lists `id` sits inside: 0 at the top level. */
  depth: (id: string) => number;
  /** Whether `id` is `ancestorId` or sits somewhere inside it. */
  within: (ancestorId: string, id: string) => boolean;
  /** Whether a shown List sits inside it, loaded or, as the core says, on a page not loaded yet. */
  hasChildren: (id: string) => boolean;
}

export function listTree(lists: TaskList[]): ListTree {
  const byId = new Map(lists.map((list) => [list.id, list]));
  const up = (id: string) => {
    const path: TaskList[] = [];
    for (let at = byId.get(id); at && path.length < WALK_LIMIT; at = byId.get(parentOf(at) ?? "")) path.push(at);
    return path;
  };
  const shown = lists.filter((list) => up(list.id).every((item) => !item.removed));
  const kids = new Map<string | null, string[]>();
  for (const list of shown) {
    const parent = parentOf(list);
    kids.set(parent, [...(kids.get(parent) ?? []), list.id]);
  }
  const children = (parentId: string | null) => kids.get(parentId) ?? [];
  return {
    byId,
    shown,
    children,
    up,
    depth: (id) => Math.max(0, up(id).length - 1),
    within: (ancestorId, id) => up(id).some((item) => item.id === ancestorId),
    hasChildren: (id) => children(id).length > 0 || byId.get(id)?.hasChildren === true,
  };
}

/** Where the subtree that starts at `from` ends (exclusive), in page order. */
function blockEnd(all: TaskList[], from: number, tree: ListTree): number {
  const root = all[from].id;
  let end = from + 1;
  while (end < all.length && tree.within(root, all[end].id)) end += 1;
  return end;
}

/**
 * The page order once `id`, with every List inside it, sits under `parentId`
 * directly after the sibling `afterId` (first without one). What the core
 * will read back, shown before it answers, its parents' `hasChildren`
 * included. Unchanged when an anchor is not loaded.
 */
export function moveSubtree(all: TaskList[], id: string, parentId: string | null, afterId: string | null): TaskList[] {
  const from = all.findIndex((list) => list.id === id);
  if (from < 0) return all;
  // Parents stay as they are in `rest`, so one tree serves both orders.
  const tree = listTree(all);
  const end = blockEnd(all, from, tree);
  const block = all.slice(from, end);
  const oldParent = parentOf(block[0]);
  block[0] = { ...block[0], parentId };
  const rest = [...all.slice(0, from), ...all.slice(end)].map((list) => {
    if (list.id === parentId) return { ...list, hasChildren: true };
    if (list.id === oldParent && oldParent !== parentId) {
      return { ...list, hasChildren: tree.children(oldParent).some((child) => child !== id) };
    }
    return list;
  });
  let at = 0;
  if (afterId !== null) {
    const anchor = rest.findIndex((list) => list.id === afterId);
    if (anchor < 0) return all;
    at = blockEnd(rest, anchor, tree);
  } else if (parentId !== null) {
    const parent = rest.findIndex((list) => list.id === parentId);
    if (parent < 0) return all;
    at = parent + 1;
  }
  return [...rest.slice(0, at), ...block, ...rest.slice(at)];
}
