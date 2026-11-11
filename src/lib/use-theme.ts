import { useEffect, useState } from "react";
import { useStoredChoice } from "./stored-choice";

export type Theme = "dark" | "light";
/** What the person chose: one appearance, or whatever the system shows. */
export type ThemePreference = Theme | "system";

/** The appearance choices, in the order Settings and the palette offer them. */
export const APPEARANCES: readonly ThemePreference[] = ["dark", "light", "system"];

export const APPEARANCE_LABELS: Record<ThemePreference, string> = {
  dark: "Dark", light: "Light", system: "Match system",
};

const STORAGE_KEY = "tasknotes.theme";
const SYSTEM_DARK = "(prefers-color-scheme: dark)";

function systemTheme(): Theme {
  // Without matchMedia (very old WebViews, tests) there is no system signal: stay dark.
  if (typeof window.matchMedia !== "function") return "dark";
  return window.matchMedia(SYSTEM_DARK).matches ? "dark" : "light";
}

export interface ThemeControl {
  /** The appearance on screen now. */
  theme: Theme;
  preference: ThemePreference;
  setPreference: (next: ThemePreference) => void;
  /** Switches to the other appearance explicitly. */
  toggle: () => void;
}

export function useTheme(): ThemeControl {
  // Without storage the app simply opens dark.
  const [preference, setPreference] = useStoredChoice<ThemePreference>(STORAGE_KEY, APPEARANCES, "dark");
  const [system, setSystem] = useState<Theme>(systemTheme);
  const theme = preference === "system" ? system : preference;

  useEffect(() => {
    if (preference !== "system" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia(SYSTEM_DARK);
    const follow = () => setSystem(query.matches ? "dark" : "light");
    follow();
    query.addEventListener("change", follow);
    return () => query.removeEventListener("change", follow);
  }, [preference]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  return {
    theme, preference, setPreference,
    toggle: () => setPreference(theme === "dark" ? "light" : "dark"),
  };
}
