import {
  ArrowDown, ArrowUp, CheckCircle2, ChevronsDown, ChevronsUp,
  Circle, LoaderCircle, Minus, Signal, SignalHigh, SignalLow, SignalMedium, SignalZero,
  SquareDashed, type LucideIcon,
} from "lucide-react";
import type { Difficulty, Priority, WorkStatus } from "./task-api";

export const STATUS_ORDER: WorkStatus[] = ["in_progress", "ready", "completed"];
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
