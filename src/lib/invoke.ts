import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { errorMessage } from "./error-message";

/** How the core starts its refusal after a restore made elsewhere (`TaskError::DatabaseRestored`). */
const DATA_RESTORED = "Tasknotes data was restored from a backup";

/** Whether the core refused because the data was restored since this window loaded it. */
export function isDataRestored(error: unknown): boolean {
  return errorMessage(error).startsWith(DATA_RESTORED);
}

let reloading = false;

/**
 * Every call to the core goes through here. When another window or the
 * `tasknotes` command restored a backup, this window's picture of the data is
 * stale: it accepts the restored data and reloads, so no stale revision is
 * ever written over it. The refusal still reaches the caller.
 */
export async function invoke<T>(command: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  try {
    // Options (raw-byte headers) only when given, so every other call reads as before.
    return await (options ? tauriInvoke<T>(command, args, options) : tauriInvoke<T>(command, args));
  } catch (error) {
    if (isDataRestored(error) && !reloading) {
      reloading = true;
      void reloadAfterRestore();
    }
    throw error;
  }
}

async function reloadAfterRestore() {
  try {
    await tauriInvoke("adopt_restored_data");
  } finally {
    reloading = false;
    window.location.reload();
  }
}
