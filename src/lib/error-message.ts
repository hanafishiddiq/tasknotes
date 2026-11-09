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

/** How the core refuses to list the Tasks of a List in Removed (`TaskError::ListRemoved`). */
const LIST_REMOVED = "List has been removed";

/** Whether the core refused because the List asked for is in Removed. */
export function isListRemoved(error: unknown): boolean {
  return errorMessage(error).startsWith(LIST_REMOVED);
}
