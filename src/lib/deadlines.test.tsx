import { describe, expect, it } from "vitest";
import { localCalendarDay, deadlineStatus } from "./deadlines";

describe("calendar-only deadlines", () => {
  it("uses the specified local timezone instead of UTC midnight", () => {
    const time = new Date("2026-10-13T00:30:00Z");
    expect(localCalendarDay(time, "America/Los_Angeles")).toBe("2026-10-12");
    expect(localCalendarDay(time, "Asia/Jakarta")).toBe("2026-10-13");
  });
  it("counts only unfinished dates strictly earlier than today as overdue", () => {
    expect(deadlineStatus("2026-10-12", "2026-10-13", "ready")).toBe("overdue");
    expect(deadlineStatus("2026-10-12", "2026-10-13", "in_progress")).toBe("overdue");
    expect(deadlineStatus("2026-10-12", "2026-10-13", "completed")).toBeNull();
    expect(deadlineStatus("2026-10-13", "2026-10-13", "ready")).toBe("today");
    expect(deadlineStatus("2026-10-14", "2026-10-13", "ready")).toBe("upcoming");
    expect(deadlineStatus(null, "2026-10-13", "ready")).toBeNull();
  });
});
