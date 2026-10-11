import {
  ArrowDown, ArrowUp, CheckCircle2, ChevronsDown, ChevronsUp,
  Circle, LoaderCircle, Minus, type LucideIcon,
} from "lucide-react";
import type { Priority, WorkStatus } from "./task-api";

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
