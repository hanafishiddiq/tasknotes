import { useEffect, useState } from "react";

/**
 * A per-window display choice kept in this window's storage (appearance,
 * where the sidebar's counts show). Not domain data: storage may be missing
 * or unreadable, and then the choice is `fallback` and lives for the session.
 */
function readChoice<T extends string>(key: string, choices: readonly T[], fallback: T): T {
  try {
    const value = window.localStorage.getItem(key);
    return choices.find((choice) => choice === value) ?? fallback;
  } catch {
    return fallback;
  }
}

export function useStoredChoice<T extends string>(
  key: string, choices: readonly T[], fallback: T,
): [T, (next: T) => void] {
  const [choice, setChoice] = useState<T>(() => readChoice(key, choices, fallback));
  useEffect(() => {
    try {
      window.localStorage.setItem(key, choice);
    } catch {
      // Keep the in-memory choice for this session.
    }
  }, [key, choice]);
  return [choice, setChoice];
}
