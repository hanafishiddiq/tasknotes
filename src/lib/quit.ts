import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Sent by the desktop shell on Cmd+Q or when the window is closed. */
export const QUIT_REQUESTED = "tasknotes://quit-requested";

/** Work that must reach the core before the app quits; each resolves false to keep the app open. */
const savers = new Set<() => Promise<boolean>>();

export function saveBeforeQuit(save: () => Promise<boolean>): () => void {
  savers.add(save);
  return () => {
    savers.delete(save);
  };
}

/**
 * Answers each quit request once every saver has finished. When one could not
 * save, the app stays open and `onHeldBack` brings that work into view.
 */
export function listenForQuit(onHeldBack: () => void): Promise<UnlistenFn> {
  return listen(QUIT_REQUESTED, async () => {
    let proceed = true;
    for (const save of savers) proceed = (await save()) && proceed;
    if (!proceed) onHeldBack();
    await invoke("quit_ready", { proceed });
  });
}
