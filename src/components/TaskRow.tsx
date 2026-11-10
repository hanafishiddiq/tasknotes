import {
  useLayoutEffect, useRef, type ButtonHTMLAttributes, type FormEvent, type KeyboardEvent,
} from "react";
import { CalendarDays, Check, Folder, LoaderCircle, Tag, Trash2 } from "lucide-react";
import {
  checkActionOf,
  statusChoiceArgs,
  statusChoiceOf,
  type StatusChoice,
  type Task,
  type TaskList,
  type Priority,
  type Difficulty,
  type TaskChange,
  type ProgressDraft,
} from "../lib/task-api";
import TaskProgressHistory from "./TaskProgressHistory";
import TaskLinkedNotes from "./TaskLinkedNotes";
import { deadlineStatus } from "../lib/deadlines";
import type { EditableFields, UnsavedTaskEdits } from "../lib/use-task-drafts";
import {
  STATUS_CHOICES,
  STATUS_VIEW,
  statusChoiceLabel,
  PRIORITY_ORDER,
  PRIORITY_VIEW,
  DIFFICULTY_ORDER,
  DIFFICULTY_VIEW,
  NO_DIFFICULTY_ICON,
  WONT_DO_ICON,
  WONT_DO_LABEL,
  checkActionLabel,
} from "../lib/task-visuals";

/** Why a finished Task's Won't do choice is closed (ADR 0008). */
const FINISHED_CANNOT_BE_SET_ASIDE = "Reopen it first: a finished Task can't be set aside";

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
  /** The round check: completes, reopens or clears Won't Do, as `checkActionOf` says. */
  onCheck: () => Promise<void>;
  /** False while the Notes workspace covers the Task view. */
  shown: boolean;
  /** What was typed and not saved, kept above the row so it outlives it. */
  edits: UnsavedTaskEdits | undefined;
  onEdits: (edits: UnsavedTaskEdits | undefined) => void;
  /** Alt+Up/Down (and a drag) move it within its group of the List's Custom Order. */
  reorderable?: boolean;
}

/** ⌘↵, with Ctrl+Return as a quiet alias (as in use-shortcuts.ts): save the edit group the key was pressed in. */
function isSaveShortcut(event: KeyboardEvent): boolean {
  return event.key === "Enter" && (event.metaKey || event.ctrlKey) && !event.altKey &&
    !event.shiftKey && !event.nativeEvent.isComposing;
}

/** A group's key handler: ⌘↵ runs that group's own save, which decides whether there is anything to save. */
function saveOnShortcut<Group extends HTMLElement>(save: (event: KeyboardEvent<Group>) => void) {
  return (event: KeyboardEvent<Group>) => {
    if (!isSaveShortcut(event)) return;
    event.preventDefault();
    save(event);
  };
}

interface SaveButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** The id of the hidden "Command Return" description. */
  shortcutDescription: string;
  /** While it reads "Saving...", the button drops its ⌘↵ hint. */
  busy?: boolean;
}

/**
 * A Task edit group's Save button: its label, then a quiet ⌘↵. Assistive tech
 * hears "Command Return" through the description, not as part of the name.
 */
