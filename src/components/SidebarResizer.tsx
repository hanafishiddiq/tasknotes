import { useEffect, useRef, type KeyboardEvent, type MouseEvent } from "react";
import { SIDEBAR_DEFAULT, SIDEBAR_MAX, SIDEBAR_MIN } from "../lib/use-sidebar-width";

const KEY_STEP = 10;

interface SidebarResizerProps {
  width: number;
  onResize: (width: number) => void;
}

/**
 * The sidebar's right edge: drag it, or focus it and use the arrow keys,
 * Home and End. A double-click, or Return, puts the default width back. Mouse events,
 * as the sidebar's row drag uses, so neither steals the other's press.
 */
export default function SidebarResizer({ width, onResize }: SidebarResizerProps) {
  const stop = useRef<(() => void) | null>(null);
  useEffect(() => () => stop.current?.(), []);

  function start(event: MouseEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = width;
    const move = (moved: globalThis.MouseEvent) => onResize(startWidth + moved.clientX - startX);
    const end = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", end);
      document.body.classList.remove("sidebar-resizing");
      stop.current = null;
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", end);
    document.body.classList.add("sidebar-resizing");
    stop.current = end;
  }

  function keys(event: KeyboardEvent) {
    const next = {
      ArrowLeft: width - KEY_STEP, ArrowRight: width + KEY_STEP, Home: SIDEBAR_MIN, End: SIDEBAR_MAX,
      Enter: SIDEBAR_DEFAULT,
    }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    onResize(next);
  }

  return (
    <div className="sidebar-resizer" role="separator" tabIndex={0}
      aria-label="Resize sidebar" aria-orientation="vertical"
      aria-valuemin={SIDEBAR_MIN} aria-valuemax={SIDEBAR_MAX} aria-valuenow={width}
      onMouseDown={start} onKeyDown={keys} onDoubleClick={() => onResize(SIDEBAR_DEFAULT)} />
  );
}
