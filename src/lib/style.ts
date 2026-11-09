// The core's own allow-list (one file, read by Rust and by the window), so the
// picker offers exactly what the core accepts. Validation stays in the core.
import options from "../../crates/tasknotes-core/src/style.json";

/** The icon and colour a List or Note Folder may carry; null is the default look. */
export interface Style {
  icon: string | null;
  color: string | null;
}

export const STYLE_COLORS: readonly string[] = options.colors;
export const STYLE_ICON_NAMES: readonly string[] = options.icons;

/** "chef-hat" → "Chef hat". */
export function styleLabel(name: string): string {
  const words = name.replace(/-/g, " ");
  return words[0].toUpperCase() + words.slice(1);
}

/** The colour the current theme gives a key (App.css `--style-<key>`), or none for the default. */
export function styleColor(color: string | null | undefined): string | undefined {
  return color && STYLE_COLORS.includes(color) ? `var(--style-${color})` : undefined;
}

/** Icons where every word of the search starts a word of the name ("lock" finds lock, not clock), in picker order. */
export function searchIcons(text: string): string[] {
  const words = text.toLowerCase().split(/[\s-]+/).filter(Boolean);
  return STYLE_ICON_NAMES.filter((name) => {
    const parts = name.split("-");
    return words.every((word) => parts.some((part) => part.startsWith(word)));
  });
}
