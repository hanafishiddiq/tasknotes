import { lazy, Suspense, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent } from "react";
import {
  ArrowDownWideNarrow, CheckCircle2, LoaderCircle, LockKeyhole, NotebookText, Search, SearchX, SquarePen,
  Tag,
} from "lucide-react";
import {
  taskApi, NO_FILTERS, TASK_PAGE_SIZE, placeLocally, sortOf, viewKey, type Task, type TaskList,
  type TaskView, type SortMode, type TaskChange, type ProgressDraft, type TaskFilters,
  type Priority, type TaskListPage, type TaskPlace,
} from "./lib/task-api";
import {
  DIFFICULTY_ORDER, DIFFICULTY_VIEW, PRIORITY_ORDER, PRIORITY_VIEW, STATUS_CHOICES,
  statusChoiceLabel,
} from "./lib/task-visuals";
import TaskRow from "./components/TaskRow";
import TaskSidebar from "./components/TaskSidebar";
import SidebarResizer from "./components/SidebarResizer";
import { useSidebarWidth } from "./lib/use-sidebar-width";
import VirtualTaskList from "./components/VirtualTaskList";
import NewTaskToast from "./components/NewTaskToast";
import { useLocalCalendarDay } from "./lib/use-local-calendar-day";
import { useTheme } from "./lib/use-theme";
import { useSound } from "./lib/sound/use-sound";
import { useShortcuts } from "./lib/use-shortcuts";
import { useTaskDrafts } from "./lib/use-task-drafts";
import { flushSync } from "react-dom";
import { useUndo } from "./lib/use-undo";
import { useNewTaskToast } from "./lib/use-new-task-toast";
import { errorMessage, isListRemoved } from "./lib/error-message";
import { listenForQuit } from "./lib/quit";
import { listenForQuickCapture, shellApi } from "./lib/shell-api";
import { newTaskKeys } from "./lib/shortcuts";
import {
  ALL_NOTES, noteApi, type Note, type NoteFolder, type NotePage, type NoteScope,
} from "./lib/note-api";
import type { NotesRequest } from "./components/NotesWorkspace";
import { useNoteFolders } from "./lib/use-note-folders";
import NoteFolderTree from "./components/NoteFolderTree";
import NoteSnippet from "./components/NoteSnippet";
import RemovedView from "./components/RemovedView";
import ApprovalsView from "./components/ApprovalsView";
import { usePendingApprovals } from "./lib/use-pending-approvals";
import { useOutsideChanges } from "./lib/use-outside-changes";
import { captureScrollAnchor, holdScrollAnchor, type ScrollAnchor } from "./lib/scroll-anchor";
import SettingsView from "./components/SettingsView";
import SoundSettings from "./components/SoundSettings";
import CommandPalette, { type Command } from "./components/CommandPalette";
import { buildCommands } from "./lib/app-commands";
import { removedApi, type RemovedItem } from "./lib/removed-api";
import type { Style } from "./lib/style";
import "./App.css";

// The rich-text editor loads only when Notes are first opened, keeping Task startup light.
const NotesWorkspace = lazy(() => import("./components/NotesWorkspace"));

// Typing settles before a search query reaches the core.
const SEARCH_DELAY_MS = 250;

function sameFilters(a: TaskFilters, b: TaskFilters): boolean {
  return a.text.trim() === b.text.trim() && a.tag.trim() === b.tag.trim() &&
    a.status === b.status && a.priority === b.priority && a.difficulty === b.difficulty &&
    a.taskId === b.taskId;
}

interface TaskQueryNow {
  sort: SortMode;
  view: TaskView;
  filters: TaskFilters;
}

/** Why one Task shows on its own: opened from a Note it is linked to, or from the new Task toast. */
type AloneReason = "linked" | "added";

/** What the Task view says while, and once, a Task shows on its own, by reason. */
const ALONE_BANNER: Record<AloneReason, { opening: string; openingText: string; shown: string; shownText: string }> = {
  linked: {
    opening: "Opening linked Task", openingText: "Opening the linked Task once the current change is saved…",
    shown: "Linked Task", shownText: "Showing one Task linked to a Note",
  },
  added: {
    opening: "Opening added Task", openingText: "Opening the Task you just added once the current change is saved…",
    shown: "Added Task", shownText: "Showing the Task you just added",
  },
};

/** The first page of Lists again, and more until at least `keep`, so Lists already loaded stay. */
async function readListsAgain(keep: number): Promise<TaskListPage> {
  let page = await taskApi.lists();
  let items = page.items;
  while (items.length < keep && page.nextCursor !== null) {
    page = await taskApi.lists(page.nextCursor);
    items = [...items, ...page.items];
  }
  return { ...page, items };
}

/** A page read again. What its cursor held may be gone: then the first page is what shows. */
async function readPageAgain(cursor: string | null, { sort, view, filters }: TaskQueryNow) {
  let page = await taskApi.list(cursor, sort, view, filters);
  const stayOnPage = cursor !== null && page.items.length > 0;
  if (cursor !== null && !stayOnPage) page = await taskApi.list(null, sort, view, filters);
  return { page, stayOnPage };
}

