import {
  ArrowDown, ArrowUp, CheckCircle2, ChevronsDown, ChevronsUp,
  Circle, LoaderCircle, Minus, Signal, SignalHigh, SignalLow, SignalMedium, SignalZero,
  SquareDashed, X, type LucideIcon,
} from "lucide-react";
import type { CheckAction, Difficulty, Priority, StatusChoice, Task, WorkStatus } from "./task-api";

export const STATUS_ORDER: WorkStatus[] = ["in_progress", "ready", "completed"];
/** Tasks in the order the list shows them: grouped In Progress, Ready, Completed, each group in page order. */
export function shownOrder<T extends { status: WorkStatus }>(tasks: readonly T[]): T[] {
  return STATUS_ORDER.flatMap((status) => tasks.filter((task) => task.status === status));
}
export const STATUS_VIEW: Record<WorkStatus, { label: string; icon: LucideIcon }> = {
  in_progress: { label: "In Progress", icon: LoaderCircle },
  ready: { label: "Ready", icon: Circle },
  completed: { label: "Completed", icon: CheckCircle2 },
};
export const PRIORITY_VIEW: Record<Priority, { label: string; icon: LucideIcon }> = {
  very_low: { label: "Very low", icon: ChevronsDown },
  low: { label: "Low", icon: ArrowDown },
  medium: { label: "Medium", icon: Minus },
  high: { label: "High", icon: ArrowUp },
  very_high: { label: "Very high", icon: ChevronsUp },
};
export const PRIORITY_ORDER: Priority[] = ["very_low", "low", "medium", "high", "very_high"];
/** Signal bars, not arrows, so a Difficulty is never read as a Priority. */
export const DIFFICULTY_VIEW: Record<Difficulty, { label: string; icon: LucideIcon }> = {
  very_easy: { label: "Very easy", icon: SignalZero },
  easy: { label: "Easy", icon: SignalLow },
  medium: { label: "Medium", icon: SignalMedium },
  hard: { label: "Hard", icon: SignalHigh },
  very_hard: { label: "Very hard", icon: Signal },
};
export const DIFFICULTY_ORDER: Difficulty[] = ["very_easy", "easy", "medium", "hard", "very_hard"];
/** No Difficulty: an empty marker, so it never reads as Very easy's single bar. */
export const NO_DIFFICULTY_ICON: LucideIcon = SquareDashed;
/** Won't Do is a kind of Completed (ADR 0008), offered beside the three work statuses. */
export const WONT_DO_LABEL = "Won't do";
/** A cross in the neutral ring, never the Completed tick. */
export const WONT_DO_ICON: LucideIcon = X;
/** Every status control's choices, in order: the three work statuses, then Won't Do. */
export const STATUS_CHOICES: StatusChoice[] = [...STATUS_ORDER, "wont_do"];
export function statusChoiceLabel(choice: StatusChoice): string {
  return choice === "wont_do" ? WONT_DO_LABEL : STATUS_VIEW[choice].label;
}
/** Why a finished Task's Won't Do choice is closed (ADR 0008). */
const FINISHED_CANNOT_BE_SET_ASIDE = "Reopen it first: a finished Task can't be set aside";
/**
 * Whether a status choice is open for a Task now, and why not when the reason is
 * worth saying. Ready is for work not yet started, which a Task completed or set
 * aside before it started still is (Q48, ADR 0008); a finished Task is reopened
 * before it is set aside. The core refuses both too; the status control and the
 * Task menu both ask here, so they never offer different choices.
 */
export function statusChoiceState(
  task: Pick<Task, "status" | "wontDo" | "completedFrom" | "wontDoFrom">, choice: StatusChoice,
): { disabled: boolean; reason?: string } {
  if (choice === "ready" && task.status !== "ready" && (task.completedFrom ?? task.wontDoFrom) !== "ready") {
    return { disabled: true };
  }
  if (choice === "wont_do" && task.status === "completed" && !task.wontDo) {
    return { disabled: true, reason: FINISHED_CANNOT_BE_SET_ASIDE };
  }
  return { disabled: false };
}
/**
 * After a check click, the checks wait this long: the second click of a double
 * click, or one landing on the row that slid under the pointer, checks nothing.
 */
export const CHECK_PAUSE_MS = 400;
/** The check's accessible name: what a click on it does, to which Task (Ticket 16). */
export function checkActionLabel(action: CheckAction, title: string): string {
  return action === "clear_wont_do" ? `Clear ${WONT_DO_LABEL} for “${title}”`
    : action === "reopen" ? `Reopen “${title}”`
    : `Complete “${title}”`;
}
