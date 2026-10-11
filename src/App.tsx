import { useEffect, useState, type FormEvent } from "react";
import {
  ArrowRight,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Circle,
  Clock3,
  Inbox,
  LoaderCircle,
  LockKeyhole,
  Plus,
  Sparkles,
} from "lucide-react";
import { taskApi, type Task } from "./lib/task-api";
import "./App.css";

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong. Please try again.";
}

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
}

function TaskRow({ task, expanded, saving, onToggle, onSave }: TaskRowProps) {
  const [draftTitle, setDraftTitle] = useState(task.title);
  const [draftDescription, setDraftDescription] = useState(task.description);

  function toggle() {
    if (!expanded) {
      setDraftTitle(task.title);
      setDraftDescription(task.description);
    }
    onToggle();
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draftTitle.trim() || saving) return;
    await onSave(draftTitle, draftDescription);
  }

  return (
    <article className={`task-card ${expanded ? "task-card--expanded" : ""}`} aria-label={task.title}>
      <div className="task-row">
        <span className="task-circle" aria-hidden="true"><Circle size={18} strokeWidth={1.8} /></span>
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
          <span className="status-pill"><span className="status-dot" />Ready</span>
          <span className="priority-pill">Medium</span>
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
              disabled={saving}
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
              disabled={saving}
            />
            <div className="editor-actions">
              <span className="editor-note"><LockKeyhole size={13} aria-hidden="true" /> Saved only on this Mac</span>
              <button className="button button-primary button-small" type="submit" disabled={!draftTitle.trim() || saving}>
                {saving && <LoaderCircle className="spin" size={15} aria-hidden="true" />}
                {saving ? "Saving..." : "Save changes"}
              </button>
            </div>
          </form>
        </section>
      )}
    </article>
  );
}

