import { useCallback, useState } from "react";

/** The Task fields a person types into before saving, as the fields hold them. */
export interface EditableFields {
  title: string;
  description: string;
  progress: string;
  nextStep: string;
  /** A new progress note; never stored on the Task itself. */
  note: string;
  tags: string;
  dueDate: string;
}

/**
 * What was typed into one Task and not saved yet, held above the rows so it
 * survives a row leaving the page (scrolling, paging, filters, sort, another
 * view). A field is present only while it differs from what was saved.
 */
export interface UnsavedTaskEdits {
  /**
   * The revision the typing started from. Every save names it, so a change
   * made elsewhere meanwhile is refused instead of being saved over.
   */
  baseRevision: number;
  fields: Partial<EditableFields>;
}

export interface TaskDrafts {
  get: (taskId: string) => UnsavedTaskEdits | undefined;
  /** Stores a Task's edits, or forgets them when given undefined. */
  set: (taskId: string, edits: UnsavedTaskEdits | undefined) => void;
}

/** The window's unsaved Task edits, kept for this session only. */
export function useTaskDrafts(): TaskDrafts {
  const [drafts, setDrafts] = useState<ReadonlyMap<string, UnsavedTaskEdits>>(new Map());

  const set = useCallback((taskId: string, edits: UnsavedTaskEdits | undefined) => {
    setDrafts((current) => {
      if (!edits && !current.has(taskId)) return current;
      const next = new Map(current);
      if (edits) next.set(taskId, edits); else next.delete(taskId);
      return next;
    });
  }, []);

  return { get: (taskId) => drafts.get(taskId), set };
}
