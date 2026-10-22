import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  ArrowDownWideNarrow, CheckCircle2, LoaderCircle, LockKeyhole, SquarePen,
} from "lucide-react";
import {
  taskApi, TASK_PAGE_SIZE, type Task, type TaskList, type TaskView,
  type SortMode, type TaskChange, type ProgressDraft,
} from "./lib/task-api";
import { STATUS_ORDER, STATUS_VIEW } from "./lib/task-visuals";
import TaskRow from "./components/TaskRow";
import TaskSidebar from "./components/TaskSidebar";
import { useLocalCalendarDay } from "./lib/use-local-calendar-day";
import { useTheme } from "./lib/use-theme";
import "./App.css";

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong. Please try again.";
}

export default function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [view, setView] = useState<TaskView>({ kind: "all" });
  const [lists, setLists] = useState<TaskList[]>([]);
  const [listCursor, setListCursor] = useState<string | null>(null);
  const [busyList, setBusyList] = useState(false);
  const [loadingLists, setLoadingLists] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [sort, setSort] = useState<SortMode>("priority");
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [currentPageCursor, setCurrentPageCursor] = useState<string | null>(null);
  const [previousPageCursors, setPreviousPageCursors] = useState<(string | null)[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [refreshError, setRefreshError] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [creating, setCreating] = useState(false);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [theme, toggleTheme] = useTheme();
  const titleInput = useRef<HTMLInputElement>(null);
  // Ignore stale results from older Task queries after a newer user action.
  const requestGeneration = useRef(0);
  const currentQuery = useRef({ sort, view });

  useEffect(() => {
    currentQuery.current = { sort, view };
    const generation = ++requestGeneration.current;
    let active = true;
    taskApi
      .list(null, sort, view)
      .then((page) => {
        if (!active || generation !== requestGeneration.current) return;
        setTasks(page.items);
        setCurrentPageCursor(null);
        setPreviousPageCursors([]);
        setNextCursor(page.nextCursor);
        setLoadError(false);
        setRefreshError(false);
        setError(null);
      })
      .catch((reason) => {
        if (!active || generation !== requestGeneration.current) return;
        setLoadError(true);
        setError(errorMessage(reason));
      })
      .finally(() => {
        if (active && generation === requestGeneration.current) setLoading(false);
      });
    return () => { active = false; };
  }, [loadAttempt, sort, view]);

  useEffect(() => {
    let active = true;
    taskApi.lists().then((page) => {
      if (!active) return;
      setLists(page.items);
      setListCursor(page.nextCursor);
    }).catch((reason) => {
      if (active) setListError("Could not load Lists: " + errorMessage(reason));
    });
    return () => { active = false; };
  }, []);

  function resetTaskPages() {
    requestGeneration.current += 1;
    setCurrentPageCursor(null);
    setPreviousPageCursors([]);
  }

  function navigate(next: TaskView) {
    if (busyList || creating || savingId || loadingMore) return;
    if (next.kind === view.kind &&
        (next.kind !== "list" || (view.kind === "list" && next.listId === view.listId))) return;
    setTasks([]);
    setNextCursor(null);
    resetTaskPages();
    setExpandedId(null);
    setLoading(true);
    setLoadError(false);
    setError(null);
    currentQuery.current = { ...currentQuery.current, view: next };
    setView(next);
  }

  async function refreshLists() {
    const page = await taskApi.lists();
    setLists(page.items);
    setListCursor(page.nextCursor);
    setListError(null);
  }

  async function createList(name: string): Promise<boolean> {
    if (busyList) return false;
    setBusyList(true);
    setError(null);
    try {
      const created = await taskApi.createList(name);
      setLists((before) => [created, ...before]);
      // This is a successful mutation, not a user navigation. busyList is still true.
      setTasks([]);
      setNextCursor(null);
      resetTaskPages();
      setExpandedId(null);
      setLoading(true);
      setLoadError(false);
      const newView: TaskView = { kind: "list", listId: created.id };
      currentQuery.current = { ...currentQuery.current, view: newView };
      setView(newView);
      try { await refreshLists(); } catch (reason) {
        setError("List created, but the sidebar could not refresh: " + errorMessage(reason));
      }
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      return false;
    } finally {
      setBusyList(false);
    }
  }

  async function renameList(list: TaskList, name: string): Promise<boolean> {
    setBusyList(true);
    setError(null);
    try {
      const updated = await taskApi.renameList({ id: list.id, expectedRevision: list.revision, name });
      setLists((all) => all.map((item) => item.id === list.id ? updated : item));
      try { await refreshLists(); } catch (reason) {
        setError("List renamed, but the sidebar could not refresh: " + errorMessage(reason));
      }
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      return false;
    } finally {
      setBusyList(false);
    }
  }

  async function setListRemoved(list: TaskList, removed: boolean) {
    setBusyList(true);
    setError(null);
    try {
      const updated = await taskApi.setListRemoved({
        id: list.id, expectedRevision: list.revision, removed,
      });
      setLists((all) => all.map((item) => item.id === list.id ? updated : item));
      if (removed && view.kind === "list" && view.listId === list.id) {
        setTasks([]);
        setNextCursor(null);
        resetTaskPages();
        setLoading(true);
        const newView: TaskView = { kind: "all" };
        currentQuery.current = { ...currentQuery.current, view: newView };
        setView(newView);
      } else {
        retryFirstLoad();
      }
      try { await refreshLists(); } catch (reason) {
        setError("List updated, but the sidebar could not refresh: " + errorMessage(reason));
      }
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setBusyList(false); }
  }

  async function loadMoreLists() {
    if (!listCursor || loadingLists) return;
    setLoadingLists(true);
    try {
      const page = await taskApi.lists(listCursor);
      setLists((current) => {
        const seen = new Set(current.map((list) => list.id));
        return [...current, ...page.items.filter((list) => !seen.has(list.id))];
      });
      setListCursor(page.nextCursor);
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setLoadingLists(false); }
  }

  function retryFirstLoad() {
    setError(null);
    setLoadError(false);
    setRefreshError(false);
    setLoading(true);
    resetTaskPages();
    setLoadAttempt((attempt) => attempt + 1);
  }

  async function createTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim() || creating || savingId || loading || loadError) return;
    setCreating(true);
    setError(null);
    try {
      const created = await taskApi.create(
        { title, description },
        view.kind === "list" ? view.listId : undefined,
      );
      const generation = ++requestGeneration.current;
      const { sort: latestSort, view: latestView } = currentQuery.current;
      // Clear only after the actual write succeeds; retry must not create a
      // duplicate if the subsequent read fails.
      setTitle("");
      setDescription("");
      try {
        const page = await taskApi.list(null, latestSort, latestView);
        if (generation !== requestGeneration.current) return;
        setLoading(false);
        setTasks(page.items);
        setCurrentPageCursor(null);
        setPreviousPageCursors([]);
        setNextCursor(page.nextCursor);
        setRefreshError(false);
      } catch (reason) {
        if (generation !== requestGeneration.current) return;
        setLoading(false);
        setTasks((current) =>
          [created, ...current.filter((item) => item.id !== created.id)].slice(0, TASK_PAGE_SIZE),
        );
        setNextCursor(null);
        resetTaskPages();
        setRefreshError(true);
        setError("Task created, but the ordered list could not refresh: " + errorMessage(reason));
      }
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
      requestGeneration.current += 1;
      setTasks((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      setExpandedId(null);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setSavingId(null);
    }
  }

  async function commitTaskMutation(task: Task, mutate: () => Promise<Task>) {
    if (savingId) return;
    setSavingId(task.id);
    setError(null);
    try {
      const updated = await mutate();
      const generation = ++requestGeneration.current;
      const { sort: latestSort, view: latestView } = currentQuery.current;
      setTasks((current) => current.map((item) => item.id === updated.id ? updated : item));
      // The mutation already succeeded; never let an old page replace its result.
      try {
        const page = await taskApi.list(null, latestSort, latestView);
        if (generation !== requestGeneration.current) return;
        setLoading(false);
        setTasks(page.items);
        setCurrentPageCursor(null);
        setPreviousPageCursors([]);
        setNextCursor(page.nextCursor);
        setRefreshError(false);
      } catch (reason) {
        if (generation !== requestGeneration.current) return;
        setLoading(false);
        setNextCursor(null);
        resetTaskPages();
        setRefreshError(true);
        setError("Change saved, but the ordered list could not refresh: " + errorMessage(reason));
      }
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setSavingId(null);
    }
  }

  async function changeTask(task: Task, change: TaskChange) {
    await commitTaskMutation(task, () => taskApi.change({
      id: task.id,
      expectedRevision: task.revision,
      ...change,
    }));
  }

  async function recordProgress(task: Task, update: ProgressDraft) {
    await commitTaskMutation(task, () => taskApi.recordProgress({
      id: task.id,
      expectedRevision: task.revision,
      ...update,
    }));
  }

  async function moveTask(task: Task, listId: string | null) {
    await commitTaskMutation(task, () => taskApi.move({
      id: task.id, expectedRevision: task.revision, listId,
    }));
  }

  async function updateDetails(task: Task, tags: string[], dueDate: string | null) {
    await commitTaskMutation(task, () => taskApi.details({
      id: task.id, expectedRevision: task.revision, tags, dueDate,
    }));
  }

  function changeSort(nextSort: SortMode) {
    if (sort === nextSort) return;
    currentQuery.current = { ...currentQuery.current, sort: nextSort };
    requestGeneration.current += 1;
    setError(null);
    setRefreshError(false);
    setLoading(true);
    setNextCursor(null);
    resetTaskPages();
    setSort(nextSort);
  }

  async function nextTaskPage() {
    if (!nextCursor || loadingMore || creating || savingId) return;
    const generation = requestGeneration.current;
    setLoadingMore(true);
    setError(null);
    try {
      const page = await taskApi.list(nextCursor, sort, view);
      if (generation !== requestGeneration.current) return;
      setPreviousPageCursors((existing) => [...existing, currentPageCursor]);
      setCurrentPageCursor(nextCursor);
      setTasks(page.items);
      setExpandedId(null);
      setNextCursor(page.nextCursor);
    } catch (reason) {
      if (generation === requestGeneration.current) setError(errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  async function previousTaskPage() {
    if (previousPageCursors.length === 0 || loadingMore || creating || savingId) return;
    const generation = requestGeneration.current;
    const previous = previousPageCursors[previousPageCursors.length - 1];
    setLoadingMore(true);
    setError(null);
    try {
      const page = await taskApi.list(previous, sort, view);
      if (generation !== requestGeneration.current) return;
      setPreviousPageCursors((existing) => existing.slice(0, -1));
      setCurrentPageCursor(previous);
      setTasks(page.items);
      setExpandedId(null);
      setNextCursor(page.nextCursor);
    } catch (reason) {
      if (generation === requestGeneration.current) setError(errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  const groups = STATUS_ORDER.map((status) => ({
    status,
    items: tasks.filter((task) => task.status === status),
  })).filter((group) => group.items.length > 0);

  const pageTitle = view.kind === "all" ? "All tasks"
    : view.kind === "inbox" ? "Inbox"
    : lists.find((list) => list.id === view.listId)?.name ?? "List";
  const today = useLocalCalendarDay();

  const captureOpen = title.length > 0 || description.length > 0;
  const captureDisabled = creating || Boolean(savingId) || loading || loadError;

  return (
    <div className="app-shell">
      <TaskSidebar
        view={view} lists={lists} shownCount={tasks.length} hasMoreTasks={Boolean(nextCursor)}
        listError={listError}
        hasMoreLists={Boolean(listCursor)} busy={busyList || creating || Boolean(savingId) || loadingMore}
        theme={theme} onToggleTheme={toggleTheme}
        onNavigate={navigate} onCreateList={createList} onRenameList={renameList}
        onRemoveList={(list) => setListRemoved(list, true)}
        onRestoreList={(list) => setListRemoved(list, false)}
        onLoadMoreLists={loadMoreLists}
      />

      <main className="main-view">
        <header className="toolbar">
          <span className="toolbar-grow" />
          <label className="toolbar-sort" htmlFor="task-sort">
            <ArrowDownWideNarrow size={17} aria-hidden="true" />
            <select
              id="task-sort"
              aria-label="Sort tasks"
              value={sort}
              disabled={loading || loadingMore || Boolean(savingId)}
              onChange={(event) => changeSort(event.target.value as SortMode)}
            >
              <option value="priority">Highest priority</option>
              <option value="newest">Newest first</option>
            </select>
          </label>
          <button className="icon-button" type="button" aria-label="New task"
            disabled={captureDisabled} onClick={() => titleInput.current?.focus()}>
            <SquarePen size={18} aria-hidden="true" />
          </button>
        </header>

        <div className="main-scroll">
          <div className="content-wrap">
            <div className="page-title">
              <h1>{pageTitle}</h1>
              <span>{tasks.length}{nextCursor ? "+" : ""}</span>
            </div>

            <form className={`capture ${captureOpen ? "capture--open" : ""}`} aria-label="Create a task" onSubmit={createTask}>
              <div className="capture-row">
                <span className="capture-ring" aria-hidden="true" />
                <input
                  ref={titleInput}
                  id="new-task-title"
                  className="capture-title"
                  aria-label="Task title"
                  placeholder="New task"
                  value={title}
                  onChange={(event) => setTitle(event.target.value)}
                  maxLength={200}
                  disabled={captureDisabled}
                  required
                />
              </div>
              {captureOpen && (
                <div className="capture-extra">
                  <textarea
                    id="new-task-desc"
                    className="capture-description"
                    aria-label="Description (optional)"
                    placeholder="Notes (optional)"
                    value={description}
                    onChange={(event) => setDescription(event.target.value)}
                    rows={2}
                    maxLength={20000}
                    disabled={captureDisabled}
                  />
                  <div className="capture-actions">
                    <span>Starts as Ready</span>
                    <button className="button button-primary" type="submit" disabled={!title.trim() || captureDisabled}>
                      {creating && <LoaderCircle className="spin" size={14} aria-hidden="true" />}
                      {creating ? "Adding..." : "Add task"}
                    </button>
                  </div>
                </div>
              )}
            </form>

            {error && <div role="alert" className="error-banner">
              <span>{error}</span>
              {refreshError && <button className="button button-small" type="button" onClick={retryFirstLoad}>Refresh list</button>}
            </div>}

            <section className="task-section" aria-label="Your tasks">
              {loading ? (
                <div className="loading-state" role="status"><LoaderCircle className="spin" size={16} /> Loading your tasks...</div>
              ) : loadError && tasks.length === 0 ? (
                <div className="empty-state" aria-label="Tasks unavailable">
                  <LockKeyhole size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h3>Couldn't load your tasks</h3>
                  <p>The local database couldn't be opened. Nothing has been changed; check the message above and try again.</p>
                  <button className="button button-small" type="button" onClick={retryFirstLoad}>Try again</button>
                </div>
              ) : tasks.length === 0 && previousPageCursors.length === 0 ? (
                <div className="empty-state">
                  <CheckCircle2 size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h3>Your list is clear</h3>
                  <p>Type a title in New task above to add your first one.</p>
                </div>
              ) : (
                <div className="task-list">
                  {groups.map(({ status, items }) => (
                    <section className="task-group" key={status} aria-label={`${STATUS_VIEW[status].label} tasks`}>
                      <h3 className="task-group-heading">
                        {STATUS_VIEW[status].label}
                        <span>{items.length}{nextCursor ? "+" : ""}</span>
                      </h3>
                      {items.map((task) => (
                        <TaskRow
                          key={task.id}
                          task={task}
                          expanded={expandedId === task.id}
                          saving={savingId === task.id}
                          onToggle={() => setExpandedId((current) => current === task.id ? null : task.id)}
                          onSave={(nextTitle, nextDescription) => updateTask(task, nextTitle, nextDescription)}
                          onChange={(change) => changeTask(task, change)}
                          onRecordProgress={(update) => recordProgress(task, update)}
                          lists={lists.filter((list) => !list.removed)}
                          today={today}
                          onMove={(listId) => moveTask(task, listId)}
                          onUpdateDetails={(tags, dueDate) => updateDetails(task, tags, dueDate)}
                        />
                      ))}
                    </section>
                  ))}
                  {(previousPageCursors.length > 0 || nextCursor) && (
                    <div className="task-page-navigation" aria-label="Task pages">
                      {previousPageCursors.length > 0 && (
                        <button className="button button-small" type="button"
                          disabled={loadingMore || Boolean(savingId) || creating}
                          onClick={() => void previousTaskPage()}>
                          Previous page
                        </button>
                      )}
                      {nextCursor && (
                        <button className="button button-small" type="button"
                          disabled={loadingMore || Boolean(savingId) || creating}
                          onClick={() => void nextTaskPage()}>
                          {loadingMore ? "Loading..." : "Next page"}
                          {loadingMore && <LoaderCircle className="spin" size={14} />}
                        </button>
                      )}
                    </div>
                  )}
                </div>
              )}
            </section>
          </div>
        </div>
      </main>
    </div>
  );
}
