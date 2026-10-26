import { useLayoutEffect, useRef, type FormEvent } from "react";
import { CalendarDays, Check, Folder, LoaderCircle, Tag, Trash2 } from "lucide-react";
import type {
  Task,
  TaskList,
  WorkStatus,
  Priority,
  TaskChange,
  ProgressDraft,
} from "../lib/task-api";
import TaskProgressHistory from "./TaskProgressHistory";
import TaskLinkedNotes from "./TaskLinkedNotes";
import { deadlineStatus } from "../lib/deadlines";
import type { EditableFields, UnsavedTaskEdits } from "../lib/use-task-drafts";
import {
  STATUS_ORDER,
  STATUS_VIEW,
  PRIORITY_ORDER,
  PRIORITY_VIEW,
} from "../lib/task-visuals";

const dateFormatter = new Intl.DateTimeFormat("en", {
  month: "short",
  day: "numeric",
});

const stampFormatter = new Intl.DateTimeFormat("en", {
  dateStyle: "long",
  timeStyle: "short",
});

function formatCalendarDeadline(dateOnly: string): string {
  // Calendar-only dates must not pass through UTC midnight.
  const [year, month, day] = dateOnly.split("-").map(Number);
  return dateFormatter.format(new Date(year, month - 1, day, 12));
}

interface TaskRowProps {
  task: Task;
  expanded: boolean;
  saving: boolean;
  onToggle: () => void;
  /** Every save names the revision its edits were typed against. */
  onSave: (title: string, description: string, expectedRevision: number) => Promise<void>;
  onChange: (change: TaskChange, expectedRevision: number) => Promise<void>;
  onRecordProgress: (update: ProgressDraft, expectedRevision: number) => Promise<void>;
  lists: TaskList[];
  today: string;
  onMove: (listId: string | null) => Promise<void>;
  onUpdateDetails: (tags: string[], dueDate: string | null, expectedRevision: number) => Promise<void>;
  onOpenNote: (noteId: string) => void;
  /** Moves the Task to Removed; restoring it later brings back everything. */
  onRemove: () => Promise<void>;
  /** False while the Notes workspace covers the Task view. */
  shown: boolean;
  /** What was typed and not saved, kept above the row so it outlives it. */
  edits: UnsavedTaskEdits | undefined;
  onEdits: (edits: UnsavedTaskEdits | undefined) => void;
}

/** What the fields show for a Task with nothing typed into it. */
function savedFields(task: Task): EditableFields {
  return {
    title: task.title,
    description: task.description,
    progress: String(task.progress),
    nextStep: task.nextStep ?? "",
    note: "",
    tags: (task.tags ?? []).join(", "),
    dueDate: task.dueDate ?? "",
  };
}

/** Keeps only the fields that still differ from what is saved; nothing left means no edits. */
function withoutSaved(
  baseRevision: number, fields: Partial<EditableFields>, saved: EditableFields,
): UnsavedTaskEdits | undefined {
  const kept: Partial<EditableFields> = {};
  for (const key of Object.keys(fields) as (keyof EditableFields)[]) {
    const value = fields[key];
    const unchanged = key === "note" ? !value?.trim() : value === saved[key];
    if (value !== undefined && !unchanged) kept[key] = value;
  }
  return Object.keys(kept).length > 0 ? { baseRevision, fields: kept } : undefined;
}