export default function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [creating, setCreating] = useState(false);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    taskApi
      .list()
      .then((page) => {
        if (!active) return;
        setTasks(page.items);
        setNextCursor(page.nextCursor);
        setLoadError(false);
      })
      .catch((reason) => {
        if (!active) return;
        setLoadError(true);
        setError(errorMessage(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => { active = false; };
  }, [loadAttempt]);

  function retryFirstLoad() {
    setError(null);
    setLoadError(false);
    setLoading(true);
    setLoadAttempt((attempt) => attempt + 1);
  }

  async function createTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim() || creating || loading || loadError) return;
    setCreating(true);
    setError(null);
    try {
      const task = await taskApi.create({ title, description });
      setTasks((current) => [task, ...current]);
      setTitle("");
      setDescription("");
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setCreating(false);
    }
  }

  async function updateTask(task: Task, nextTitle: string, nextDescription: string) {
    setSavingId(task.id);
    setError(null);
    try {
      const updated = await taskApi.update({
        id: task.id,
        title: nextTitle,
        description: nextDescription,
        expectedRevision: task.revision,
      });
      setTasks((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      setExpandedId(null);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setSavingId(null);
    }
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) return;
    setLoadingMore(true);
    setError(null);
    try {
      const page = await taskApi.list(nextCursor);
      setTasks((current) => {
        const knownIds = new Set(current.map((task) => task.id));
        return [...current, ...page.items.filter((task) => !knownIds.has(task.id))];
      });
      setNextCursor(page.nextCursor);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><CheckCircle2 size={22} strokeWidth={2.3} /></div>
          <div><strong>tasknotes</strong><span>Make room for progress.</span></div>
        </div>

        <div className="sidebar-section-title">WORKSPACE</div>
        <nav aria-label="Workspace">
          <div className="nav-item nav-item--active" aria-current="page">
            <Inbox size={18} strokeWidth={1.8} aria-hidden="true" />
            <span>All tasks</span>
            <span className="nav-count">{tasks.length}{nextCursor ? "+" : ""}</span>
          </div>
        </nav>

        <div className="sidebar-spacer" />
        <div className="local-note">
          <span className="local-note-icon"><LockKeyhole size={15} /></span>
          <div><strong>Private by design</strong><span>Your tasks live on this Mac.</span></div>
        </div>
        <span className="sidebar-version">TASKNOTES · EARLY PREVIEW</span>
      </aside>

      <main className="main-view">
        <header className="top-bar">
          <div className="top-location"><span>Workspace</span><ChevronDown size={14} className="breadcrumb-icon" /><strong>All tasks</strong></div>
          <span className="local-badge"><span /> Local storage</span>
        </header>

        <div className="content-wrap">
          <div className="hero-intro">
            <span className="eyebrow"><Sparkles size={14} aria-hidden="true" /> YOUR SPACE TO FOCUS</span>
            <h1>All tasks</h1>
            <p>A quieter place to keep track of what matters.</p>
          </div>

          <section className="composer" aria-label="Create a task">
            <div className="composer-top">
              <span className="composer-icon"><Plus size={17} aria-hidden="true" /></span>
              <div>
                <h2>Capture a task</h2>
                <p>Start small. The details can follow.</p>
              </div>
            </div>
            <form onSubmit={createTask}>
              <label className="sr-only" htmlFor="new-task-title">Task title</label>
              <input
                id="new-task-title"
                className="new-task-title"
                aria-label="Task title"
                placeholder="What needs to get done?"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                maxLength={200}
                disabled={creating || loading || loadError}
                required
              />
              <label className="sr-only" htmlFor="new-task-desc">Description (optional)</label>
              <textarea
                id="new-task-desc"
                aria-label="Description (optional)"
                placeholder="A little context, if you like..."
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                rows={2}
                maxLength={20000}
                disabled={creating || loading || loadError}
              />
              <div className="composer-footer">
                <span className="composer-hint"><Clock3 size={14} aria-hidden="true" /> Ready to work on later</span>
                <button className="button button-primary" type="submit" disabled={!title.trim() || creating || loading || loadError}>
                  {creating ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />}
                  {creating ? "Adding..." : "Add task"}
                </button>
              </div>
            </form>
          </section>

          {error && <div role="alert" className="error-banner">{error}</div>}

          <section className="task-section" aria-label="Your tasks">
            <div className="list-heading">
              <div><h2>Your tasks</h2><p>{tasks.length}{nextCursor ? "+" : ""} currently shown</p></div>
              <span className="section-indicator"><span /> Ready</span>
            </div>

            {loading ? (
              <div className="loading-state" role="status"><LoaderCircle className="spin" size={20} /> Loading your tasks...</div>
            ) : loadError && tasks.length === 0 ? (
              <div className="empty-state" aria-label="Tasks unavailable">
                <div className="empty-icon"><LockKeyhole size={29} aria-hidden="true" /></div>
                <h3>Couldn't load your tasks</h3>
                <p>The local database couldn't be opened. Nothing has been changed; check the message above and try again.</p>
                <button className="button button-more retry-button" type="button" onClick={retryFirstLoad}>Try again</button>
              </div>
            ) : tasks.length === 0 ? (
              <div className="empty-state">
                <div className="empty-icon"><CheckCircle2 size={31} strokeWidth={1.4} aria-hidden="true" /></div>
                <h3>Your list is clear</h3>
                <p>Every great plan starts with a single step. Add your first task above whenever you're ready.</p>
                <ArrowRight size={19} className="empty-arrow" aria-hidden="true" />
              </div>
            ) : (
              <div className="task-list">
                {tasks.map((task) => (
                  <TaskRow
                    key={task.id}
                    task={task}
                    expanded={expandedId === task.id}
                    saving={savingId === task.id}
                    onToggle={() => setExpandedId((current) => current === task.id ? null : task.id)}
                    onSave={(nextTitle, nextDescription) => updateTask(task, nextTitle, nextDescription)}
                  />
                ))}
                {nextCursor && (
                  <button className="button button-more" type="button" disabled={loadingMore} onClick={loadMore}>
                    {loadingMore ? "Loading..." : "Load more tasks"}
                    {loadingMore && <LoaderCircle className="spin" size={16} />}
                  </button>
                )}
              </div>
            )}
          </section>
        </div>

        <footer className="footer-note"><LockKeyhole size={13} aria-hidden="true" /> Local-first. No account needed.</footer>
      </main>
    </div>
  );
}
