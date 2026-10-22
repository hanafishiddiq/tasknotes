import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { CalendarDays, Check, Folder, LoaderCircle, Tag } from "lucide-react";
import type {
  Task,
  TaskList,
  WorkStatus,
  Priority,
  TaskChange,
  ProgressDraft,
} from "../lib/task-api";
import TaskProgressHistory from "./TaskProgressHistory";
import { deadlineStatus } from "../lib/deadlines";
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
  onSave: (title: string, description: string) => Promise<void>;
  onChange: (change: TaskChange) => Promise<void>;
  onRecordProgress: (update: ProgressDraft) => Promise<void>;
  lists: TaskList[];
  today: string;
  onMove: (listId: string | null) => Promise<void>;
  onUpdateDetails: (tags: string[], dueDate: string | null) => Promise<void>;
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
}: TaskRowProps) {
  const [draftTitle, setDraftTitle] = useState(task.title);
  const [draftDescription, setDraftDescription] = useState(task.description);
  const [draftProgress, setDraftProgress] = useState(String(task.progress));
  const [draftNextStep, setDraftNextStep] = useState(task.nextStep ?? "");
  const [draftNote, setDraftNote] = useState("");
  const [draftTags, setDraftTags] = useState((task.tags ?? []).join(", "));
  const [draftDueDate, setDraftDueDate] = useState(task.dueDate ?? "");
  const PriorityIcon = PRIORITY_VIEW[task.priority].icon;
  const titleField = useRef<HTMLTextAreaElement>(null);
  const due = deadlineStatus(task.dueDate, today, task.status);
  const hasUnsavedText =
    draftTitle !== task.title || draftDescription !== task.description;
  const hasUnsavedProgress = draftProgress !== String(task.progress);
  const hasUnsavedNextStep = draftNextStep !== (task.nextStep ?? "");
  const hasUnsavedNote = draftNote.trim().length > 0;
  const hasUnsavedProgressUpdate =
    hasUnsavedProgress || hasUnsavedNextStep || hasUnsavedNote;
  const hasUnsavedDetails =
    draftTags !== (task.tags ?? []).join(", ") ||
    draftDueDate !== (task.dueDate ?? "");
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
  useEffect(() => {
    setDraftProgress(String(task.progress));
    setDraftNextStep(task.nextStep ?? "");
    setDraftNote("");
    setDraftTags((task.tags ?? []).join(", "));
    setDraftDueDate(task.dueDate ?? "");
  }, [task.progress, task.nextStep, task.revision, task.dueDate, task.tags]);

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

  function toggle() {
    if (!expanded) {
      setDraftTitle(task.title);
      setDraftDescription(task.description);
      setDraftProgress(String(task.progress));
      setDraftNextStep(task.nextStep ?? "");
      setDraftNote("");
      setDraftTags((task.tags ?? []).join(", "));
      setDraftDueDate(task.dueDate ?? "");
    }
    onToggle();
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draftTitle.trim() || saving || hasUnsavedProgressUpdate || hasUnsavedDetails) return;
    await onSave(draftTitle, draftDescription);
  }

  async function saveProgressUpdate() {
    if (!canChangeMetadata || !hasUnsavedProgressUpdate || !validProgress)
      return;
    // Keep the older simple button behavior for percentage-only updates.
    if (!hasUnsavedNextStep && !hasUnsavedNote) {
      await onChange({ progress: numericProgress });
      return;
    }
    await onRecordProgress({
      ...(hasUnsavedProgress ? { progress: numericProgress } : {}),
      ...(hasUnsavedNextStep ? { nextStep: draftNextStep } : {}),
      ...(hasUnsavedNote ? { note: draftNote.trim() } : {}),
    });
  }

  async function saveDetails() {
    if (!canChangeDetails || !hasUnsavedDetails) return;
    const tags = draftTags.split(",").map((value) => value.trim()).filter(Boolean);
    await onUpdateDetails(tags, draftDueDate || null);
  }

  function discardUpdateDraft() {
    setDraftProgress(String(task.progress));
    setDraftNextStep(task.nextStep ?? "");
    setDraftNote("");
  }

  const listName = lists.find((list) => list.id === task.listId)?.name ?? "Inbox";
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
          onClick={toggle}
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
          <form className="task-page" onSubmit={save}>
            <label className="sr-only" htmlFor={`title-${task.id}`}>Title</label>
            <textarea
              ref={titleField}
              id={`title-${task.id}`}
              className="task-page-title"
              aria-label="Edit title"
              rows={1}
              value={draftTitle}
              onChange={(event) => setDraftTitle(event.target.value.replace(/\n/g, " "))}
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
              onChange={(event) => setDraftDescription(event.target.value)}
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
                  void onChange({ status: event.target.value as WorkStatus })
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
                  void onChange({ priority: event.target.value as Priority })
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
                onChange={(event) => setDraftDueDate(event.target.value)} />
            </label>
            <label className="property property--wide" htmlFor={`tags-${task.id}`}>
              <span className="sr-only">Tags (comma-separated)</span>
              <Tag size={13} aria-hidden="true" />
              <input id={`tags-${task.id}`} aria-label="Tags" maxLength={640}
                placeholder="Tags, comma-separated" value={draftTags}
                disabled={!canChangeDetails}
                onChange={(event) => setDraftTags(event.target.value)} />
            </label>
            {hasUnsavedDetails && (
              <button type="button" className="button button-primary button-small"
                disabled={!canChangeDetails}
                onClick={() => void saveDetails()}>Save tags and deadline</button>
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
                  onChange={(event) => setDraftProgress(event.target.value)}
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
              onChange={(event) => setDraftNextStep(event.target.value)}
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
              onChange={(event) => setDraftNote(event.target.value)}
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
          <TaskProgressHistory taskId={task.id} revision={task.revision} />
        </section>
      )}
    </article>
  );
}