export default function TaskRow({
  task,
  expanded,
  saving,
  onToggle,
  onSave,
  onChange,
  onRecordProgress,
  lists,
  today,
  onMove,
  onUpdateDetails,
  onOpenNote,
  onRemove,
  shown,
  edits,
  onEdits,
}: TaskRowProps) {
  const saved = savedFields(task);
  const typed = edits?.fields ?? {};
  const draft: EditableFields = { ...saved, ...typed };
  const {
    title: draftTitle, description: draftDescription, progress: draftProgress,
    nextStep: draftNextStep, note: draftNote, tags: draftTags, dueDate: draftDueDate,
  } = draft;
  // Saves name the revision the typing started from, never a newer one.
  const baseRevision = edits?.baseRevision ?? task.revision;
  const changedElsewhere = edits !== undefined && edits.baseRevision !== task.revision;
  const PriorityIcon = PRIORITY_VIEW[task.priority].icon;
  const hiddenTags = (task.tags ?? []).slice(2);
  const titleField = useRef<HTMLTextAreaElement>(null);
  const due = deadlineStatus(task.dueDate, today, task.status);
  const hasUnsavedText = typed.title !== undefined || typed.description !== undefined;
  const hasUnsavedProgress = typed.progress !== undefined;
  const hasUnsavedNextStep = typed.nextStep !== undefined;
  const hasUnsavedNote = typed.note !== undefined;
  const hasUnsavedProgressUpdate =
    hasUnsavedProgress || hasUnsavedNextStep || hasUnsavedNote;
  const hasUnsavedDetails = typed.tags !== undefined || typed.dueDate !== undefined;
  const dirty = edits !== undefined;
  const canChangeDetails = !saving && !hasUnsavedText && !hasUnsavedProgressUpdate;
  const canChangeMetadata = !saving && !hasUnsavedText && !hasUnsavedDetails;
  const canChangeSelectors = canChangeMetadata && !hasUnsavedProgressUpdate;
  const numericProgress =
    draftProgress.trim() === "" ? NaN : Number(draftProgress);
  const validProgress =
    Number.isInteger(numericProgress) &&
    numericProgress >= 0 &&
    numericProgress <= 100;
  const progressButtonLabel =
    hasUnsavedNextStep || hasUnsavedNote
      ? "Save progress update"
      : "Apply progress";

  // The detail title wraps like a note heading; grow it with its content.
  useLayoutEffect(() => {
    const field = titleField.current;
    if (!field) return;
    const fit = () => {
      field.style.height = "auto";
      field.style.height = `${field.scrollHeight}px`;
    };
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, [draftTitle, expanded]);

  function edit(field: keyof EditableFields, value: string) {
    onEdits(withoutSaved(baseRevision, { ...typed, [field]: value }, saved));
  }

  function forget(fields: (keyof EditableFields)[]) {
    const rest = { ...typed };
    for (const field of fields) delete rest[field];
    onEdits(withoutSaved(baseRevision, rest, saved));
  }

  /** Carries on from the newer version: typed fields stay, the rest show what is saved now. */
  function keepOnLatest() {
    onEdits(withoutSaved(task.revision, typed, saved));
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draftTitle.trim() || saving || hasUnsavedProgressUpdate || hasUnsavedDetails) return;
    await onSave(draftTitle, draftDescription, baseRevision);
  }

  async function saveProgressUpdate() {
    if (!canChangeMetadata || !hasUnsavedProgressUpdate || !validProgress)
      return;
    // Keep the older simple button behavior for percentage-only updates.
    if (!hasUnsavedNextStep && !hasUnsavedNote) {
      await onChange({ progress: numericProgress }, baseRevision);
      return;
    }
    await onRecordProgress({
      ...(hasUnsavedProgress ? { progress: numericProgress } : {}),
      ...(hasUnsavedNextStep ? { nextStep: draftNextStep } : {}),
      ...(hasUnsavedNote ? { note: draftNote.trim() } : {}),
    }, baseRevision);
  }

  async function saveDetails() {
    if (!canChangeDetails || !hasUnsavedDetails) return;
    const tags = draftTags.split(",").map((value) => value.trim()).filter(Boolean);
    await onUpdateDetails(tags, draftDueDate || null, baseRevision);
  }

  function discardUpdateDraft() {
    forget(["progress", "nextStep", "note"]);
  }

  function discardDetailsDraft() {
    forget(["tags", "dueDate"]);
  }

  const listName = task.listId
    ? lists.find((list) => list.id === task.listId)?.name ?? "List not loaded"
    : "Inbox";
  const preview = task.nextStep
    ? `Next: ${task.nextStep}`
    : task.description || `Created ${dateFormatter.format(new Date(task.createdAt))}`;

  return (
    <article
      className={`task-card task-card--${task.status} ${expanded ? "task-card--expanded" : ""}`}
      aria-label={task.title}
    >
      <div className="task-row">
        <span className={`task-check task-check--${task.status}`} aria-hidden="true">
          {task.status === "completed" && <Check size={11} strokeWidth={3} />}
        </span>
        <button
          className="task-open"
          type="button"
          onClick={onToggle}
          aria-label={`${expanded ? "Close" : "Open"} ${task.title}`}
          aria-expanded={expanded}
        >
          <span className="task-row-title">{task.title}</span>
          <span className="task-row-subtitle">
            {task.status === "in_progress" && (
              <span className="progress-readout" aria-label={`Progress ${task.progress}%`}>
                {task.progress}%
              </span>
            )}
            <span className="task-summary">{preview}</span>
          </span>
        </button>
        <div className="task-row-meta">
          {dirty && !expanded && <span className="unsaved-marker">Unsaved edits</span>}
          <span className={`status-pill status-pill--${task.status} sr-only`}>
            {STATUS_VIEW[task.status].label}
          </span>
          <span className={`priority-pill priority-pill--${task.priority}`}>
            <PriorityIcon size={13} strokeWidth={2} aria-hidden="true" />
            {PRIORITY_VIEW[task.priority].label}
          </span>
          {due === "overdue" && <span className="deadline-pill deadline-pill--overdue">Overdue</span>}
          {due === "today" && <span className="deadline-pill deadline-pill--today">Due today</span>}
          {due === "upcoming" && task.dueDate && (
            <span className="deadline-pill deadline-pill--upcoming">Due {formatCalendarDeadline(task.dueDate)}</span>
          )}
          {(task.tags ?? []).slice(0, 2).map((tag) => (
            <span className="tag-pill" key={tag}>#{tag}</span>
          ))}
          {hiddenTags.length > 0 && (
            <span className="tag-pill tag-pill--more" role="note" title={hiddenTags.join(", ")}
              aria-label={`${hiddenTags.length} more tags: ${hiddenTags.join(", ")}`}>
              +{hiddenTags.length}
            </span>
          )}
        </div>
      </div>

      {expanded && (
        <section
          role="region"
          aria-label="Task details"
          className="task-editor"
        >
          <p className="task-stamp">
            Updated {stampFormatter.format(new Date(task.updatedAt))} · {listName}
          </p>
          {changedElsewhere && (
            <div role="alert" aria-label="Changed elsewhere" className="notice-banner">
              <span>
                This Task was changed somewhere else after you started typing. Your edits are
                still here and saving them now is refused. Discard them to see the latest, or
                keep them to save over the change.
              </span>
              <span className="notice-actions">
                <button className="button button-small" type="button" disabled={saving}
                  onClick={() => onEdits(undefined)}>
                  Discard my edits
                </button>
                <button className="button button-small" type="button" disabled={saving}
                  onClick={keepOnLatest}>
                  Keep my edits
                </button>
              </span>
            </div>
          )}
          <form className="task-page" onSubmit={save}>
            <label className="sr-only" htmlFor={`title-${task.id}`}>Title</label>
            <textarea
              ref={titleField}
              id={`title-${task.id}`}
              className="task-page-title"
              aria-label="Edit title"
              rows={1}
              value={draftTitle}
              onChange={(event) => edit("title", event.target.value.replace(/\n/g, " "))}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  event.currentTarget.form?.requestSubmit();
                }
              }}
              maxLength={200}
              required
              disabled={saving || hasUnsavedProgressUpdate || hasUnsavedDetails}
            />
            <label className="sr-only" htmlFor={`description-${task.id}`}>Description</label>
            <textarea
              id={`description-${task.id}`}
              className="task-page-body"
              aria-label="Edit description"
              value={draftDescription}
              onChange={(event) => edit("description", event.target.value)}
              rows={3}
              maxLength={20000}
              placeholder="Add notes..."
              disabled={saving || hasUnsavedProgressUpdate || hasUnsavedDetails}
            />
            {(hasUnsavedText || saving) && (
              <div className="editor-actions">
                <button
                  className="button button-primary button-small"
                  type="submit"
                  disabled={
                    !draftTitle.trim() || saving || hasUnsavedProgressUpdate || hasUnsavedDetails
                  }
                >
                  {saving && (
                    <LoaderCircle className="spin" size={13} aria-hidden="true" />
                  )}
                  {saving ? "Saving..." : "Save changes"}
                </button>
              </div>
            )}
          </form>

          <div className="task-properties" role="group" aria-label="Task status and priority">
            <label className="property" htmlFor={`status-${task.id}`}>
              <span className="sr-only">Work status</span>
              <span className={`task-check task-check--${task.status} task-check--small`} aria-hidden="true" />
              <select
                id={`status-${task.id}`}
                aria-label="Work status"
                value={task.status}
                disabled={!canChangeSelectors}
                onChange={(event) =>
                  void onChange({ status: event.target.value as WorkStatus }, task.revision)
                }
              >
                {STATUS_ORDER.map((status) => (
                  <option
                    key={status}
                    value={status}
                    disabled={status === "ready" && task.status !== "ready"}
                  >
                    {STATUS_VIEW[status].label}
                  </option>
                ))}
              </select>
            </label>
            <label className="property" htmlFor={`priority-${task.id}`}>
              <span className="sr-only">Priority</span>
              <PriorityIcon size={13} strokeWidth={2} aria-hidden="true" />
              <select
                id={`priority-${task.id}`}
                aria-label="Priority"
                value={task.priority}
                disabled={!canChangeSelectors}
                onChange={(event) =>
                  void onChange({ priority: event.target.value as Priority }, task.revision)
                }
              >
                {PRIORITY_ORDER.map((priority) => (
                  <option key={priority} value={priority}>
                    {PRIORITY_VIEW[priority].label}
                  </option>
                ))}
              </select>
            </label>
            <label className="property" htmlFor={`list-${task.id}`}>
              <span className="sr-only">List</span>
              <Folder size={13} aria-hidden="true" />
              <select id={`list-${task.id}`} aria-label="Task list"
                value={task.listId ?? ""} disabled={!canChangeSelectors}
                onChange={(event) => void onMove(event.target.value || null)}>
                <option value="">Inbox</option>
                {/* The sidebar loads Lists in pages; never fall back to showing Inbox. */}
                {task.listId && !lists.some((list) => list.id === task.listId) && (
                  <option value={task.listId} disabled>Current List (not loaded)</option>
                )}
                {lists.map((list) => (
                  <option value={list.id} key={list.id}>{list.name}</option>
                ))}
              </select>
            </label>
          </div>

          <div className="task-properties" aria-label="Task organization">
            <label className="property" htmlFor={`due-${task.id}`}>
              <span className="sr-only">Deadline</span>
              <CalendarDays size={13} aria-hidden="true" />
              <input id={`due-${task.id}`} aria-label="Deadline" type="date"
                value={draftDueDate} disabled={!canChangeDetails}
                onChange={(event) => edit("dueDate", event.target.value)} />
            </label>
            <label className="property property--wide" htmlFor={`tags-${task.id}`}>
              <span className="sr-only">Tags (comma-separated)</span>
              <Tag size={13} aria-hidden="true" />
              <input id={`tags-${task.id}`} aria-label="Tags" maxLength={640}
                placeholder="Tags, comma-separated" value={draftTags}
                disabled={!canChangeDetails}
                onChange={(event) => edit("tags", event.target.value)} />
            </label>
            {hasUnsavedDetails && (
              <div className="details-actions">
                <button type="button" className="button button-small"
                  disabled={saving} onClick={discardDetailsDraft}>
                  Discard tag and deadline changes
                </button>
                <button type="button" className="button button-primary button-small"
                  disabled={!canChangeDetails}
                  onClick={() => void saveDetails()}>Save tags and deadline</button>
              </div>
            )}
          </div>

          <div className="progress-editor" aria-label="Progress update">
            <div className="progress-line">
              <label className="field-label" htmlFor={`progress-${task.id}`}>Progress</label>
              <span className="progress-track" aria-hidden="true">
                <span style={{ transform: `scaleX(${(validProgress ? numericProgress : task.progress) / 100})` }} />
              </span>
              <span className="progress-number">
                <input
                  id={`progress-${task.id}`}
                  aria-label="Progress percentage"
                  type="number"
                  min="0"
                  max="100"
                  step="1"
                  value={draftProgress}
                  disabled={!canChangeMetadata}
                  onChange={(event) => edit("progress", event.target.value)}
                />
                %
              </span>
            </div>
            <label className="field-label" htmlFor={`next-step-${task.id}`}>Next step</label>
            <textarea
              id={`next-step-${task.id}`}
              className="next-step-input"
              aria-label="Next step"
              rows={1}
              maxLength={4000}
              placeholder="What will you do next?"
              value={draftNextStep}
              disabled={!canChangeMetadata}
              onChange={(event) => edit("nextStep", event.target.value)}
            />
            <label className="sr-only" htmlFor={`progress-note-${task.id}`}>Progress note</label>
            <textarea
              id={`progress-note-${task.id}`}
              className="progress-note-input"
              aria-label="Progress note"
              rows={2}
              maxLength={10000}
              placeholder="Add a progress note..."
              value={draftNote}
              disabled={!canChangeMetadata}
              onChange={(event) => edit("note", event.target.value)}
            />
            <div className="editor-actions">
              {hasUnsavedProgressUpdate && (
                <button
                  type="button"
                  className="button button-small"
                  onClick={discardUpdateDraft}
                >
                  {hasUnsavedNextStep || hasUnsavedNote
                    ? "Discard update draft"
                    : "Discard progress edit"}
                </button>
              )}
              <button
                className="button button-primary button-small"
                type="button"
                disabled={
                  !canChangeMetadata ||
                  !validProgress ||
                  !hasUnsavedProgressUpdate
                }
                onClick={() => void saveProgressUpdate()}
              >
                {progressButtonLabel}
              </button>
            </div>
          </div>

          {hasUnsavedText && (
            <p className="metadata-hint">
              Save title/description edits before changing status or priority.
            </p>
          )}
          {hasUnsavedDetails && (
            <p className="metadata-hint">Save tags and deadline before editing progress or another field.</p>
          )}
          {hasUnsavedProgressUpdate && (
            <p className="metadata-hint">
              Save or discard the progress update before changing another field.
            </p>
          )}
          <TaskLinkedNotes taskId={task.id} shown={shown} onOpenNote={onOpenNote} />
          <TaskProgressHistory taskId={task.id} revision={task.revision} />
          <div className="task-remove-row">
            <button type="button" className="button button-small" disabled={!canChangeSelectors}
              onClick={() => void onRemove()}>
              <Trash2 size={13} aria-hidden="true" /> Move to Removed
            </button>
          </div>
        </section>
      )}
    </article>
  );
}
