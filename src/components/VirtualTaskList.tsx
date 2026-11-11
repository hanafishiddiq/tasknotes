import {
  memo, useCallback, useLayoutEffect, type KeyboardEvent, useMemo, useRef, useState, type ReactNode, type RefObject,
} from "react";
import {
  defaultRangeExtractor, measureElement, observeElementRect, useVirtualizer, type Range, type Virtualizer,
} from "@tanstack/react-virtual";
import { TASK_PAGE_SIZE, type Task, type TaskPlace, type WorkStatus } from "../lib/task-api";
import { STATUS_ORDER, STATUS_VIEW, shownOrder } from "../lib/task-visuals";
import { useRowDrag, watchRefusedDrag, type RowDrop } from "../lib/use-row-drag";

type Item =
  | { kind: "heading"; status: WorkStatus; count: number }
  | { kind: "task"; task: Task };

/** A Task its check just sent to another group, and the place it showed at in `shownOrder`. */
export interface CheckedTask {
  taskId: string;
  slot: number;
}

interface VirtualTaskListProps {
  /** One page of Tasks; the list groups them In Progress, Ready, Completed. */
  tasks: Task[];
  /** More Tasks exist beyond this page, so group counts are lower bounds. */
  hasMore: boolean;
  /** The element that scrolls the Task view. */
  scrollRef: RefObject<HTMLElement | null>;
  /**
   * Tasks that must stay mounted however far away the view scrolls (the open
   * one). Unsaved edits are kept above the rows, so they never depend on this.
   */
  keepMounted: ReadonlySet<string>;
  renderTask: (task: Task) => ReactNode;
  /** The open Task, which Escape closes. */
  expandedId: string | null;
  onCollapse: () => void;
  /**
   * A Task to scroll to and focus, when the person asked for it. One try only: hit or
   * miss, `onRevealed` drops it, so a Task arriving later never moves the list.
   */
  reveal?: { taskId: string } | null;
  onRevealed?: () => void;
  /**
   * Given only where a Task may move (one List, nothing filtered): a Task row
   * is dragged, or moved with Alt+Up/Down, within its work-status group.
   * Resolves whether the move was made.
   */
  onPlace?: (task: Task, place: TaskPlace) => Promise<boolean>;
  /**
   * Given where a Task may not move (several Lists, or a filter on): a try,
   * by a drag or Alt+Up/Down, calls this instead, and nothing moves.
   */
  onMoveBlocked?: () => void;
  /** Says a move to screen readers, through the Task view's polite live region. */
  onAnnounce: (text: string) => void;
  /**
   * A Task whose check moved it to another group: focus, left behind, goes to the
   * title of the row now in its place (the last row, past the end), without a
   * scroll. One try only: `onCheckSettled` drops it.
   */
  checked?: CheckedTask | null;
  onCheckSettled?: () => void;
}

/**
 * A hidden view (Notes or Settings on top) has no height. Keep the last real
 * height, or the window's, so the rows and the open Task stay mounted rather
 * than collapsing to nothing and losing what was typed.
 */
function observeVisibleRect(
  instance: Virtualizer<HTMLElement, Element>,
  report: (rect: { width: number; height: number }) => void,
) {
  let last = 0;
  return observeElementRect(instance, (rect) => {
    if (rect.height > 0) last = rect.height;
    report({ width: rect.width, height: rect.height > 0 ? rect.height : last || window.innerHeight });
  });
}

const HEADING_ESTIMATE = 30;
const ROW_ESTIMATE = 52;

/** A hidden row measures 0; keep an estimate so rows never collapse onto one point. */
function measureShown(
  element: Element, entry: ResizeObserverEntry | undefined, instance: Virtualizer<HTMLElement, Element>,
): number {
  const size = measureElement(element, entry, instance);
  if (size > 0) return size;
  return (element as HTMLElement).dataset.heading !== undefined ? HEADING_ESTIMATE : ROW_ESTIMATE;
}
/**
 * Scrolling re-renders the list each time a row enters or leaves the range. A row
 * only moved, so it keeps its render until its Task or `renderTask` changes.
 */
const TaskSlotContent = memo(function TaskSlotContent({ task, renderTask }: {
  task: Task;
  renderTask: (task: Task) => ReactNode;
}) {
  return renderTask(task);
});

