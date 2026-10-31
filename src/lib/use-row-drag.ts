import { useCallback, useEffect, useRef, useState, type MouseEvent, type RefObject } from "react";

/**
 * Where a dragged sidebar row would land. "end" is below every row: after the
 * last row of the outermost level.
 */
export type RowDrop = { id: string; edge: "before" | "after" | "into" } | { id: null; edge: "end" };

interface RowDragOptions {
  /** Holds the rows; each is marked `data-drag-row="<id>"`. */
  container: RefObject<HTMLElement | null>;
  /** A row the dragged one may not land on (itself, its own subfolders). */
  canTarget: (dragged: string, id: string) => boolean;
  /** Whether a row's middle means "inside it" rather than before or after. */
  canNest: boolean;
  onDrop: (dragged: string, drop: RowDrop) => void;
}

/** How far the mouse travels before a press on a row becomes a drag, not a click. */
const DRAG_DISTANCE = 4;
const SCROLL_EDGE = 28;
const SCROLL_STEP = 6;

function dropAt(
  container: HTMLElement,
  x: number,
  y: number,
  dragged: string,
  { canTarget, canNest }: Pick<RowDragOptions, "canTarget" | "canNest">,
): RowDrop | null {
  const rows = [...container.querySelectorAll<HTMLElement>("[data-drag-row]")];
  const area = container.getBoundingClientRect();
  if (x < area.left || x > area.right) return null;
  for (const row of rows) {
    const box = row.getBoundingClientRect();
    if (y >= box.bottom) continue;
    const id = row.dataset.dragRow!;
    if (!canTarget(dragged, id)) return null;
    const along = (y - box.top) / (box.height || 1);
    if (!canNest) return { id, edge: along < 0.5 ? "before" : "after" };
    if (along < 0.25) return { id, edge: "before" };
    return { id, edge: along > 0.75 ? "after" : "into" };
  }
  return rows.length > 0 ? { id: null, edge: "end" } : null;
}

/**
 * Press, move and release on a sidebar row. Mouse events rather than HTML drag
 * and drop: the desktop window gives file drops to the shell, and these rows
 * never leave it. Escape puts the row back.
 */
export function useRowDrag(options: RowDragOptions) {
  const [dragging, setDragging] = useState<string | null>(null);
  const [drop, setDrop] = useState<RowDrop | null>(null);
  const latest = useRef(options);
  latest.current = options;
  const stop = useRef<(() => void) | null>(null);

  useEffect(() => () => stop.current?.(), []);

  const start = useCallback((event: MouseEvent, id: string, label: string) => {
    if (event.button !== 0) return;
    const startX = event.clientX;
    const startY = event.clientY;
    let lastX = startX;
    let lastY = startY;
    let active = false;
    let current: RowDrop | null = null;
    let ghost: HTMLElement | null = null;
    let scroller: number | undefined;

    const aim = () => {
      const { container } = latest.current;
      if (!container.current) return;
      const next = dropAt(container.current, lastX, lastY, id, latest.current);
      if (JSON.stringify(next) !== JSON.stringify(current)) {
        current = next;
        setDrop(next);
      }
    };

    const begin = () => {
      active = true;
      setDragging(id);
      document.body.classList.add("row-dragging");
      ghost = document.createElement("div");
      ghost.className = "row-drag-ghost";
      ghost.setAttribute("aria-hidden", "true");
      ghost.textContent = label;
      document.body.append(ghost);
      // Holding near the sidebar's edge keeps scrolling it.
      scroller = window.setInterval(() => {
        const sidebar = latest.current.container.current?.closest<HTMLElement>(".sidebar");
        if (!sidebar) return;
        const box = sidebar.getBoundingClientRect();
        if (lastY < box.top + SCROLL_EDGE) sidebar.scrollTop -= SCROLL_STEP;
        else if (lastY > box.bottom - SCROLL_EDGE) sidebar.scrollTop += SCROLL_STEP;
        else return;
        aim();
      }, 16);
    };

    const onMove = (move: globalThis.MouseEvent) => {
      lastX = move.clientX;
      lastY = move.clientY;
      if (!active) {
        if (Math.hypot(move.clientX - startX, move.clientY - startY) < DRAG_DISTANCE) return;
        begin();
      }
      if (ghost) ghost.style.transform = `translate(${move.clientX + 10}px, ${move.clientY + 6}px)`;
      aim();
    };

    const finish = () => {
      window.removeEventListener("mousemove", onMove, true);
      window.removeEventListener("mouseup", onUp, true);
      window.removeEventListener("keydown", onKey, true);
      window.clearInterval(scroller);
      ghost?.remove();
      document.body.classList.remove("row-dragging");
      setDragging(null);
      setDrop(null);
      stop.current = null;
    };

    function onUp() {
      const dropped = active ? current : null;
      if (active) {
        // The release would also click the row that was pressed.
        const swallow = (click: Event) => click.stopPropagation();
        window.addEventListener("click", swallow, { capture: true, once: true });
        window.setTimeout(() => window.removeEventListener("click", swallow, true));
      }
      finish();
      if (dropped) latest.current.onDrop(id, dropped);
    }

    function onKey(key: KeyboardEvent) {
      if (key.key !== "Escape" || !active) return;
      key.preventDefault();
      key.stopPropagation();
      finish();
    }

    stop.current = finish;
    window.addEventListener("mousemove", onMove, true);
    window.addEventListener("mouseup", onUp, true);
    window.addEventListener("keydown", onKey, true);
  }, []);

  return { dragging, drop, start };
}

/**
 * The sibling a row should sit directly after to land before or after
 * `targetId`, or null for first. The core takes this anchor, so pages the
 * sidebar has not loaded do not matter.
 */
export function anchorFor(siblings: string[], dragged: string, targetId: string, edge: "before" | "after"): string | null {
  const others = siblings.filter((id) => id !== dragged);
  const at = others.indexOf(targetId);
  if (edge === "after") return targetId;
  return at > 0 ? others[at - 1] : null;
}

/** Whether the row already sits directly after `anchor`. */
export function alreadyThere(siblings: string[], dragged: string, anchor: string | null): boolean {
  const at = siblings.indexOf(dragged);
  return at >= 0 && (at === 0 ? null : siblings[at - 1]) === anchor;
}
