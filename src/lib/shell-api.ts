import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Sent by the desktop shell when the global Quick Capture shortcut (or the menu-bar item) asks for a new Task. */
export const QUICK_CAPTURE_REQUESTED = "tasknotes://quick-capture";

/**
 * Whether the global shortcut works. `reason` is the shell's own wording for
 * this system; `current` is the shortcut still working after a refused change.
 */
export type QuickCaptureStatus =
  | { state: "off" }
  | { state: "active"; label: string }
  | { state: "unavailable"; label: string; reason: string; current: string | null }
  | { state: "rejected"; reason: string; current: string | null };

/** The desktop shell's settings, kept by the shell beside the database. */
export interface ShellView {
  keepInMenuBar: boolean;
  /** Where the tray item lives on this system, e.g. "menu bar". */
  trayPlace: string;
  /** What closing the window does right now, in the shell's words. */
  closeHint: string;
  /** The saved choice; `quickCapture` says whether it is working. */
  quickCaptureEnabled: boolean;
  quickCapture: QuickCaptureStatus;
  /** The recorded shortcut, also while Quick Capture is off. */
  rememberedLabel: string | null;
  /** The settings file could not be read at launch, so the defaults are in use. */
  settingsUnreadable: boolean;
}

export const shellApi = {
  get: () => invoke<ShellView>("get_shell_settings"),
  setKeepInMenuBar: (enabled: boolean) => invoke<ShellView>("set_keep_in_menu_bar", { enabled }),
  setQuickCapture: (enabled: boolean, shortcut: string | null = null) =>
    invoke<ShellView>("set_quick_capture", { input: { enabled, shortcut } }),
  /** Whether Quick Capture was asked for since the page last looked (e.g. while it was loading). */
  takeQuickCapture: () => invoke<boolean>("take_quick_capture"),
  /** Why the saved shortcut could not be used at launch; given out once. */
  takeLaunchNotice: () => invoke<string | null>("take_launch_notice"),
};

/**
 * Calls `onCapture` for each Quick Capture the shell asks for, including one
 * asked for before the page was listening. The shell keeps the request until
 * it is collected, so neither a press during loading nor a doubled event
 * focuses twice. Where the shell has no such commands nothing happens.
 */
export async function listenForQuickCapture(onCapture: () => void): Promise<UnlistenFn> {
  const collect = () => shellApi.takeQuickCapture()
    .then((pending) => { if (pending) onCapture(); })
    .catch(() => {});
  const unlisten = await listen(QUICK_CAPTURE_REQUESTED, () => collect());
  await collect();
  return unlisten;
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
