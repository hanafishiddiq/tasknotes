import { lazy, Suspense, useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import {
  ArrowDownWideNarrow, CheckCircle2, LoaderCircle, LockKeyhole, NotebookText, Search, SearchX, SquarePen,
  Tag,
} from "lucide-react";
import {
  taskApi, NO_FILTERS, TASK_PAGE_SIZE, type Task, type TaskList, type TaskView,
  type SortMode, type TaskChange, type ProgressDraft, type TaskFilters, type WorkStatus,
  type Priority,
} from "./lib/task-api";
import { PRIORITY_ORDER, PRIORITY_VIEW, STATUS_ORDER, STATUS_VIEW } from "./lib/task-visuals";
import TaskRow from "./components/TaskRow";
import TaskSidebar from "./components/TaskSidebar";
import VirtualTaskList from "./components/VirtualTaskList";
import { useLocalCalendarDay } from "./lib/use-local-calendar-day";
import { useTheme } from "./lib/use-theme";
import { useShortcuts } from "./lib/use-shortcuts";
import { errorMessage } from "./lib/error-message";
import { listenForQuit } from "./lib/quit";
import {
  ALL_NOTES, noteApi, type Note, type NoteFolder, type NotePage, type NoteScope,
} from "./lib/note-api";
import type { NotesRequest } from "./components/NotesWorkspace";
import { useNoteFolders } from "./lib/use-note-folders";
import NoteFolderTree from "./components/NoteFolderTree";
import NoteSnippet from "./components/NoteSnippet";
import RemovedView from "./components/RemovedView";
import SettingsView from "./components/SettingsView";
import CommandPalette, { type Command } from "./components/CommandPalette";
import { removedApi, type RemovedItem } from "./lib/removed-api";
import "./App.css";

// The rich-text editor loads only when Notes are first opened, keeping Task startup light.
const NotesWorkspace = lazy(() => import("./components/NotesWorkspace"));

// Typing settles before a search query reaches the core.
const SEARCH_DELAY_MS = 250;

function sameFilters(a: TaskFilters, b: TaskFilters): boolean {
  return a.text.trim() === b.text.trim() && a.tag.trim() === b.tag.trim() &&
    a.status === b.status && a.priority === b.priority && a.taskId === b.taskId;
}

/** A move to Removed that can still be taken back. */
interface Undoable {
  title: string;
  undo: () => Promise<void>;
}

export default function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [view, setView] = useState<TaskView>({ kind: "all" });
  const [workspace, setWorkspace] = useState<"tasks" | "notes" | "removed" | "settings">("tasks");
  // Bumped after something is moved to Removed, so the Removed view asks again.
  const [removedKey, setRemovedKey] = useState(0);
  // The last move to Removed, which Undo (or ⌘Z) brings back.
  const [undoable, setUndoable] = useState<Undoable | null>(null);
  const [undoing, setUndoing] = useState(false);
  const [undoError, setUndoError] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const paletteReturn = useRef<HTMLElement | null>(null);
  // A field to focus once the Task view is showing (⌘N, ⌘F, palette).
  const [focusRequest, setFocusRequest] = useState<{ field: "capture" | "search"; seq: number } | null>(null);
  // Bumped after a Note or folder is restored, so the Notes list asks again.
  const [notesRefresh, setNotesRefresh] = useState(0);
  // Once opened, Notes stay mounted so a pending autosave survives visiting Tasks.
  const [notesOpened, setNotesOpened] = useState(false);
  const [noteScope, setNoteScope] = useState<NoteScope>(ALL_NOTES);
  // The folder tree loads with Notes, keeping Task startup independent of it.
  const noteFolders = useNoteFolders(notesOpened);
  // Global search: the first few matching Notes beside the Tasks, never their bodies.
  // notesRequest also carries a Note opened from a Task's linked Notes.
  const [noteHits, setNoteHits] = useState<NotePage | null>(null);
  const [noteHitsFailed, setNoteHitsFailed] = useState(false);
  const [notesRequest, setNotesRequest] = useState<NotesRequest | null>(null);
  // A linked Task asked for while the Task view was busy.
  const [pendingLinkedTask, setPendingLinkedTask] = useState<string | null>(null);
  const [lists, setLists] = useState<TaskList[]>([]);
  const [listCursor, setListCursor] = useState<string | null>(null);
  const [busyList, setBusyList] = useState(false);
  const [loadingLists, setLoadingLists] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [sort, setSort] = useState<SortMode>("priority");
  const [filters, setFilters] = useState<TaskFilters>(NO_FILTERS);
  const [searchDraft, setSearchDraft] = useState("");
  const [tagDraft, setTagDraft] = useState("");
  // A Task just created outside the active search or filters.
  const [createdHidden, setCreatedHidden] = useState(false);
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
  // Rows holding unsaved edits; the virtualized list keeps them mounted.
  const [dirtyIds, setDirtyIds] = useState<ReadonlySet<string>>(new Set());
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);
  const appearance = useTheme();
  const titleInput = useRef<HTMLInputElement>(null);
  const taskScroll = useRef<HTMLDivElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  // Ignore stale results from older Task queries after a newer user action.
  const requestGeneration = useRef(0);
  const currentQuery = useRef({ sort, view, filters });

  useEffect(() => {
    currentQuery.current = { sort, view, filters };
    const generation = ++requestGeneration.current;
    let active = true;
    taskApi
      .list(null, sort, view, filters)
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
  }, [loadAttempt, sort, view, filters]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      applyFilters({ ...currentQuery.current.filters, text: searchDraft, tag: tagDraft });
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
    // applyFilters reads the latest query through currentQuery.
  }, [searchDraft, tagDraft]);

  useEffect(() => {
    const text = filters.text.trim();
    // Only blank text is no search; the core decides what the words are.
    if (!text) {
      setNoteHits(null);
      setNoteHitsFailed(false);
      return;
    }
    let active = true;
    noteApi.search(text)
      .then((page) => {
        if (!active) return;
        setNoteHits(page);
        setNoteHitsFailed(false);
      })
      .catch(() => {
        if (!active) return;
        setNoteHits(null);
        setNoteHitsFailed(true);
      });
    return () => { active = false; };
  }, [filters.text]);

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

  useEffect(() => {
    // Only Notes save on a delay, so they are what can hold a quit back.
    // Showing Notes keeps the folder in view; only an unsaved Note can hold the quit back.
    const stop = listenForQuit(() => { setNotesOpened(true); setWorkspace("notes"); });
    return () => void stop.then((unlisten) => unlisten());
  }, []);

  useEffect(() => {
    if (pendingLinkedTask && !(busyList || creating || savingId || loadingMore)) {
      openLinkedTask(pendingLinkedTask);
    }
    // openLinkedTask reads the same busy flags listed here.
  }, [pendingLinkedTask, busyList, creating, savingId, loadingMore]);

  useEffect(() => {
    if (!focusRequest || workspace !== "tasks") return;
    (focusRequest.field === "capture" ? titleInput.current : searchInput.current)?.focus();
    setFocusRequest(null);
  }, [focusRequest, workspace]);

  function resetTaskPages() {
    requestGeneration.current += 1;
    setCurrentPageCursor(null);
    setPreviousPageCursors([]);
  }

  function openNotes(scope: NoteScope = ALL_NOTES) {
    setUndoable(null);
    setNotesOpened(true);
    setWorkspace("notes");
    setNoteScope(scope);
  }

  /** Shows a global search in Notes, opening one of its Notes when given. */
  function searchInNotes(openId: string | null) {
    setNotesRequest((previous) => ({
      seq: (previous?.seq ?? 0) + 1, search: filters.text.trim(), openId,
    }));
    openNotes(ALL_NOTES);
  }

  /** Shows one Task, expanded, as when it is opened from a Note it is linked to. */
  function openLinkedTask(taskId: string) {
    if (busyList || creating || savingId || loadingMore) {
      // Opened once the Task view has finished its current change.
      setPendingLinkedTask(taskId);
      setWorkspace("tasks");
      return;
    }
    setPendingLinkedTask(null);
    navigate({ kind: "all" });
    setSearchDraft("");
    setTagDraft("");
    applyFilters({ ...NO_FILTERS, taskId });
    setExpandedId(taskId);
  }

  function openSettings() {
    if (busyList || creating || savingId || loadingMore) return;
    setUndoable(null);
    setWorkspace("settings");
  }

  function openRemoved() {
    if (busyList || creating || savingId || loadingMore) return;
    setUndoable(null);
    setWorkspace("removed");
  }

  /** Shows a restored item again wherever it belongs. */
  function restored(item: RemovedItem) {
    // What Undo would bring back may be the very item just restored.
    setUndoable(null);
    if (item.kind === "list") {
      refreshLists().catch((reason) => setListError("Could not load Lists: " + errorMessage(reason)));
    }
    if (item.kind === "task" || item.kind === "list") retryFirstLoad();
    if (item.kind === "note_folder") {
      void noteFolders.refresh();
    }
    if (item.kind === "note" || item.kind === "note_folder") setNotesRefresh((key) => key + 1);
  }

  /**
   * A Removed folder hides its subtree. The Notes list refreshes; if the open
   * folder is now hidden, the core refuses to list it and the view goes back
   * to All notes (onScopeHidden).
   */
  function folderRemoved(folder: NoteFolder) {
    setRemovedKey((key) => key + 1);
    setNotesRefresh((key) => key + 1);
    setUndoable({
      title: folder.name,
      undo: () => restoreNoteItem("note_folder", folder),
    });
  }

  function noteRemoved(note: Note) {
    setRemovedKey((key) => key + 1);
    setUndoable({ title: note.title, undo: () => restoreNoteItem("note", note) });
  }

  /** Undo for a Note or folder: restore it, then let Notes and the folder tree ask again. */
  async function restoreNoteItem(kind: "note" | "note_folder", item: Note | NoteFolder) {
    try {
      await removedApi.restore(kind, item);
      if (kind === "note_folder") await noteFolders.refresh();
      setNotesRefresh((key) => key + 1);
      setRemovedKey((key) => key + 1);
    } catch (reason) {
      setUndoError("Couldn't undo: " + errorMessage(reason));
    }
  }

  function openLinkedNote(noteId: string) {
    setNotesRequest((previous) => ({ seq: (previous?.seq ?? 0) + 1, search: "", openId: noteId }));
    openNotes(ALL_NOTES);
  }

  function navigate(next: TaskView) {
    if (busyList || creating || savingId || loadingMore) return;
    setWorkspace("tasks");
    setUndoable(null);
    if (next.kind === view.kind &&
        (next.kind !== "list" || (view.kind === "list" && next.listId === view.listId))) return;
    setTasks([]);
    setNextCursor(null);
    resetTaskPages();
    setExpandedId(null);
    setCreatedHidden(false);
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
      setRemovedKey((key) => key + 1);
      setUndoable(removed ? { title: updated.name, undo: () => setListRemoved(updated, false) } : null);
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
      const { sort: latestSort, view: latestView, filters: latestFilters } = currentQuery.current;
      // Clear only after the actual write succeeds; retry must not create a
      // duplicate if the subsequent read fails.
      setTitle("");
      setDescription("");
      try {
        const page = await taskApi.list(null, latestSort, latestView, latestFilters);
        if (generation !== requestGeneration.current) return;
        setLoading(false);
        setTasks(page.items);
        setCurrentPageCursor(null);
        setPreviousPageCursors([]);
        setNextCursor(page.nextCursor);
        setRefreshError(false);
        const offPage = !sameFilters(latestFilters, NO_FILTERS) &&
          !page.items.some((item) => item.id === created.id);
        // Off the first page, only the core can say whether the new Task
        // matches. If that check fails, say nothing rather than guess.
        const hidden = offPage && (!page.nextCursor || await taskApi
          .matches(created.id, latestView, latestFilters)
          .then((matches) => !matches, () => false));
        if (generation === requestGeneration.current) setCreatedHidden(hidden);
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

  /** Resolves true once the mutation itself succeeded. */
  async function commitTaskMutation(task: Task, mutate: () => Promise<Task>): Promise<boolean> {
    if (savingId) return false;
    setSavingId(task.id);
    setError(null);
    // Paging is disabled while saving, so this is still the visible page.
    const queryAtStart = currentQuery.current;
    const startCursor = currentPageCursor;
    try {
      const updated = await mutate();
      const generation = ++requestGeneration.current;
      const { sort: latestSort, view: latestView, filters: latestFilters } = currentQuery.current;
      // A cursor belongs to the query that produced it; a new query starts on page 1.
      const pageCursor = currentQuery.current === queryAtStart ? startCursor : null;
      setTasks((current) => current.map((item) => item.id === updated.id ? updated : item));
      // The mutation already succeeded; never let an old page replace its result.
      try {
        let page = await taskApi.list(pageCursor, latestSort, latestView, latestFilters);
        const stayOnPage = pageCursor !== null && page.items.length > 0;
        if (pageCursor !== null && !stayOnPage) {
          page = await taskApi.list(null, latestSort, latestView, latestFilters);
        }
        if (generation !== requestGeneration.current) return true;
        setLoading(false);
        setTasks(page.items);
        if (!stayOnPage) {
          setCurrentPageCursor(null);
          setPreviousPageCursors([]);
        }
        setNextCursor(page.nextCursor);
        setRefreshError(false);
      } catch (reason) {
        if (generation !== requestGeneration.current) return true;
        setLoading(false);
        setNextCursor(null);
        resetTaskPages();
        setRefreshError(true);
        setError("Change saved, but the ordered list could not refresh: " + errorMessage(reason));
      }
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      return false;
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

  async function removeTask(task: Task) {
    const done: { task?: Task } = {};
    const saved = await commitTaskMutation(task, async () => (done.task = await removedApi.remove<Task>("task", task)));
    if (saved && done.task) {
      setExpandedId(null);
      const removed = done.task;
      setUndoable({
        title: removed.title,
        undo: async () => {
          if (await commitTaskMutation(removed, () => removedApi.restore<Task>("task", removed))) {
            setRemovedKey((key) => key + 1);
          }
        },
      });
      setRemovedKey((key) => key + 1);
    }
  }

  /** Runs the pending Undo once; the notice goes away whether or not it succeeds. */
  function runUndo(): boolean {
    if (!undoable || undoing) return false;
    const pending = undoable;
    setUndoable(null);
    setUndoError(null);
    setUndoing(true);
    void pending.undo().finally(() => setUndoing(false));
    return true;
  }

  function markDirty(id: string, dirty: boolean) {
    setDirtyIds((current) => {
      if (current.has(id) === dirty) return current;
      const next = new Set(current);
      if (dirty) next.add(id); else next.delete(id);
      return next;
    });
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

  /** Start a fresh first page for new search text or filters. */
  function applyFilters(next: TaskFilters) {
    if (sameFilters(next, currentQuery.current.filters)) return;
    setCreatedHidden(false);
    currentQuery.current = { ...currentQuery.current, filters: next };
    requestGeneration.current += 1;
    setError(null);
    setRefreshError(false);
    setLoading(true);
    setNextCursor(null);
    setExpandedId(null);
    resetTaskPages();
    setFilters(next);
  }

  function clearFilters() {
    setCreatedHidden(false);
    setSearchDraft("");
    setTagDraft("");
    applyFilters(NO_FILTERS);
  }

  async function nextTaskPage() {
    if (!nextCursor || loadingMore || creating || savingId) return;
    const generation = requestGeneration.current;
    setLoadingMore(true);
    setError(null);
    try {
      const page = await taskApi.list(nextCursor, sort, view, filters);
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
      const page = await taskApi.list(previous, sort, view, filters);
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

  /** Shows the Task view (keeping its place) and then focuses one of its fields. */
  function focusInTasks(field: "capture" | "search") {
    if (workspace !== "tasks") navigate(view);
    setFocusRequest((previous) => ({ field, seq: (previous?.seq ?? 0) + 1 }));
  }

  function closePalette() {
    setPaletteOpen(false);
    // Hand focus back unless the command moved it somewhere on purpose.
    const previous = paletteReturn.current;
    paletteReturn.current = null;
    window.setTimeout(() => {
      if (previous?.isConnected && document.activeElement === document.body) previous.focus();
    });
  }

  function openPalette() {
    if (paletteOpen) return;
    paletteReturn.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setPaletteOpen(true);
  }

  useShortcuts({
    palette: openPalette,
    newTask: () => focusInTasks("capture"),
    search: () => focusInTasks("search"),
    settings: openSettings,
    undo: runUndo,
  });

  const appearanceLabels = { dark: "Dark", light: "Light", system: "Match system" } as const;
  const commands: Command[] = [
    { id: "go-all", label: "All tasks", group: "Go to", run: () => navigate({ kind: "all" }) },
    { id: "go-inbox", label: "Inbox", group: "Go to", run: () => navigate({ kind: "inbox" }) },
    ...lists.filter((list) => !list.removed).map((list): Command => ({
      id: `go-list-${list.id}`, label: list.name, group: "Go to", keywords: "list",
      run: () => navigate({ kind: "list", listId: list.id }),
    })),
    { id: "go-notes", label: "All notes", group: "Go to", run: () => openNotes() },
    { id: "go-removed", label: "Removed", group: "Go to", keywords: "trash restore", run: openRemoved },
    { id: "go-settings", label: "Settings", group: "Go to", keywords: "preferences", run: openSettings },
    { id: "new-task", label: "New task", group: "Actions", keywords: "add create capture", run: () => focusInTasks("capture") },
    { id: "search", label: "Search tasks and notes", group: "Actions", keywords: "find", run: () => focusInTasks("search") },
    ...(undoable ? [{
      id: "undo", label: `Undo moving “${undoable.title}” to Removed`, group: "Actions", keywords: "restore",
      run: () => void runUndo(),
    }] : []),
    ...(["dark", "light", "system"] as const).filter((value) => value !== appearance.preference)
      .map((value): Command => ({
        id: `appearance-${value}`, label: `Appearance: ${appearanceLabels[value]}`, group: "Actions",
        keywords: "theme mode", run: () => appearance.setPreference(value),
      })),
  ];
  const searchCommand = (text: string): Command => ({
    id: "search-text", label: `Search for “${text}”`, group: "Actions",
    run: () => { focusInTasks("search"); setSearchDraft(text); },
  });

  const keepMounted = useMemo(
    () => new Set([...dirtyIds, ...(expandedId ? [expandedId] : [])]), [dirtyIds, expandedId],
  );

  const pageTitle = view.kind === "all" ? "All tasks"
    : view.kind === "inbox" ? "Inbox"
    : lists.find((list) => list.id === view.listId)?.name ?? "List";
  const today = useLocalCalendarDay();

  const filtering = !sameFilters(filters, NO_FILTERS);
  const filterControlsActive = filtering || searchDraft.trim() !== "" || tagDraft.trim() !== "";
  const captureOpen = title.length > 0 || description.length > 0;
  const captureDisabled = creating || Boolean(savingId) || loading || loadError;
  // After a refused search the rows on hand belong to the previous query.
  const countKnown = !(loadError && filtering);

  return (
    <div className="app-shell">
      <TaskSidebar
        view={view} notesActive={workspace !== "tasks"} onOpenNotes={() => openNotes()}
        removedActive={workspace === "removed"} onOpenRemoved={openRemoved}
        allNotesCurrent={workspace === "notes" && noteScope.kind === "all"}
        noteFolders={notesOpened && (
          <NoteFolderTree folders={noteFolders}
            currentId={workspace === "notes" && noteScope.kind === "folder" ? noteScope.folderId : null}
            disabled={busyList || creating || Boolean(savingId) || loadingMore}
            onOpen={(folder) => openNotes({ kind: "folder", folderId: folder.id })}
            onRemoved={folderRemoved} />
        )}
        lists={lists} shownCount={countKnown ? tasks.length : null} hasMoreTasks={Boolean(nextCursor)}
        listError={listError}
        hasMoreLists={Boolean(listCursor)} busy={busyList || creating || Boolean(savingId) || loadingMore}
        theme={appearance.theme} onToggleTheme={appearance.toggle}
        settingsActive={workspace === "settings"} onOpenSettings={openSettings}
        onNavigate={navigate} onCreateList={createList} onRenameList={renameList}
        onRemoveList={(list) => setListRemoved(list, true)}
        onLoadMoreLists={loadMoreLists}
      />

      <main className="main-view" hidden={workspace !== "tasks"}>
        <header className="toolbar">
          <span className="toolbar-grow" />
          <label className="toolbar-search" htmlFor="task-search">
            <Search size={15} aria-hidden="true" />
            <input
              ref={searchInput}
              id="task-search"
              type="search"
              aria-label="Search tasks and notes"
              placeholder="Search"
              maxLength={200}
              value={searchDraft}
              onChange={(event) => setSearchDraft(event.target.value)}
            />
          </label>
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

        <div className="main-scroll" ref={taskScroll}>
          <div className="content-wrap">
            <div className="page-title">
              <h1>{pageTitle}</h1>
              {countKnown && <span>{tasks.length}{nextCursor ? "+" : ""}</span>}
            </div>

            <div className="filter-chips" role="group" aria-label="Filters">
              <label className={`property filter-chip ${filters.status ? "filter-chip--active" : ""}`}>
                <select aria-label="Filter by status" value={filters.status}
                  onChange={(event) => applyFilters({
                    ...currentQuery.current.filters, status: event.target.value as WorkStatus | "",
                  })}>
                  <option value="">Any status</option>
                  {STATUS_ORDER.map((status) => (
                    <option key={status} value={status}>{STATUS_VIEW[status].label}</option>
                  ))}
                </select>
              </label>
              <label className={`property filter-chip ${filters.priority ? "filter-chip--active" : ""}`}>
                <select aria-label="Filter by priority" value={filters.priority}
                  onChange={(event) => applyFilters({
                    ...currentQuery.current.filters, priority: event.target.value as Priority | "",
                  })}>
                  <option value="">Any priority</option>
                  {[...PRIORITY_ORDER].reverse().map((priority) => (
                    <option key={priority} value={priority}>{PRIORITY_VIEW[priority].label}</option>
                  ))}
                </select>
              </label>
              <label className={`property filter-chip filter-chip--tag ${filters.tag ? "filter-chip--active" : ""}`}>
                <Tag size={13} aria-hidden="true" />
                <input aria-label="Filter by tag" placeholder="Tag" maxLength={40}
                  value={tagDraft} onChange={(event) => setTagDraft(event.target.value)} />
              </label>
              {filterControlsActive && (
                <button className="button button-small" type="button" onClick={clearFilters}>
                  Clear search and filters
                </button>
              )}
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

            {pendingLinkedTask && (
              <div role="status" aria-label="Opening linked Task" className="notice-banner">
                <LoaderCircle className="spin" size={14} aria-hidden="true" />
                <span>Opening the linked Task once the current change is saved…</span>
              </div>
            )}

            {filters.taskId && (
              <div role="status" aria-label="Linked Task" className="notice-banner">
                <span>Showing one Task linked to a Note</span>
                <button className="button button-small" type="button" onClick={clearFilters}>
                  Show all tasks
                </button>
              </div>
            )}

            {createdHidden && (
              <div role="status" aria-label="Task hidden by filters" className="notice-banner">
                <span>Task added, but hidden by the current search or filters</span>
                <button className="button button-small" type="button" onClick={clearFilters}>
                  Clear search and filters
                </button>
              </div>
            )}

            {error && <div role="alert" className="error-banner">
              <span>{error}</span>
              {refreshError && <button className="button button-small" type="button" onClick={retryFirstLoad}>Refresh list</button>}
            </div>}

            {(noteHits || noteHitsFailed) && (
              <section className="note-hits" aria-label="Matching notes">
                <h3 className="task-group-heading">Notes</h3>
                {noteHitsFailed ? (
                  <p className="note-hits-empty">Notes couldn't be searched.</p>
                ) : noteHits && noteHits.items.length === 0 ? (
                  <p className="note-hits-empty">No matching notes</p>
                ) : noteHits && (
                  <>
                    <ul className="note-hits-list">
                      {noteHits.items.map((hit) => (
                        <li key={hit.id}>
                          <button type="button" className="note-hit" onClick={() => searchInNotes(hit.id)}>
                            <NotebookText size={15} aria-hidden="true" />
                            <span className="note-hit-text">
                              <span className="note-hit-title">{hit.title}</span>
                              {hit.snippet && <NoteSnippet text={hit.snippet} />}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                    {noteHits.nextCursor && (
                      <button className="button button-small" type="button" onClick={() => searchInNotes(null)}>
                        Show all matching notes
                      </button>
                    )}
                  </>
                )}
              </section>
            )}

            <section className="task-section" aria-label="Your tasks">
              {loading ? (
                <div className="loading-state" role="status"><LoaderCircle className="spin" size={16} /> Loading your tasks...</div>
              ) : loadError && filtering ? (
                <div className="empty-state" aria-label="Search unavailable">
                  <SearchX size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h3>Search couldn't run</h3>
                  <p>Check the message above, or clear the search and filters to see your tasks.</p>
                  <div className="empty-state-actions">
                    <button className="button button-small" type="button" onClick={clearFilters}>
                      Clear search and filters
                    </button>
                    <button className="button button-small" type="button" onClick={retryFirstLoad}>Try again</button>
                  </div>
                </div>
              ) : loadError && tasks.length === 0 ? (
                <div className="empty-state" aria-label="Tasks unavailable">
                  <LockKeyhole size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h3>Couldn't load your tasks</h3>
                  <p>The local database couldn't be opened. Nothing has been changed; check the message above and try again.</p>
                  <button className="button button-small" type="button" onClick={retryFirstLoad}>Try again</button>
                </div>
              ) : tasks.length === 0 && previousPageCursors.length === 0 && filtering ? (
                <div className="empty-state">
                  <SearchX size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h3>No matching tasks</h3>
                  <p>Nothing in this view matches the search and filters.</p>
                  <button className="button button-small" type="button" onClick={clearFilters}>
                    Clear search and filters
                  </button>
                </div>
              ) : tasks.length === 0 && previousPageCursors.length === 0 ? (
                <div className="empty-state">
                  <CheckCircle2 size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h3>Your list is clear</h3>
                  <p>Type a title in New task above to add your first one.</p>
                </div>
              ) : (
                <>
                  <VirtualTaskList
                    tasks={tasks}
                    hasMore={Boolean(nextCursor)}
                    scrollRef={taskScroll}
                    keepMounted={keepMounted}
                    expandedId={expandedId}
                    onCollapse={() => setExpandedId(null)}
                    renderTask={(task) => (
                      <TaskRow
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
                        onOpenNote={openLinkedNote}
                        onRemove={() => removeTask(task)}
                        shown={workspace === "tasks"}
                        onDirtyChange={(dirty) => markDirty(task.id, dirty)}
                      />
                    )}
                  />
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
                </>
              )}
            </section>
          </div>
        </div>
      </main>
      <SettingsView hidden={workspace !== "settings"} preference={appearance.preference}
        onPreference={appearance.setPreference} />
      <RemovedView hidden={workspace !== "removed"} refreshKey={removedKey} onRestored={restored} />
      {notesOpened && (
        <Suspense fallback={workspace === "notes" &&
          <div className="loading-state" role="status"><LoaderCircle className="spin" size={16} /> Loading notes...</div>}>
          <NotesWorkspace hidden={workspace !== "notes"} scope={noteScope}
            scopeName={noteScope.kind === "folder"
              ? noteFolders.byId.get(noteScope.folderId)?.name ?? "Folder" : "Notes"}
            folders={noteFolders.options} request={notesRequest} onOpenTask={openLinkedTask}
            refreshKey={notesRefresh} onRemoved={noteRemoved}
            onScopeHidden={() => setNoteScope(ALL_NOTES)} />
        </Suspense>
      )}
      {paletteOpen && (
        <CommandPalette commands={commands} textCommand={searchCommand} onClose={closePalette} />
      )}
      {undoError && !undoable && (
        <div role="alert" className="undo-bar undo-bar--error">
          <span>{undoError}</span>
          <button className="button button-small" type="button" onClick={() => setUndoError(null)}>
            Dismiss
          </button>
        </div>
      )}
      {undoable && (
        <div role="status" aria-label="Moved to Removed" className="undo-bar">
          <span>“{undoable.title}” moved to Removed</span>
          <button className="button button-small" type="button" disabled={undoing || Boolean(savingId)}
            onClick={runUndo}>
            Undo
          </button>
        </div>
      )}
    </div>
  );
}
