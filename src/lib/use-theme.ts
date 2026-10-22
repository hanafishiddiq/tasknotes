import { useEffect, useState } from "react";

export type Theme = "dark" | "light";

const STORAGE_KEY = "tasknotes.theme";

// A per-window display preference, not domain data: storage may be unavailable,
// in which case the app simply opens dark.
function storedTheme(): Theme {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export function useTheme(): [Theme, () => void] {
  const [theme, setTheme] = useState<Theme>(storedTheme);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      window.localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // Keep the in-memory choice for this session.
    }
  }, [theme]);

  return [theme, () => setTheme((current) => (current === "dark" ? "light" : "dark"))];
}
