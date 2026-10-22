import type { WorkStatus } from "./task-api";

export type DeadlineStatus = "overdue" | "today" | "upcoming";

/**
 * Date-only deadlines follow the local calendar day, never UTC midnight.
 * The optional IANA timezone is for reproducible DST/timezone tests.
 */
export function localCalendarDay(now = new Date(), timeZone?: string): string {
  const formatter = new Intl.DateTimeFormat("en-US", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
  const parts = formatter.formatToParts(now);
  const field = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${field("year")}-${field("month")}-${field("day")}`;
}

export function deadlineStatus(
  dueDate: string | null | undefined,
  today: string,
  status: WorkStatus,
): DeadlineStatus | null {
  if (!dueDate || status === "completed") return null;
  if (dueDate < today) return "overdue";
  if (dueDate === today) return "today";
  return "upcoming";
}