function SaveButton({ shortcutDescription, busy = false, children, ...button }: SaveButtonProps) {
  return (
    <button className="button button-primary button-small" {...button}
      aria-keyshortcuts="Meta+Enter" aria-describedby={shortcutDescription}>
      {children}
      {!busy && <kbd className="save-shortcut" aria-hidden="true">⌘↵</kbd>}
    </button>
  );
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
  onCheck,
  shown,
  edits,
  onEdits,
  reorderable = false,
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
  // Older replies may lack the field; treat that as No Difficulty.
  const difficulty = task.difficulty ?? null;
  const DifficultyIcon = difficulty ? DIFFICULTY_VIEW[difficulty].icon : NO_DIFFICULTY_ICON;
  // Older replies may lack the field; treat that as not Won't Do.
  const wontDo = task.wontDo === true;
  // A Won't Do Task's check is its own, so the finished tick never shows on it.
  const checkKind = wontDo ? "wont-do" : task.status;
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
  // The check changes the status, so it waits for typed edits and a save in flight like the status control.
  const canCheck = !saving && !dirty;
  const numericProgress =
    draftProgress.trim() === "" ? NaN : Number(draftProgress);
  const validProgress =
    Number.isInteger(numericProgress) &&
    numericProgress >= 0 &&
    numericProgress <= 100;
  const shortcutDescription = `save-shortcut-${task.id}`;
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
      className={`task-card task-card--${task.status} ${wontDo ? "task-card--wont-do" : ""} ${expanded ? "task-card--expanded" : ""}`}
      aria-label={task.title}
    >
      <div className="task-row">
        {/* Outside the open button, so a click never opens the row or starts a drag. */}
        <button
          type="button"
          className={`task-check task-check--${checkKind}`}
          aria-label={checkActionLabel(checkActionOf(task), task.title)}
          disabled={!canCheck}
          onClick={() => void onCheck()}
        >
          {wontDo
            ? <WONT_DO_ICON size={11} strokeWidth={3} aria-hidden="true" />
            : task.status === "completed" && <Check size={11} strokeWidth={3} aria-hidden="true" />}
        </button>
        <button
          className="task-open"
          type="button"
          onClick={onToggle}
          aria-label={`${expanded ? "Close" : "Open"} ${task.title}`}
          aria-expanded={expanded}
          aria-keyshortcuts={reorderable ? "Alt+ArrowUp Alt+ArrowDown" : undefined}
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
          {/* Won't Do keeps its progress, so the row shows it. */}
          {wontDo && <span className="wont-do-label">{WONT_DO_LABEL} · {task.progress}%</span>}
          <span className={`priority-pill priority-pill--${task.priority}`}>
            <PriorityIcon size={13} strokeWidth={2} aria-hidden="true" />
            {PRIORITY_VIEW[task.priority].label}
          </span>
          {difficulty && (
            <span className={`difficulty-pill difficulty-pill--${difficulty}`}>
              <DifficultyIcon size={13} strokeWidth={2} aria-hidden="true" />
              <span className="sr-only">Difficulty </span>
              {DIFFICULTY_VIEW[difficulty].label}
            </span>
          )}
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
          <span id={shortcutDescription} hidden>Command Return</span>
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
          <form className="task-page" onSubmit={save}
            onKeyDown={saveOnShortcut<HTMLFormElement>((event) => {
              if (hasUnsavedText) event.currentTarget.requestSubmit();
            })}>
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
                // ⌘↵ is left to the form, which saves only when something changed.
                if (event.key === "Enter" && !isSaveShortcut(event) && !event.nativeEvent.isComposing) {
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
                <SaveButton
                  type="submit"
                  shortcutDescription={shortcutDescription}
                  busy={saving}
                  disabled={
                    !draftTitle.trim() || saving || hasUnsavedProgressUpdate || hasUnsavedDetails
                  }
                >
                  {saving && (
                    <LoaderCircle className="spin" size={13} aria-hidden="true" />
                  )}
                  {saving ? "Saving..." : "Save changes"}
                </SaveButton>
              </div>
            )}
          </form>

          <div className="task-properties" role="group" aria-label="Task status and priority">
            <label className="property" htmlFor={`status-${task.id}`}>
              <span className="sr-only">Work status</span>
              <span className={`task-check task-check--${checkKind} task-check--small`} aria-hidden="true" />
              <select
                id={`status-${task.id}`}
                aria-label="Work status"
                value={statusChoiceOf(task)}
                disabled={!canChangeSelectors}
                onChange={(event) =>
                  void onChange(statusChoiceArgs(event.target.value as StatusChoice), task.revision)
                }
              >
                {STATUS_CHOICES.map((choice) => {
                  // Ready is for work not yet started, which a Task set aside before it started still is.
                  const readyClosed = choice === "ready" && task.status !== "ready"
                    && !(wontDo && task.wontDoFrom === "ready");
                  // A finished Task is reopened first; the core refuses it too.
                  const finishedNote = choice === "wont_do" && task.status === "completed" && !wontDo
                    ? FINISHED_CANNOT_BE_SET_ASIDE : undefined;
                  return (
                    <option
                      key={choice}
                      value={choice}
                      disabled={readyClosed || finishedNote !== undefined}
                      title={finishedNote}
                      aria-description={finishedNote}
                    >
                      {statusChoiceLabel(choice)}
                    </option>
                  );
                })}
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
            <label className="property" htmlFor={`difficulty-${task.id}`}>
              <span className="sr-only">Difficulty</span>
              <DifficultyIcon size={13} strokeWidth={2} aria-hidden="true" />
              <select
                id={`difficulty-${task.id}`}
                aria-label="Difficulty"
                value={difficulty ?? ""}
                disabled={!canChangeSelectors}
                onChange={(event) =>
                  void onChange(
                    { difficulty: (event.target.value || null) as Difficulty | null },
                    task.revision,
                  )
                }
              >
                <option value="">No difficulty</option>
                {DIFFICULTY_ORDER.map((level) => (
                  <option key={level} value={level}>
                    {DIFFICULTY_VIEW[level].label}
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

          <div className="task-properties" role="group" aria-label="Task organization"
            onKeyDown={saveOnShortcut(() => void saveDetails())}>
            <label className="property" htmlFor={`due-${task.id}`}>
              <span className="sr-only">Deadline</span>
              <CalendarDays size={13} aria-hidden="true" />
              <input id={`due-${task.id}`} aria-label="Deadline" type="date"
                className={draftDueDate ? undefined : "property-date--empty"}
                aria-describedby={draftDueDate ? undefined : `no-due-${task.id}`}
                value={draftDueDate} disabled={!canChangeDetails}
                onChange={(event) => edit("dueDate", event.target.value)} />
              {!draftDueDate && (
                <span id={`no-due-${task.id}`} className="property-date-empty-label">No deadline</span>
              )}
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
                <SaveButton type="button" shortcutDescription={shortcutDescription}
                  disabled={!canChangeDetails} onClick={() => void saveDetails()}>
                  Save tags and deadline
                </SaveButton>
              </div>
            )}
          </div>

          <div className="progress-editor" role="group" aria-label="Progress update"
            onKeyDown={saveOnShortcut(() => void saveProgressUpdate())}>
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
              <SaveButton
                type="button"
                shortcutDescription={shortcutDescription}
                disabled={
                  !canChangeMetadata ||
                  !validProgress ||
                  !hasUnsavedProgressUpdate
                }
                onClick={() => void saveProgressUpdate()}
              >
                {progressButtonLabel}
              </SaveButton>
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
