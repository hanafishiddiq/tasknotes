import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Sent by the desktop shell when the global Quick Capture shortcut (or the menu-bar item) asks for a new Task. */
export const QUICK_CAPTURE_REQUESTED = "tasknotes://quick-capture";

/** Why the shell would not use a key combination; Settings words each one. */
export type ShortcutProblem =
  | { kind: "invalid" }
  | { kind: "needsTwoModifiers" }
  | { kind: "unsupportedKey" }
  | { kind: "reservedBySystem"; usedFor: string };

/** `current` is the shortcut still working after a change was refused. */
export type QuickCaptureStatus =
  | { state: "off" }
  | { state: "active"; label: string }
  | { state: "unavailable"; label: string; current: string | null }
  | { state: "rejected"; problem: ShortcutProblem; current: string | null };

/** The desktop shell's settings, kept by the shell beside the database. */
export interface ShellView {
  keepInMenuBar: boolean;
  /** Menu-bar mode is on but this system could not show the item; closing still quits. */
  menuBarUnavailable: boolean;
  /** The saved choice; `quickCapture` says whether it is working. */
  quickCaptureEnabled: boolean;
  quickCapture: QuickCaptureStatus;
  /** The recorded shortcut, also while Quick Capture is off. */
  rememberedLabel: string | null;
  settingsUnreadable: boolean;
}

export const shellApi = {
  get: () => invoke<ShellView>("get_shell_settings"),
  setKeepInMenuBar: (enabled: boolean) => invoke<ShellView>("set_keep_in_menu_bar", { enabled }),
  setQuickCapture: (enabled: boolean, shortcut: string | null = null) =>
    invoke<ShellView>("set_quick_capture", { input: { enabled, shortcut } }),
};

export function listenForQuickCapture(onCapture: () => void): Promise<UnlistenFn> {
  return listen(QUICK_CAPTURE_REQUESTED, () => onCapture());
}

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "OS", "Hyper", "Super", "CapsLock", "Fn"]);

/**
 * The accelerator for a key press while recording, e.g. `Control+Alt+Space`,
 * or null while only modifiers are held. Uses the physical key (`code`) so the
 * keyboard layout does not change what is registered; the shell decides
 * whether the combination is acceptable.
 */
export function acceleratorFromKey(event: Pick<KeyboardEvent,
  "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  if (MODIFIER_KEYS.has(event.key) || !event.code) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Control");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  parts.push(event.code);
  return parts.join("+");
}
