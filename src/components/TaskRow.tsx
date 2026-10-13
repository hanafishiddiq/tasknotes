import { useEffect, useState, type FormEvent } from "react";
import { ChevronDown, ChevronUp, LoaderCircle, LockKeyhole } from "lucide-react";
import type { Task, WorkStatus, Priority, TaskChange } from "../lib/task-api";
import { STATUS_ORDER, STATUS_VIEW, PRIORITY_ORDER, PRIORITY_VIEW } from "../lib/task-visuals";

const dateFormatter = new Intl.DateTimeFormat("en", {
  month: "short",
  day: "numeric",
});

interface TaskRowProps {
  task: Task;
  expanded: boolean;
  saving: boolean;
  onToggle: () => void;
  onSave: (title: string, description: string) => Promise<void>;
  onChange: (change: TaskChange) => Promise<void>;
}

export default function TaskRow({ task, expanded, saving, onToggle, onSave, onChange }: TaskRowProps) {
  const [draftTitle, setDraftTitle] = useState(task.title);
  const [draftDescription, setDraftDescription] = useState(task.description);
  const [draftProgress, setDraftProgress] = useState(String(task.progress));
  const StatusIcon = STATUS_VIEW[task.status].icon;
  const PriorityIcon = PRIORITY_VIEW[task.priority].icon;
  const hasUnsavedText = draftTitle !== task.title || draftDescription !== task.description;
  const hasUnsavedProgress = draftProgress !== String(task.progress);
  const canChangeMetadata = !saving && !hasUnsavedText;
  const canChangeSelectors = canChangeMetadata && !hasUnsavedProgress;
  const numericProgress = draftProgress.trim() === "" ? NaN : Number(draftProgress);
  const validProgress = Number.isInteger(numericProgress) && numericProgress >= 0 && numericProgress <= 100;
  useEffect(() => {
    setDraftProgress(String(task.progress));
  }, [task.progress]);

  function toggle() {
    if (!expanded) {
      setDraftTitle(task.title);
      setDraftDescription(task.description);
      setDraftProgress(String(task.progress));
    }
    onToggle();
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draftTitle.trim() || saving || hasUnsavedProgress) return;
    await onSave(draftTitle, draftDescription);
  }

  return (
    <article className={`task-card ${expanded ? "task-card--expanded" : ""}`} aria-label={task.title}>
      <div className="task-row">
        <span className={`task-circle status-color--${task.status}`} aria-hidden="true"><StatusIcon size={18} strokeWidth={1.8} /></span>
        <button
          className="task-open"
          type="button"
          onClick={toggle}
          aria-label={`${expanded ? "Close" : "Open"} ${task.title}`}
          aria-expanded={expanded}
        >
          <span className="task-row-title">{task.title}</span>
          <span className="task-row-subtitle">
            {task.description
              ? <span className="task-summary">{task.description}</span>
              : <span>Created {dateFormatter.format(new Date(task.createdAt))}</span>}
          </span>
        </button>
        <div className="task-row-meta" aria-label="Task details summary">
          <span className={`status-pill status-pill--${task.status}`}><StatusIcon size={12} aria-hidden="true" />{STATUS_VIEW[task.status].label}</span>
          <span className={`priority-pill priority-pill--${task.priority}`}><PriorityIcon size={12} aria-hidden="true" />{PRIORITY_VIEW[task.priority].label}</span>
          <span className="progress-readout" aria-label={`Progress ${task.progress}%`}>{task.progress}%</span>
          <button
            className="expand-icon"
            type="button"
            onClick={toggle}
            aria-label={`${expanded ? "Collapse" : "Expand"} task details`}
          >
            {expanded ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
          </button>
        </div>
      </div>

      {expanded && (
        <section role="region" aria-label="Task details" className="task-editor">
          <form onSubmit={save}>
            <label htmlFor={`title-${task.id}`}>Title</label>
            <input
              id={`title-${task.id}`}
              aria-label="Edit title"
              value={draftTitle}
              onChange={(event) => setDraftTitle(event.target.value)}
              maxLength={200}
              required
              disabled={saving || hasUnsavedProgress}
            />
            <label htmlFor={`description-${task.id}`}>Description</label>
            <textarea
              id={`description-${task.id}`}
              aria-label="Edit description"
              value={draftDescription}
              onChange={(event) => setDraftDescription(event.target.value)}
              rows={3}
              maxLength={20000}
              placeholder="Add a little more context..."
              disabled={saving || hasUnsavedProgress}
            />
            <div className="editor-actions">
              <span className="editor-note"><LockKeyhole size={13} aria-hidden="true" /> Saved only on this Mac</span>
              <button className="button button-primary button-small" type="submit" disabled={!draftTitle.trim() || saving || hasUnsavedProgress}>
                {saving && <LoaderCircle className="spin" size={15} aria-hidden="true" />}
                {saving ? "Saving..." : "Save changes"}
              </button>
            </div>
          </form>
          <div className="task-controls" role="group" aria-label="Task status and priority">
            <label htmlFor={`status-${task.id}`}>
              Work status
              <select
                id={`status-${task.id}`}
                aria-label="Work status"
                value={task.status}
                disabled={!canChangeSelectors}
                onChange={(event) => void onChange({ status: event.target.value as WorkStatus })}
              >
                {STATUS_ORDER.map((status) => (
                  <option key={status} value={status} disabled={status === "ready" && task.status !== "ready"}>
                    {STATUS_VIEW[status].label}
                  </option>
                ))}
              </select>
            </label>
            <label htmlFor={`priority-${task.id}`}>
              Priority
              <select
                id={`priority-${task.id}`}
                aria-label="Priority"
                value={task.priority}
                disabled={!canChangeSelectors}
                onChange={(event) => void onChange({ priority: event.target.value as Priority })}
              >
                {PRIORITY_ORDER.map((priority) => (
                  <option key={priority} value={priority}>{PRIORITY_VIEW[priority].label}</option>
                ))}
              </select>
            </label>
            <label htmlFor={`progress-${task.id}`}>
              Progress (%)
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
            </label>
            <button
              className="button button-secondary button-small"
              type="button"
              disabled={!canChangeMetadata || !validProgress || numericProgress === task.progress}
              onClick={() => void onChange({ progress: numericProgress })}
            >
              Apply progress
            </button>
          </div>
          {hasUnsavedText && <p className="metadata-hint">Save title/description edits before changing status or priority.</p>}
          {hasUnsavedProgress && <div className="metadata-hint">
            Apply or discard the progress edit before changing another field.
            <button type="button" className="text-button" onClick={() => setDraftProgress(String(task.progress))}>Discard progress edit</button>
          </div>}
        </section>
      )}
    </article>
  );
}
