import {
  useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject,
} from "react";
import {
  defaultRangeExtractor, measureElement, observeElementRect, useVirtualizer, type Range, type Virtualizer,
} from "@tanstack/react-virtual";
import type { Task, WorkStatus } from "../lib/task-api";
import { STATUS_ORDER, STATUS_VIEW } from "../lib/task-visuals";

type Item =
  | { kind: "heading"; status: WorkStatus; count: number }
  | { kind: "task"; task: Task };

interface VirtualTaskListProps {
  /** One page of Tasks; the list groups them In Progress, Ready, Completed. */
  tasks: Task[];
  /** More Tasks exist beyond this page, so group counts are lower bounds. */
  hasMore: boolean;
  /** The element that scrolls the Task view. */
  scrollRef: RefObject<HTMLElement | null>;
  /**
   * Tasks that must stay mounted however far away the view scrolls: the open
   * one and any holding unsaved edits, so nothing typed is ever thrown away.
   */
  keepMounted: ReadonlySet<string>;
  renderTask: (task: Task) => ReactNode;
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
  return element.tagName === "H3" ? HEADING_ESTIMATE : ROW_ESTIMATE;
}
/**
 * The Task rows of the current page, rendered only near what is on screen.
 * Rows stay in normal flow (spacers stand in for the rest), so each grouped
 * section keeps its own landmark and rows grow freely when they open.
 */
export default function VirtualTaskList({
  tasks, hasMore, scrollRef, keepMounted, renderTask,
}: VirtualTaskListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);

  const items = useMemo<Item[]>(() => STATUS_ORDER.flatMap((status) => {
    const group = tasks.filter((task) => task.status === status);
    if (group.length === 0) return [];
    return [
      { kind: "heading", status, count: group.length } as Item,
      ...group.map((task): Item => ({ kind: "task", task })),
    ];
  }), [tasks]);

  const pinned = useMemo(() => items.flatMap((item, index) =>
    item.kind === "task" && keepMounted.has(item.task.id) ? [index] : []), [items, keepMounted]);

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
    overscan: 6,
    scrollMargin,
    rangeExtractor,
    observeElementRect: observeVisibleRect,
    measureElement: measureShown,
  });

  // Capture, banners and Note matches sit above the rows in the same scroll area.
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

  const virtualItems = virtualizer.getVirtualItems();
  const total = virtualizer.getTotalSize();

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
      <h3 key={virtual.key} data-index={virtual.index}
        // Space above a later group is part of its heading, so it is measured.
        className={`task-group-heading ${virtual.index > 0 ? "task-group-heading--later" : ""}`}
        ref={virtualizer.measureElement}>
        {STATUS_VIEW[item.status].label}
        <span>{item.count}{hasMore ? "+" : ""}</span>
      </h3>
    ) : (
      <div key={virtual.key} className="task-slot" data-index={virtual.index}
        data-task-id={item.task.id} ref={virtualizer.measureElement}>
        {renderTask(item.task)}
      </div>
    ));
  }
  const tail = Math.max(0, total + scrollMargin - previousEnd);

  return (
    <div className="task-list" ref={listRef}>
      {sections.map(({ status, children }) => (
        <section key={status} className="task-group" aria-label={`${STATUS_VIEW[status].label} tasks`}>
          {children}
        </section>
      ))}
      {tail > 0 && <div aria-hidden="true" style={{ height: tail }} />}
    </div>
  );
}
