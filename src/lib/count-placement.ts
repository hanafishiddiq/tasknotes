import { useStoredChoice } from "./stored-choice";

/** Where the sidebar shows each place's number of unfinished Tasks (batch 3, Q48). */
export type CountPlacement = "every" | "open" | "none";

/** The choices, in the order Settings offers them; every folder is the default (Q46). */
export const COUNT_PLACEMENTS: readonly CountPlacement[] = ["every", "open", "none"];

export const COUNT_PLACEMENT_LABELS: Record<CountPlacement, string> = {
  every: "Every folder", open: "Open folder only", none: "None",
};

export const useCountPlacement = () => useStoredChoice<CountPlacement>("tasknotes.sidebarCounts", COUNT_PLACEMENTS, "every");
