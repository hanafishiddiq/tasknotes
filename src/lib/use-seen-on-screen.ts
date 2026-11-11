import { useEffect, useRef, type RefObject } from "react";

/**
 * Calls `onSeen` when the element is on screen (half of it, inside every
 * scrolling area around it) while the window is in front. Watches only while
 * `onSeen` is given; it may be called more than once, so the caller keeps
 * whatever should happen only once.
 */
export function useSeenOnScreen(ref: RefObject<Element | null>, onSeen: (() => void) | undefined) {
  const latest = useRef(onSeen);
  latest.current = onSeen;
  const watching = Boolean(onSeen);

  useEffect(() => {
    const element = ref.current;
    if (!watching || !element || typeof IntersectionObserver === "undefined") return;
    let onScreen = false;
    const report = () => {
      if (onScreen && document.visibilityState === "visible" && document.hasFocus()) latest.current?.();
    };
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) if (entry.target === element) onScreen = entry.isIntersecting;
      report();
    }, { threshold: 0.5 });
    observer.observe(element);
    window.addEventListener("focus", report);
    document.addEventListener("visibilitychange", report);
    return () => {
      observer.disconnect();
      window.removeEventListener("focus", report);
      document.removeEventListener("visibilitychange", report);
    };
  }, [ref, watching]);
}
