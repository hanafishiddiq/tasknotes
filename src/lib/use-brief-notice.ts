import { useEffect, useRef, useState } from "react";

/** How long a brief notice stays before it fades. */
export const NOTICE_MS = 2000;
/** The fade out, matching DESIGN.md's 120ms fades and the new Task toast. */
export const NOTICE_FADE_MS = 120;

export interface BriefNotice {
  /** What it says, or null when nothing shows. */
  text: string | null;
  /** Changes with every notice, so the same words are announced again. */
  seq: number;
  /** Fading out; it leaves after NOTICE_FADE_MS. */
  leaving: boolean;
  /** Shows `text`, replacing whatever showed, for NOTICE_MS. */
  show(text: string): void;
}

/**
 * A short line that says why something did not happen, such as a Task that
 * cannot move here. Nothing to click and never focused, so it gives way on
 * its own. View state only.
 */
export function useBriefNotice(): BriefNotice {
  const [shown, setShown] = useState<{ text: string; seq: number; leaving: boolean } | null>(null);
  const seq = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  function show(text: string) {
    seq.current += 1;
    clearTimeout(timer.current);
    setShown({ text, seq: seq.current, leaving: false });
    timer.current = setTimeout(() => {
      setShown((now) => now && { ...now, leaving: true });
      timer.current = setTimeout(() => setShown(null), NOTICE_FADE_MS);
    }, NOTICE_MS);
  }

  return { text: shown?.text ?? null, seq: shown?.seq ?? 0, leaving: shown?.leaving ?? false, show };
}
