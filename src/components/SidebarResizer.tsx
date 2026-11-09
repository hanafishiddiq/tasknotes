import { useEffect, useRef, type KeyboardEvent, type MouseEvent } from "react";
import { SIDEBAR_DEFAULT, SIDEBAR_MIN } from "../lib/use-sidebar-width";

const KEY_STEP = 10;

interface SidebarResizerProps {
  width: number;
  max: number;
  /** Shows a width during a drag without rendering the app; returns the width it kept. */
  onPreview: (width: number) => number;
  /** Commits a width: once per drag, keyboard step or reset. */
  onResize: (width: number) => void;
}

/**
 * The sidebar's right edge: drag it, or focus it and use the arrow keys,
 * Home and End. A double-click, or Return, puts the default width back. Mouse events,
 * as the sidebar's row drag uses, so neither steals the other's press.
 *
 * A drag only moves the shell's `--sidebar-width` (and this edge's value) until
 * the button is released, so the task list does not re-render on every move.
 * Releasing, or the window losing focus, commits the width; Escape puts the
 * starting width back.
 */
export default function SidebarResizer({ width, max, onPreview, onResize }: SidebarResizerProps) {
  const edge = useRef<HTMLDivElement>(null);
  const stop = useRef<(() => void) | null>(null);
  useEffect(() => () => stop.current?.(), []);

  function start(event: MouseEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = width;
    let current = startWidth;
    const show = (next: number) => {
      current = onPreview(next);
      edge.current?.setAttribute("aria-valuenow", String(current));
    };
    const move = (moved: globalThis.MouseEvent) => show(startWidth + moved.clientX - startX);
    const finish = (commit: boolean) => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", release);
      window.removeEventListener("blur", release);
      window.removeEventListener("keydown", escape, true);
      document.body.classList.remove("sidebar-resizing");
      stop.current = null;
      if (!commit) show(startWidth);
      else if (current !== startWidth) onResize(current);
    };
    const release = () => finish(true);
    const escape = (key: globalThis.KeyboardEvent) => {
      if (key.key !== "Escape") return;
      key.preventDefault();
      key.stopPropagation();
      finish(false);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", release);
    window.addEventListener("blur", release);
    window.addEventListener("keydown", escape, true);
    document.body.classList.add("sidebar-resizing");
    stop.current = () => finish(false);
  }

  function keys(event: KeyboardEvent) {
    const next = {
      ArrowLeft: width - KEY_STEP, ArrowRight: width + KEY_STEP, Home: SIDEBAR_MIN, End: max,
      Enter: SIDEBAR_DEFAULT,
    }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    onResize(next);
  }

  return (
    <div ref={edge} className="sidebar-resizer" role="separator" tabIndex={0}
      aria-label="Resize sidebar" aria-orientation="vertical"
      aria-valuemin={SIDEBAR_MIN} aria-valuemax={max} aria-valuenow={width}
      onMouseDown={start} onKeyDown={keys} onDoubleClick={() => onResize(SIDEBAR_DEFAULT)} />
  );
}