/**
 * The Task rows of the current page, rendered only near what is on screen.
 * Rows stay in normal flow (spacers stand in for the rest), so each grouped
 * section keeps its own landmark and rows grow freely when they open.
 */
export default function VirtualTaskList({
  tasks, hasMore, scrollRef, keepMounted, renderTask, expandedId, onCollapse,
  reveal = null, onRevealed, onPlace, onMoveBlocked, onAnnounce, checked = null, onCheckSettled,
}: VirtualTaskListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  // The row keyboard focus is moving to, until focus lands there, and which of its
  // controls takes it: the title, or the check when the arrows started on a check.
  const [focusTarget, setFocusTarget] = useState<string | null>(null);
  const focusPart = useRef<"task-open" | "task-check">("task-open");
  // The row holding focus now; it stays mounted however far the list scrolls.
  const [focusedId, setFocusedId] = useState<string | null>(null);

  const items = useMemo(() => STATUS_ORDER.flatMap((status): Item[] => {
    const group = tasks.filter((task) => task.status === status);
    if (group.length === 0) return [];
    return [
      { kind: "heading", status, count: group.length },
      ...group.map((task): Item => ({ kind: "task", task })),
    ];
  }), [tasks]);

  const byId = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);

  /** The Tasks of one group, in order: what a move is placed among. */
  const groupOf = (status: WorkStatus) => tasks.filter((task) => task.status === status);

  function place(task: Task, where: TaskPlace) {
    if (!onPlace) return;
    const anchorId = "beforeId" in where ? where.beforeId : where.afterId;
    const anchor = anchorId === null ? undefined : byId.get(anchorId);
    void onPlace(task, where).then((placed) => {
      if (!placed) return;
      onAnnounce(anchor === undefined ? `${task.title} moved to the top of ${STATUS_VIEW[task.status].label}`
        : `${task.title} moved ${"beforeId" in where ? "before" : "after"} ${anchor.title}`);
    });
  }

  const drag = useRowDrag({
    container: listRef,
    scrollArea: scrollRef,
    canNest: false,
    // Within its own group only: a drop elsewhere never changes a status.
    canTarget: (dragged, id) => dragged !== id && byId.get(dragged)?.status === byId.get(id)?.status,
    onDrop(dragged: string, drop: RowDrop) {
      const task = byId.get(dragged);
      if (!task || drop.edge === "into") return;
      if (drop.id === null) {
        // Below every row: after the last one, when that is in the same group.
        const last = tasks[tasks.length - 1];
        if (last && last.id !== task.id && last.status === task.status) place(task, { afterId: last.id });
        return;
      }
      place(task, drop.edge === "before" ? { beforeId: drop.id } : { afterId: drop.id });
    },
  });

  const pinned = useMemo(() => items.flatMap((item, index) =>
    item.kind === "task" && (keepMounted.has(item.task.id) || item.task.id === focusTarget || item.task.id === focusedId)
      ? [index] : []), [items, keepMounted, focusTarget, focusedId]);

  const rangeExtractor = useCallback((range: Range) => {
    const visible = defaultRangeExtractor(range);
    if (pinned.length === 0) return visible;
    return [...new Set([...visible, ...pinned])].sort((a, b) => a - b);
  }, [pinned]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => items[index]?.kind === "heading" ? HEADING_ESTIMATE : ROW_ESTIMATE,
    getItemKey: (index) => {
      const item = items[index];
      return item.kind === "heading" ? `heading:${item.status}` : item.task.id;
    },
    // A page holds at most this many Tasks. Keeping them all mounted took dropped
    // frames in a real scroll from about 50 to about 15: mounting rows costs more than they weigh.
    overscan: TASK_PAGE_SIZE,
    scrollMargin,
    rangeExtractor,
    observeElementRect: observeVisibleRect,
    measureElement: measureShown,
  });

  // Capture, banners and Note matches sit above the rows in the same scroll area.
  // No dependency list on purpose: any of them can change height, and each such
  // change re-renders this list. Reading offsets is cheap, and state is only set
  // when the margin really moved, so this cannot loop.
  useLayoutEffect(() => {
    const list = listRef.current;
    const area = scrollRef.current;
    if (!list || !area) return;
    // Layout offsets, not on-screen positions, so scrolling does not change it.
    let margin = 0;
    for (let at: HTMLElement | null = list; at && at !== area; at = at.offsetParent as HTMLElement | null) {
      margin += at.offsetTop;
    }
    if (margin !== scrollMargin) setScrollMargin(margin);
  });

  function focusTask(id: string, part: "task-open" | "task-check" = "task-open") {
    const index = items.findIndex((item) => item.kind === "task" && item.task.id === id);
    if (index < 0) return;
    // Always instant: a smooth scroll would ignore Reduce Motion, which CSS cannot reach here.
    virtualizer.scrollToIndex(index, { align: "auto", behavior: "auto" });
    focusPart.current = part;
    setFocusTarget(id);
  }

  /** Focus goes to a row's title, the usual way in. */
  function focusTitle(id: string) {
    focusPart.current = "task-open";
    setFocusTarget(id);
  }

  // Up/Down/Home/End move between rows (headings are skipped), from a row's title or
  // its check to the same control in the next row; Escape closes the open Task.
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement;
    const id = target.closest<HTMLElement>("[data-task-id]")?.dataset.taskId;
    if (!id) return;
    if (event.key === "Escape" && id === expandedId) {
      event.preventDefault();
      onCollapse();
      focusTitle(id);
      return;
    }
    const onCheck = target.classList.contains("task-check");
    if (!target.classList.contains("task-open") && !onCheck) return;
    // Alt with the arrows moves a Task from its title only, never from its check.
    if (onCheck && event.altKey) return;
    if (!onPlace && onMoveBlocked && event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      // Not here: say why, and leave the list and focus where they are.
      event.preventDefault();
      onMoveBlocked();
      return;
    }
    if (onPlace && event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      // Alt with the arrows moves the Task itself, within its group.
      event.preventDefault();
      const task = byId.get(id);
      if (!task) return;
      const group = groupOf(task.status);
      const at = group.findIndex((item) => item.id === id);
      const neighbour = group[event.key === "ArrowUp" ? at - 1 : at + 1];
      if (!neighbour) return;
      place(task, event.key === "ArrowUp" ? { beforeId: neighbour.id } : { afterId: neighbour.id });
      // The row moves in the page; focus follows it.
      focusTitle(id);
      return;
    }
    const order = items.flatMap((item) => item.kind === "task" ? [item.task.id] : []);
    const at = order.indexOf(id);
    const next = event.key === "ArrowDown" ? order[at + 1]
      : event.key === "ArrowUp" ? order[at - 1]
      : event.key === "Home" ? order[0]
      : event.key === "End" ? order[order.length - 1]
      : undefined;
    if (next === undefined) return;
    event.preventDefault();
    focusTask(next, onCheck ? "task-check" : "task-open");
  }

  // Asked for from the new Task toast, so moving the list is the person's doing. A
  // view still loading has no list yet; the first list it shows gets the one try.
  // `virtualizer` keeps one instance; `onRevealed` must be stable, or this reruns each render.
  useLayoutEffect(() => {
    if (!reveal) return;
    const index = items.findIndex((item) => item.kind === "task" && item.task.id === reveal.taskId);
    if (index >= 0) {
      // Always instant, as in focusTask; the top of the open Task lands at the top of the view.
      virtualizer.scrollToIndex(index, { align: "start", behavior: "auto" });
      focusTitle(reveal.taskId);
    }
    onRevealed?.();
  }, [reveal, items, virtualizer, onRevealed]);

  // An open Task that closed itself (a save, or a click, which gives a button no focus
  // in WebKit) leaves focus on the page: its row gets focus instead. Focus only, no
  // scrollToIndex: the row is on the page, and its old height would throw the list up for a frame.
  const wasExpanded = useRef(expandedId);
  useLayoutEffect(() => {
    const closed = wasExpanded.current;
    wasExpanded.current = expandedId;
    if (closed && !expandedId && document.activeElement === document.body) focusTitle(closed);
  }, [expandedId]);

  // A check sent its Task to another group, so its row left. Focus that was left
  // behind (on the page, or still in that row) goes to the title of the row now in
  // its place, or the last row past the end; never a scroll (PR #21), as the
  // person did not move the list. Focus put elsewhere since stays where it is.
  useLayoutEffect(() => {
    if (!checked) return;
    onCheckSettled?.();
    const active = document.activeElement;
    const leftBehind = active === null || active === document.body ||
      active.closest(`[data-task-id="${CSS.escape(checked.taskId)}"]`) !== null;
    if (!leftBehind) return;
    const order = shownOrder(tasks);
    const now = order[Math.min(checked.slot, order.length - 1)];
    if (!now) return;
    listRef.current?.querySelector<HTMLElement>(`[data-task-id="${CSS.escape(now.id)}"] .task-open`)
      ?.focus({ preventScroll: true });
  }, [checked, tasks, onCheckSettled]);

  const virtualItems = virtualizer.getVirtualItems();
  const total = virtualizer.getTotalSize();

  // The target row is pinned, so it enters the page within a render or two of
  // being asked for; once focus lands there it is let go.
  useLayoutEffect(() => {
    if (!focusTarget) return;
    const slot = listRef.current?.querySelector<HTMLElement>(`[data-task-id="${CSS.escape(focusTarget)}"]`);
    if (!slot) return;
    // A check that is held (a save on its way) cannot take focus: the title does.
    const row = (focusPart.current === "task-check" ? slot.querySelector<HTMLElement>(".task-check:not(:disabled)") : null)
      ?? slot.querySelector<HTMLElement>(".task-open");
    if (!row) return;
    row.focus();
    setFocusTarget(null);
  }, [focusTarget, virtualItems]);

  // Group what is rendered into its status sections, with spacers for the rest.
  const sections: { status: WorkStatus; children: ReactNode[] }[] = [];
  let previousEnd = scrollMargin;
  for (const virtual of virtualItems) {
    const item = items[virtual.index];
    if (!item) continue;
    const status = item.kind === "heading" ? item.status : item.task.status;
    let section = sections[sections.length - 1];
    if (!section || section.status !== status) {
      section = { status, children: [] };
      sections.push(section);
    }
    const gap = virtual.start - previousEnd;
    if (gap > 0) {
      section.children.push(<div key={`gap:${virtual.key}`} aria-hidden="true" style={{ height: gap }} />);
    }
    previousEnd = virtual.end;
    section.children.push(item.kind === "heading" ? (
      <h2 key={virtual.key} data-index={virtual.index} data-heading=""
        // Space above a later group is part of its heading, so it is measured.
        className={`task-group-heading ${virtual.index > 0 ? "task-group-heading--later" : ""}`}
        ref={virtualizer.measureElement}>
        {STATUS_VIEW[item.status].label}
        <span>{item.count}{hasMore ? "+" : ""}</span>
      </h2>
    ) : (
      <div key={virtual.key} className="task-slot"
        data-index={virtual.index}
        data-drag-row={onPlace ? item.task.id : undefined}
        data-dragging={drag.dragging === item.task.id || undefined}
        data-drop-edge={dropEdge(item.task.id)}
        data-task-id={item.task.id} ref={virtualizer.measureElement}>
        <TaskSlotContent task={item.task} renderTask={renderTask} />
      </div>
    ));
  }
  const tail = Math.max(0, total + scrollMargin - previousEnd);

  function dropEdge(id: string) {
    if (drag.drop?.id === id) return drag.drop.edge;
    // Below every row lands after the last one, only when that is in the dragged Task's group.
    const last = tasks[tasks.length - 1];
    return drag.drop?.edge === "end" && id === last?.id && drag.dragging !== null &&
      byId.get(drag.dragging)?.status === last.status ? "after" : undefined;
  }

  return (
    <div className="task-list" ref={listRef} onKeyDown={onKeyDown}
      onMouseDown={(onPlace || onMoveBlocked) && ((event) => {
        // A drag starts on the row's title area, never on a control inside an open Task.
        const id = (event.target as HTMLElement).closest(".task-open")
          ?.closest<HTMLElement>("[data-task-id]")?.dataset.taskId;
        if (!id) return;
        if (onPlace) drag.start(event, id);
        else if (onMoveBlocked) watchRefusedDrag(event, onMoveBlocked);
      })}
      onFocus={(event) => setFocusedId(
        (event.target as HTMLElement).closest<HTMLElement>("[data-task-id]")?.dataset.taskId ?? null)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocusedId(null);
      }}>
      {sections.map(({ status, children }) => (
        <section key={status} className="task-group" aria-label={`${STATUS_VIEW[status].label} tasks`}>
          {children}
        </section>
      ))}
      {tail > 0 && <div aria-hidden="true" style={{ height: tail }} />}
    </div>
  );
}
