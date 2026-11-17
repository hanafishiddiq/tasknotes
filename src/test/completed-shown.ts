import { COMPLETED_SHOWN_KEY } from "../lib/use-completed-shown";

/**
 * Hide Completed (batch 4) hides a view's Completed Tasks unless the person
 * showed them there. Tests written for other features, which look at Completed
 * rows, open their views with them shown, as a person who chose so would.
 */
export function showCompletedIn(...views: string[]) {
  const keys = views.length > 0 ? views : ["all", "inbox"];
  window.localStorage.setItem(COMPLETED_SHOWN_KEY, JSON.stringify(Object.fromEntries(keys.map((key) => [key, true]))));
}
