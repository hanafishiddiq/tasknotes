/** A Task row and where it sat in the window, so a change to the list can leave it there. */
export interface ScrollAnchor {
  taskId: string;
  top: number;
}

/**
 * The open Task, or else the first row in view. WebKit has no scroll anchoring
 * of its own. At the very top nothing holds, so rows added above come into view.
 */
export function captureScrollAnchor(area: HTMLElement | null, openId: string | null): ScrollAnchor | null {
  if (!area) return null;
  const rows = [...area.querySelectorAll<HTMLElement>("[data-task-id]")];
  const areaTop = area.getBoundingClientRect().top;
  const row = rows.find((candidate) => candidate.dataset.taskId === openId) ??
    (area.scrollTop > 0 ? rows.find((candidate) => candidate.getBoundingClientRect().bottom > areaTop) : undefined);
  return row ? { taskId: row.dataset.taskId!, top: row.getBoundingClientRect().top } : null;
}

/** Scrolls by however far the row moved, if it is still on the page. */
export function holdScrollAnchor(area: HTMLElement, anchor: ScrollAnchor) {
  const row = area.querySelector<HTMLElement>(`[data-task-id="${CSS.escape(anchor.taskId)}"]`);
  if (!row) return;
  const moved = row.getBoundingClientRect().top - anchor.top;
  if (moved !== 0) area.scrollTop += moved;
}
