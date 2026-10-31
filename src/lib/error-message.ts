/** The text to show for a failed Tauri call; the core sends its errors as strings. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong. Please try again.";
}

/** How the core starts its refusal for a folder hidden by Removed (`TaskError::NoteFolderRemoved`). */
const FOLDER_REMOVED = "This folder, or a folder it is in, is in Removed";

/** Whether the core refused because a folder is in Removed or below one. */
export function isFolderRemoved(error: unknown): boolean {
  return errorMessage(error).startsWith(FOLDER_REMOVED);
}