export default function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [view, setView] = useState<TaskView>({ kind: "all" });
  const [workspace, setWorkspace] = useState<"tasks" | "notes" | "removed" | "settings" | "approvals">("tasks");
  // Bulk Changes an AI client asked for, waiting for the person.
  const approvals = usePendingApprovals();
  // Bumped after something is moved to Removed, so the Removed view asks again.
  const [removedKey, setRemovedKey] = useState(0);
  // Recent moves to Removed, which Undo (or ⌘Z) brings back newest first.
  const undo = useUndo();
  // "Task added · Click to edit" after a Task is created here; view state only.
  const newTaskToast = useNewTaskToast();
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
  // A Task asked to show alone while the Task view was busy, and why.
  const [pendingAlone, setPendingAlone] = useState<{ taskId: string; reason: AloneReason } | null>(null);
  // Why one Task shows alone: opened from a Note, or from the new Task toast.
  const [shownAlone, setShownAlone] = useState<AloneReason>("linked");
  const [lists, setLists] = useState<TaskList[]>([]);
  const [listCursor, setListCursor] = useState<string | null>(null);
  const [busyList, setBusyList] = useState(false);
  const [loadingLists, setLoadingLists] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  // The sort each view remembers, kept by the core; a view not in it opens by Priority.
  const [viewSorts, setViewSorts] = useState<Readonly<Record<string, SortMode>>>({});
  const sort = sortOf(viewSorts, view);
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
  // Unsaved Task edits live here, not in the rows, so they outlive a row leaving the page.
  const drafts = useTaskDrafts();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);
  const appearance = useTheme();
  const sidebar = useSidebarWidth();
  const sound = useSound();
  // A Task view change in flight: moving elsewhere waits until it settles.
  const busy = busyList || creating || Boolean(savingId) || loadingMore;
  const titleInput = useRef<HTMLInputElement>(null);
  const taskScroll = useRef<HTMLDivElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  // Ignore stale results from older Task queries after a newer user action.
  const requestGeneration = useRef(0);
  const currentQuery = useRef<TaskQueryNow>({ sort, view, filters });
  // The page showing now, for Undo: it runs long after the render that queued it.
  const currentPage = useRef(currentPageCursor);
  currentPage.current = currentPageCursor;
  // Where the rows sat before an outside change replaced them.
  const scrollAnchor = useRef<ScrollAnchor | null>(null);
  // Tasks kept on screen for their unsaved text, though the view no longer holds them.
  const keptForEdits = useRef<ReadonlySet<string>>(new Set());
  // The open Task as last rendered, for a read that finishes after the person opened another.
  const expandedNow = useRef(expandedId);
  expandedNow.current = expandedId;
  // Only the newest re-read of an outside change may show what it read.
  const outsideRead = useRef(0);

  useOutsideChanges(showOutsideChanges);

  // Before paint, so the rows never show in their new place.
  useLayoutEffect(() => {
    if (scrollAnchor.current && taskScroll.current) holdScrollAnchor(taskScroll.current, scrollAnchor.current);
    scrollAnchor.current = null;
  }, [tasks]);

  // Once the text is saved or given up, the list is what the core says again.
  const keptWithoutEdits = [...keptForEdits.current].some((id) => !drafts.get(id));
  useEffect(() => {
    if (keptWithoutEdits && !busy && !loading) void showOutsideChanges();
    // showOutsideChanges only has to see the edits gone and the view at rest.
  }, [keptWithoutEdits, busy, loading]);

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
    // Asked again when Notes change, so what another process added is found too.
  }, [filters.text, notesRefresh]);

  useEffect(() => {
    let active = true;
    // Until these arrive every view opens by Priority; a view that remembers
    // another sort then asks again, before the person can have done anything.
    taskApi.viewSorts().then((sorts) => {
      if (active) setViewSorts((chosen) => ({ ...sorts, ...chosen }));
    }).catch(() => undefined);
    return () => { active = false; };
  }, []);

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
    if (pendingAlone && !busy) showTaskAlone(pendingAlone.taskId, pendingAlone.reason);
    // showTaskAlone reads the same busy flag listed here.
  }, [pendingAlone, busy]);

  // A scroll the toast asked for gets one try. A view that finished loading without
  // its Task (moved to Removed, say) has no list to try it, so drop it here, or a
  // later Undo or refresh would move the list by itself.
  const { reveal, clearReveal } = newTaskToast;
  useEffect(() => {
    if (reveal && !loading && !tasks.some((task) => task.id === reveal.taskId)) clearReveal();
  }, [reveal, loading, tasks, clearReveal]);

  useEffect(() => {
    if (!focusRequest || workspace !== "tasks") return;
    const target = focusRequest.field === "capture" ? titleInput.current : searchInput.current;
    // A Quick Capture asked for while the list is still loading waits for the field.
    if (target?.disabled) return;
    target?.focus();
    setFocusRequest(null);
  }, [focusRequest, workspace, loading, creating, savingId]);

  function resetTaskPages() {
    requestGeneration.current += 1;
    setCurrentPageCursor(null);
    setPreviousPageCursors([]);
  }

  function openNotes(scope: NoteScope = ALL_NOTES) {
    undo.hideNotice();
    newTaskToast.clearReveal();
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

  /** Shows one Task on its own and expanded, under the banner for `reason`. */
  function showTaskAlone(taskId: string, reason: AloneReason) {
    if (busy) {
      // Opened once the Task view has finished its current change, for the same reason.
      setPendingAlone({ taskId, reason });
      setWorkspace("tasks");
      return;
    }
    setPendingAlone(null);
    setShownAlone(reason);
    // Its banner is the only one: applyFilters clears the filter notice only when a
    // filter changes, and this view must never show both.
    setCreatedHidden(false);
    navigate({ kind: "all" });
    setSearchDraft("");
    setTagDraft("");
    applyFilters({ ...NO_FILTERS, taskId });
    setExpandedId(taskId);
    // After navigate and applyFilters, which drop any earlier scroll request.
    if (reason === "added") newTaskToast.revealTask(taskId);
  }

  /** The new Task toast was clicked: open the Task where it is, or on its own when this view hides it. */
  function openNewTask(taskId: string) {
    if (workspace === "tasks" && tasks.some((task) => task.id === taskId)) {
      setExpandedId(taskId);
      newTaskToast.revealTask(taskId);
    } else showTaskAlone(taskId, "added");
  }

  function openSettings() {
    if (busy) return;
    undo.hideNotice();
    newTaskToast.clearReveal();
    setWorkspace("settings");
  }

  function openRemoved() {
    if (busy) return;
    undo.hideNotice();
    newTaskToast.clearReveal();
    setWorkspace("removed");
  }

  function openApprovals() {
    if (busy) return;
    undo.hideNotice();
    newTaskToast.clearReveal();
    void approvals.refresh();
    setWorkspace("approvals");
  }

  /** After a Bulk Change was decided here: every view and the count ask again. */
  function bulkDecided(approved: boolean) {
    void approvals.refresh();
    if (!approved) return;
    refreshLists().catch((reason) => setListError("Could not load Lists: " + errorMessage(reason)));
    retryFirstLoad();
    if (notesOpened) void noteFolders.refresh();
    setNotesRefresh((key) => key + 1);
    setRemovedKey((key) => key + 1);
  }

  /** Shows a restored item again wherever it belongs. */
  function restored(item: RemovedItem) {
    // What Undo would bring back may be the very item just restored.
    undo.forget(`${item.kind}:${item.id}`);
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
    undo.push({
      key: `note_folder:${folder.id}`, title: folder.name,
      undo: () => restoreNoteItem("note_folder", folder),
    });
  }

  function noteRemoved(note: Note) {
    setRemovedKey((key) => key + 1);
    undo.push({ key: `note:${note.id}`, title: note.title, undo: () => restoreNoteItem("note", note) });
  }

  /** Undo for a Note or folder: restore it, then let Notes and the folder tree ask again. */
  async function restoreNoteItem(kind: "note" | "note_folder", item: Note | NoteFolder) {
    await removedApi.restore(kind, item);
    if (kind === "note_folder") await noteFolders.refresh();
    setNotesRefresh((key) => key + 1);
    setRemovedKey((key) => key + 1);
  }

  function openLinkedNote(noteId: string) {
    setNotesRequest((previous) => ({ seq: (previous?.seq ?? 0) + 1, search: "", openId: noteId }));
    openNotes(ALL_NOTES);
  }

  function navigate(next: TaskView) {
    if (busy) return;
    setWorkspace("tasks");
    undo.hideNotice();
    newTaskToast.clearReveal();
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
    currentQuery.current = { ...currentQuery.current, view: next, sort: sortOf(viewSorts, next) };
    setView(next);
  }

  async function refreshLists() {
    showLists(await taskApi.lists());
  }

  function showLists(page: TaskListPage) {
    setLists(page.items);
    setListCursor(page.nextCursor);
    setListError(null);
  }

  function showAllTasks() {
    setTasks([]);
    setNextCursor(null);
    resetTaskPages();
    setLoading(true);
    const newView: TaskView = { kind: "all" };
    currentQuery.current = { ...currentQuery.current, view: newView, sort: sortOf(viewSorts, newView) };
    setView(newView);
  }

  /**
   * Shows in place what the CLI or an MCP client changed. Unlike retryFirstLoad
   * it never shows the loading state, which would unmount the rows, so what is
   * typed, the focus and the scroll position stay. A failed read says nothing:
   * resolving false has the next check offer the change again.
   */
  async function showOutsideChanges(): Promise<boolean> {
    if (busy || loading || loadError || !(await readTaskViewAgain())) return false;
    setRemovedKey((key) => key + 1);
    setNotesRefresh((key) => key + 1);
    if (notesOpened) void noteFolders.refresh();
    return true;
  }

  /**
   * Lists and the Task page showing, read again. A Task with unsaved text stays
   * on screen. A newer re-read, or anything the person did meanwhile, wins.
   */
  async function readTaskViewAgain(): Promise<boolean> {
    const read = ++outsideRead.current;
    const query = currentQuery.current;
    const cursor = currentPage.current;
    const generation = requestGeneration.current;
    const edited = tasks.filter((task) => drafts.get(task.id));
    const superseded = () =>
      read !== outsideRead.current || generation !== requestGeneration.current || cursor !== currentPage.current;
    try {
      const listPage = await readListsAgain(lists.length);
      let again: Awaited<ReturnType<typeof readPageAgain>>;
      try {
        again = await readPageAgain(cursor, query);
      } catch (reason) {
        // The List on screen went to Removed: the core says so for that List, on any page of Lists.
        if (query.view.kind !== "list" || !isListRemoved(reason)) throw reason;
        if (superseded()) return false;
        showLists(listPage);
        keptForEdits.current = new Set(edited.map((task) => task.id));
        if (edited.length === 0) {
          setExpandedId(null);
          showAllTasks();
        }
        return true;
      }
      const { page, stayOnPage } = again;
      const gone = edited.filter((task) => !page.items.some((item) => item.id === task.id));
      // Where each is now, if anywhere: a newer revision lets the row offer to keep the edits.
      const kept = await Promise.all(gone.map((task) =>
        taskApi.list(null, query.sort, { kind: "all" }, { ...NO_FILTERS, taskId: task.id })
          .then((found) => found.items[0] ?? task, () => task)));
      if (superseded()) return false;
      showLists(listPage);
      const items = [...kept, ...page.items];
      keptForEdits.current = new Set(kept.map((task) => task.id));
      setExpandedId((open) => items.some((task) => task.id === open) ? open : null);
      scrollAnchor.current = captureScrollAnchor(taskScroll.current, expandedNow.current);
      setTasks(items);
      setNextCursor(page.nextCursor);
      if (!stayOnPage) {
        setCurrentPageCursor(null);
        setPreviousPageCursors([]);
      }
      return true;
    } catch {
      return false;
    }
  }

  async function createList(name: string): Promise<boolean> {
    if (busyList) return false;
    setBusyList(true);
    setError(null);
    try {
      const created = await taskApi.createList(name);
      setLists((before) => [...before, created]);
      // This is a successful mutation, not a user navigation. busyList is still true.
      setTasks([]);
      setNextCursor(null);
      resetTaskPages();
      setExpandedId(null);
      setLoading(true);
      setLoadError(false);
      const newView: TaskView = { kind: "list", listId: created.id };
      currentQuery.current = { ...currentQuery.current, view: newView, sort: sortOf(viewSorts, newView) };
      setView(newView);
      try { await refreshLists(); } catch (reason) {
        setError("List created, but the sidebar could not refresh: " + errorMessage(reason));
      }
      // At the bottom it may be past the first page; the List just made stays in view.
      setLists((shown) => shown.some((item) => item.id === created.id) ? shown : [...shown, created]);
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

  async function styleList(list: TaskList, style: Style): Promise<boolean> {
    setBusyList(true);
    setError(null);
    try {
      const updated = await taskApi.setListStyle({ id: list.id, expectedRevision: list.revision, ...style });
      setLists((all) => all.map((item) => item.id === list.id ? updated : item));
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      return false;
    } finally {
      setBusyList(false);
    }
  }

  /**
   * Shows the new order at once; the sidebar stays usable, so focus stays on
   * the moved row. Resolves false when the core refused it.
   */
  async function reorderList(list: TaskList, afterId: string | null): Promise<boolean> {
    setLists((all) => {
      const others = all.filter((item) => item.id !== list.id);
      others.splice(afterId === null ? 0 : others.findIndex((item) => item.id === afterId) + 1, 0, list);
      return others;
    });
    setError(null);
    try {
      const updated = await taskApi.reorderList({ id: list.id, expectedRevision: list.revision, afterId });
      // Committed before this resolves, so the next key press reads the new revision.
      flushSync(() => setLists((all) => all.map((item) => item.id === list.id ? updated : item)));
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      // Whatever moved it or changed it elsewhere is what the sidebar should show now.
      try { await refreshLists(); } catch (refresh) {
        setError("The sidebar could not refresh: " + errorMessage(refresh));
      }
      return false;
    }
  }

  /** Rejects when the core refuses, so Undo can say so in whatever view is showing. */
  async function applyListRemoved(list: TaskList, removed: boolean) {
    setBusyList(true);
    setError(null);
    try {
      const updated = await taskApi.setListRemoved({
        id: list.id, expectedRevision: list.revision, removed,
      });
      setLists((all) => all.map((item) => item.id === list.id ? updated : item));
      setRemovedKey((key) => key + 1);
      if (removed) {
        undo.push({ key: `list:${updated.id}`, title: updated.name, undo: () => applyListRemoved(updated, false) });
      }
      if (removed && view.kind === "list" && view.listId === list.id) {
        showAllTasks();
      } else {
        retryFirstLoad();
      }
      try { await refreshLists(); } catch (reason) {
        setError("List updated, but the sidebar could not refresh: " + errorMessage(reason));
      }
    } finally {
      setBusyList(false);
    }
  }

  async function setListRemoved(list: TaskList, removed: boolean) {
    try {
      await applyListRemoved(list, removed);
    } catch (reason) {
      setError(errorMessage(reason));
    }
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
    const form = event.currentTarget;
    setCreating(true);
    setError(null);
    try {
      const created = await taskApi.create(
        { title, description },
        view.kind === "list" ? view.listId : undefined,
      );
      sound.play("create");
      newTaskToast.added(created.id);
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
      // The field was disabled while saving, which drops focus: hand it back for the next Task.
      if (document.activeElement === document.body || form.contains(document.activeElement)) {
        setFocusRequest((previous) => ({ field: "capture", seq: (previous?.seq ?? 0) + 1 }));
      }
    }
  }

  async function updateTask(task: Task, nextTitle: string, nextDescription: string, expectedRevision: number) {
    setSavingId(task.id);
    setError(null);
    try {
      const updated = await taskApi.update({
        id: task.id,
        title: nextTitle,
        description: nextDescription,
        expectedRevision,
      });
      requestGeneration.current += 1;
      setTasks((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      drafts.set(task.id, undefined);
      setExpandedId(null);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setSavingId(null);
    }
  }

  /**
   * Resolves true once the mutation itself succeeded. Rejects when the core
   * refuses, so Undo can say so in whatever view is showing.
   */
  async function applyTaskMutation(task: Task, mutate: () => Promise<Task>): Promise<boolean> {
    if (savingId) return false;
    setSavingId(task.id);
    setError(null);
    // Paging is disabled while saving, so this stays the visible page.
    const queryAtStart = currentQuery.current;
    const startCursor = currentPage.current;
    try {
      const updated = await mutate();
      // The core decides completion (status or 100% progress); the cue follows its answer.
      // Won't Do is not finishing: setting a Task aside plays nothing, and
      // completing a Won't Do Task for real does play the cue.
      const finished = (item: Task) => item.status === "completed" && !item.wontDo;
      if (!finished(task) && finished(updated)) sound.play("complete");
      const generation = ++requestGeneration.current;
      const { sort: latestSort, view: latestView, filters: latestFilters } = currentQuery.current;
      // A cursor belongs to the query that produced it; a new query starts on page 1.
      const pageCursor = currentQuery.current === queryAtStart ? startCursor : null;
      setTasks((current) => current.map((item) => item.id === updated.id ? updated : item));
      // The mutation already succeeded; never let an old page replace its result.
      try {
        const { page, stayOnPage } = await readPageAgain(
          pageCursor, { sort: latestSort, view: latestView, filters: latestFilters },
        );
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
    } finally {
      setSavingId(null);
    }
  }

  /** A change made in the Task view: a refusal shows in its banner. */
  async function commitTaskMutation(task: Task, mutate: () => Promise<Task>): Promise<boolean> {
    try {
      return await applyTaskMutation(task, mutate);
    } catch (reason) {
      setError(errorMessage(reason));
      return false;
    }
  }

  /**
   * A save of typed edits: once it succeeds, the edits it carried are no longer
   * unsaved. They go with the new revision, not after the list refresh, or the
   * row would take its own save for a change made elsewhere.
   */
  async function saveEdits(task: Task, mutate: () => Promise<Task>) {
    await commitTaskMutation(task, async () => {
      const updated = await mutate();
      drafts.set(task.id, undefined);
      return updated;
    });
  }

  async function changeTask(task: Task, change: TaskChange, expectedRevision: number) {
    await saveEdits(task, () => taskApi.change({ id: task.id, expectedRevision, ...change }));
  }

  async function recordProgress(task: Task, update: ProgressDraft, expectedRevision: number) {
    await saveEdits(task, () => taskApi.recordProgress({ id: task.id, expectedRevision, ...update }));
  }

  async function moveTask(task: Task, listId: string | null) {
    await commitTaskMutation(task, () => taskApi.move({
      id: task.id, expectedRevision: task.revision, listId,
    }));
  }

  async function updateDetails(task: Task, tags: string[], dueDate: string | null, expectedRevision: number) {
    await saveEdits(task, () => taskApi.details({ id: task.id, expectedRevision, tags, dueDate }));
  }

  async function removeTask(task: Task) {
    const done: { task?: Task } = {};
    const saved = await commitTaskMutation(task, async () => (done.task = await removedApi.remove<Task>("task", task)));
    if (saved && done.task) {
      setExpandedId(null);
      newTaskToast.forget(task.id);
      const removed = done.task;
      undo.push({
        key: `task:${removed.id}`, title: removed.title,
        undo: async () => {
          if (await applyTaskMutation(removed, () => removedApi.restore<Task>("task", removed))) {
            setRemovedKey((key) => key + 1);
          }
        },
      });
      setRemovedKey((key) => key + 1);
    }
  }

  /** Shows the view in `nextSort` and remembers it for this view. */
  function changeSort(nextSort: SortMode) {
    if (sort === nextSort) return;
    newTaskToast.clearReveal();
    currentQuery.current = { ...currentQuery.current, sort: nextSort };
    requestGeneration.current += 1;
    setError(null);
    setRefreshError(false);
    setLoading(true);
    setNextCursor(null);
    resetTaskPages();
    setViewSorts((sorts) => ({ ...sorts, [viewKey(view)]: nextSort }));
    taskApi.setViewSort(view, nextSort).catch((reason) => {
      setError("Sorted, but Tasknotes could not remember it for next time: " + errorMessage(reason));
    });
  }

  // One move at a time: a second would name the revision the first is replacing.
  const placing = useRef(false);

  /**
   * Moves a Task within its group, showing the new order at once. Sorted by
   * anything but Custom, the core starts the List's Custom Order from the
   * order on screen and the view switches to Custom. Resolves false when
   * nothing moved or the core refused it.
   */
  async function placeTask(task: Task, place: TaskPlace): Promise<boolean> {
    if (placing.current || savingId || creating || loadingMore || loading) return false;
    const placed = placeLocally(tasks, task.id, place);
    const order = (items: Task[]) => items.map((item) => item.id).join(" ");
    if (order(placed) === order(tasks)) return false;
    const shownSort = sort;
    const shownView = view;
    placing.current = true;
    // What an outside re-read brought back in the meantime must not undo this.
    requestGeneration.current += 1;
    setError(null);
    setTasks(placed);
    try {
      const updated = await taskApi.reorder({ id: task.id, expectedRevision: task.revision, ...place, shownSort });
      // Committed before this resolves, so the next key press reads the new revision.
      flushSync(() => setTasks((current) => current.map((item) => item.id === updated.id ? updated : item)));
      if (shownSort !== "custom") {
        // The core switched the List to Custom; showing it reads the first page again in that order.
        currentQuery.current = { ...currentQuery.current, sort: "custom" };
        setViewSorts((sorts) => ({ ...sorts, [viewKey(shownView)]: "custom" }));
      }
      return true;
    } catch (reason) {
      setError(errorMessage(reason));
      // Whatever moved or changed it elsewhere is what the list should show now.
      const generation = ++requestGeneration.current;
      try {
        const { page, stayOnPage } = await readPageAgain(currentPage.current, currentQuery.current);
        if (generation !== requestGeneration.current) return false;
        setTasks(page.items);
        setNextCursor(page.nextCursor);
        if (!stayOnPage) {
          setCurrentPageCursor(null);
          setPreviousPageCursors([]);
        }
      } catch {
        if (generation === requestGeneration.current) setTasks(tasks);
      }
      return false;
    } finally {
      placing.current = false;
    }
  }

  /** Start a fresh first page for new search text or filters. */
  function applyFilters(next: TaskFilters) {
    if (sameFilters(next, currentQuery.current.filters)) return;
    setCreatedHidden(false);
    newTaskToast.clearReveal();
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

  // While the palette is open the window behind it is inert: its shortcuts are swallowed.
  const behindPalette = (handler: () => boolean | void) => () => paletteOpen || handler();
  useShortcuts({
    palette: openPalette,
    newTask: behindPalette(() => focusInTasks("capture")),
    search: behindPalette(() => focusInTasks("search")),
    settings: behindPalette(openSettings),
    undo: behindPalette(undo.run),
  });

  // The global Quick Capture shortcut lands in the ordinary New task field, like ⌘N.
  const quickCapture = useRef(() => {});
  quickCapture.current = () => {
    setPaletteOpen(false);
    focusInTasks("capture");
  };
  useEffect(() => {
    const stop = listenForQuickCapture(() => quickCapture.current());
    return () => void stop.then((unlisten) => unlisten());
  }, []);

  // A saved Quick Capture shortcut another app took since the last launch: said once, never blocking.
  const [launchNotice, setLaunchNotice] = useState<string | null>(null);
  useEffect(() => {
    shellApi.takeLaunchNotice().then(setLaunchNotice).catch(() => {});
  }, []);

  const commands = buildCommands({
    lists, undoTitle: undo.next?.title ?? null, appearance: appearance.preference,
    navigate, openNotes: () => openNotes(), openRemoved, openSettings, focusInTasks,
    undo: undo.run, setAppearance: appearance.setPreference,
  });
  const searchCommand = (text: string): Command => ({
    id: "search-text", label: `Search for “${text}”`, group: "Actions",
    run: () => { focusInTasks("search"); setSearchDraft(text); },
  });

  const keepMounted = useMemo(() => new Set(expandedId ? [expandedId] : []), [expandedId]);

  const pageTitle = view.kind === "all" ? "All tasks"
    : view.kind === "inbox" ? "Inbox"
    : lists.find((list) => list.id === view.listId)?.name ?? "List";
  const today = useLocalCalendarDay();

  const filtering = !sameFilters(filters, NO_FILTERS);
  // Tasks move only in one List or Inbox with nothing filtered: there the
  // whole group is what the person sees (ADR 0010).
  const canPlace = view.kind !== "all" && !filtering;
  const filterControlsActive = filtering || searchDraft.trim() !== "" || tagDraft.trim() !== "";
  const captureOpen = title.length > 0 || description.length > 0;
  const captureDisabled = creating || Boolean(savingId) || loading || loadError;
  // After a refused search the rows on hand belong to the previous query.
  const countKnown = !(loadError && filtering);

  return (
    <div className="app-shell" inert={paletteOpen} ref={sidebar.shellRef}
      style={{ "--sidebar-width": `${sidebar.width}px` } as CSSProperties}>
      <TaskSidebar
        view={view} notesActive={workspace !== "tasks"} onOpenNotes={() => openNotes()}
        removedActive={workspace === "removed"} onOpenRemoved={openRemoved}
        pendingApprovals={approvals.count} approvalsActive={workspace === "approvals"}
        onOpenApprovals={openApprovals}
        allNotesCurrent={workspace === "notes" && noteScope.kind === "all"}
        noteFolders={notesOpened && (
          <NoteFolderTree folders={noteFolders}
            currentId={workspace === "notes" && noteScope.kind === "folder" ? noteScope.folderId : null}
            disabled={busy}
            onOpen={(folder) => openNotes({ kind: "folder", folderId: folder.id })}
            onRemoved={folderRemoved} />
        )}
        lists={lists} shownCount={countKnown ? tasks.length : null} hasMoreTasks={Boolean(nextCursor)}
        listError={listError}
        hasMoreLists={Boolean(listCursor)} busy={busy}
        theme={appearance.theme} onToggleTheme={appearance.toggle}
        settingsActive={workspace === "settings"} onOpenSettings={openSettings}
        onNavigate={navigate} onCreateList={createList} onRenameList={renameList}
        onReorderList={reorderList} onStyleList={styleList}
        onRemoveList={(list) => setListRemoved(list, true)}
        onLoadMoreLists={loadMoreLists}
        resizer={<SidebarResizer width={sidebar.width} max={sidebar.max}
          onPreview={sidebar.preview} onResize={sidebar.setWidth} />}
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
              <option value="difficulty">Hardest first</option>
              <option value="newest">Newest first</option>
              {view.kind !== "all" && <option value="custom">Custom order</option>}
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
                    ...currentQuery.current.filters, status: event.target.value as TaskFilters["status"],
                  })}>
                  <option value="">Any status</option>
                  {STATUS_CHOICES.map((choice) => (
                    <option key={choice} value={choice}>{statusChoiceLabel(choice)}</option>
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
              <label className={`property filter-chip ${filters.difficulty ? "filter-chip--active" : ""}`}>
                <select aria-label="Filter by difficulty" value={filters.difficulty}
                  onChange={(event) => applyFilters({
                    ...currentQuery.current.filters,
                    difficulty: event.target.value as TaskFilters["difficulty"],
                  })}>
                  <option value="">Any difficulty</option>
                  {[...DIFFICULTY_ORDER].reverse().map((difficulty) => (
                    <option key={difficulty} value={difficulty}>{DIFFICULTY_VIEW[difficulty].label}</option>
                  ))}
                  <option value="none">No difficulty</option>
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

            {launchNotice && (
              <div role="status" aria-label="Quick Capture shortcut" className="notice-banner">
                <span>{launchNotice} {newTaskKeys} still creates a task.</span>
                <div className="notice-actions">
                  <button className="button button-small" type="button"
                    onClick={() => { setLaunchNotice(null); openSettings(); }}>
                    Open Settings
                  </button>
                  <button className="button button-small" type="button" onClick={() => setLaunchNotice(null)}>
                    Dismiss
                  </button>
                </div>
              </div>
            )}

            {pendingAlone && (
              <div role="status" aria-label={ALONE_BANNER[pendingAlone.reason].opening} className="notice-banner">
                <LoaderCircle className="spin" size={14} aria-hidden="true" />
                <span>{ALONE_BANNER[pendingAlone.reason].openingText}</span>
              </div>
            )}

            {filters.taskId && (
              <div role="status" aria-label={ALONE_BANNER[shownAlone].shown} className="notice-banner">
                <span>{ALONE_BANNER[shownAlone].shownText}</span>
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
                <h2 className="task-group-heading">Notes</h2>
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
                <div className="empty-state" role="group" aria-label="Search unavailable">
                  <SearchX size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h2>Search couldn't run</h2>
                  <p>Check the message above, or clear the search and filters to see your tasks.</p>
                  <div className="empty-state-actions">
                    <button className="button button-small" type="button" onClick={clearFilters}>
                      Clear search and filters
                    </button>
                    <button className="button button-small" type="button" onClick={retryFirstLoad}>Try again</button>
                  </div>
                </div>
              ) : loadError && tasks.length === 0 ? (
                <div className="empty-state" role="group" aria-label="Tasks unavailable">
                  <LockKeyhole size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h2>Couldn't load your tasks</h2>
                  <p>The local database couldn't be opened. Nothing has been changed; check the message above and try again.</p>
                  <button className="button button-small" type="button" onClick={retryFirstLoad}>Try again</button>
                </div>
              ) : tasks.length === 0 && previousPageCursors.length === 0 && filtering ? (
                <div className="empty-state">
                  <SearchX size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h2>No matching tasks</h2>
                  <p>Nothing in this view matches the search and filters.</p>
                  <button className="button button-small" type="button" onClick={clearFilters}>
                    Clear search and filters
                  </button>
                </div>
              ) : tasks.length === 0 && previousPageCursors.length === 0 ? (
                <div className="empty-state">
                  <CheckCircle2 size={26} strokeWidth={1.5} aria-hidden="true" />
                  <h2>Your list is clear</h2>
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
                    flashId={newTaskToast.flashId}
                    reveal={newTaskToast.reveal}
                    onRevealed={newTaskToast.clearReveal}
                    onPlace={canPlace ? placeTask : undefined}
                    renderTask={(task) => (
                      <TaskRow
                        task={task}
                        expanded={expandedId === task.id}
                        saving={savingId === task.id}
                        onToggle={() => setExpandedId((current) => current === task.id ? null : task.id)}
                        onSave={(nextTitle, nextDescription, base) => updateTask(task, nextTitle, nextDescription, base)}
                        onChange={(change, base) => changeTask(task, change, base)}
                        onRecordProgress={(update, base) => recordProgress(task, update, base)}
                        lists={lists.filter((list) => !list.removed)}
                        today={today}
                        onMove={(listId) => moveTask(task, listId)}
                        onUpdateDetails={(tags, dueDate, base) => updateDetails(task, tags, dueDate, base)}
                        onOpenNote={openLinkedNote}
                        onRemove={() => removeTask(task)}
                        shown={workspace === "tasks"}
                        edits={drafts.get(task.id)}
                        onEdits={(edits) => drafts.set(task.id, edits)}
                        reorderable={canPlace}
                      />
                    )}
                  />
                  {(previousPageCursors.length > 0 || nextCursor) && (
                    <nav className="task-page-navigation" aria-label="Task pages">
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
                    </nav>
                  )}
                </>
              )}
            </section>
          </div>
        </div>
        <NewTaskToast toast={newTaskToast} onOpen={openNewTask} />
      </main>
      <SettingsView hidden={workspace !== "settings"} preference={appearance.preference}
        onPreference={appearance.setPreference}>
        <SoundSettings sound={sound} />
      </SettingsView>
      <RemovedView hidden={workspace !== "removed"} refreshKey={removedKey} onRestored={restored} />
      <ApprovalsView hidden={workspace !== "approvals"} refreshKey={approvals.count}
        onDecided={bulkDecided} />
      {notesOpened && (
        <Suspense fallback={workspace === "notes" &&
          <div className="loading-state" role="status"><LoaderCircle className="spin" size={16} /> Loading notes...</div>}>
          <NotesWorkspace hidden={workspace !== "notes"} scope={noteScope}
            scopeName={noteScope.kind === "folder"
              ? noteFolders.byId.get(noteScope.folderId)?.name ?? "Folder" : "Notes"}
            folders={noteFolders.options} request={notesRequest}
            onOpenTask={(taskId) => showTaskAlone(taskId, "linked")}
            refreshKey={notesRefresh} onRemoved={noteRemoved}
            onScopeHidden={() => setNoteScope(ALL_NOTES)} />
        </Suspense>
      )}
      {paletteOpen && (
        <CommandPalette commands={commands} textCommand={searchCommand} onClose={closePalette} />
      )}
      {undo.error && (
        <div role="alert" className="undo-bar undo-bar--error">
          <span>{undo.error}</span>
          <button className="button button-small" type="button" onClick={undo.dismissError}>
            Dismiss
          </button>
        </div>
      )}
      {undo.noticeShown && undo.next && (
        <div role="status" aria-label="Moved to Removed" className="undo-bar">
          <span>“{undo.next.title}” moved to Removed</span>
          <button className="button button-small" type="button" disabled={undo.undoing || Boolean(savingId)}
            onClick={undo.run}>
            Undo
          </button>
        </div>
      )}
    </div>
  );
}
