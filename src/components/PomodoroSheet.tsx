import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

/** Space kept between a sheet and the window's edges, and under what opened it. */
const EDGE = 8;
const GAP = 6;

interface PomodoroSheetProps {
  /** What opened it: the sheet sits under it, centred or flush with its right edge. */
  anchor: HTMLElement;
  align: "center" | "end";
  label: string;
  className: string;
  /** Closes it; `refocus` gives focus back to what opened it. */
  onClose: (refocus: boolean) => void;
  children: ReactNode;
}

/**
 * A sheet in the Task menu's look (batch 3, Q28) that the Pomodoro page opens
 * under a control: on the sidebar gray with a hairline edge, no shadow and no
 * backdrop, kept inside the window. A click elsewhere, the window losing
 * focus or Escape closes it; Escape gives focus back to what opened it.
 */
export default function PomodoroSheet({ anchor, align, label, className, onClose, children }: PomodoroSheetProps) {
  const sheet = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);
  const latestClose = useRef(onClose);
  latestClose.current = onClose;

  useLayoutEffect(() => {
    const element = sheet.current;
    if (!element) return;
    const box = anchor.getBoundingClientRect();
    const width = element.offsetWidth;
    const wanted = align === "center" ? box.left + box.width / 2 - width / 2 : box.right - width;
    setPlace({
      left: Math.max(EDGE, Math.min(wanted, window.innerWidth - width - EDGE)),
      top: Math.max(EDGE, Math.min(box.bottom + GAP, window.innerHeight - element.offsetHeight - EDGE)),
    });
  }, [anchor, align]);

  useEffect(() => {
    const away = (event: Event) => {
      const target = event.target;
      if (target instanceof Node && (sheet.current?.contains(target) || anchor.contains(target))) return;
      latestClose.current(false);
    };
    const leave = () => latestClose.current(false);
    document.addEventListener("pointerdown", away, true);
    window.addEventListener("blur", leave);
    return () => {
      document.removeEventListener("pointerdown", away, true);
      window.removeEventListener("blur", leave);
    };
  }, [anchor]);

  return (
    <div ref={sheet} className={`task-menu ${className}`} role="dialog" aria-label={label}
      // Off screen until placed with its real size.
      style={place ?? { left: -9999, top: -9999 }}
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
        event.preventDefault();
        event.stopPropagation();
        onClose(true);
      }}>
      {children}
    </div>
  );
}
