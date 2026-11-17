import { describe, expect, it } from "vitest";
import { completedRowFor, visibleTasks } from "./completed-row";
import type { Task } from "./task-api";

const task = (id: string, status: Task["status"]) => ({ id, status }) as Task;
const NONE = { text: "", status: "", priority: "", difficulty: "", tag: "" } as const;

describe("which Tasks the list shows while Completed is hidden", () => {
  it("drops Completed ones, but never a Task holding unsaved text", () => {
    const tasks = [task("a", "ready"), task("b", "completed"), task("c", "completed")];
    // c was completed elsewhere while the person typed in it: their words stay on screen.
    expect(visibleTasks(tasks, true, (id) => id === "c").map((t) => t.id)).toEqual(["a", "c"]);
    expect(visibleTasks(tasks, false, () => false)).toBe(tasks);
  });
});

describe("the row under the list", () => {
  const base = { loading: false, lastPage: true, shownHere: false, count: 2, tasks: [task("a", "completed")] };
  it("offers to show what is hidden, with the count for this very query", () => {
    expect(completedRowFor({ ...base, filters: { ...NONE, hideCompleted: true } })).toBe("show");
    expect(completedRowFor({ ...base, count: null, filters: { ...NONE, hideCompleted: true } })).toBeNull();
    expect(completedRowFor({ ...base, count: 0, filters: { ...NONE, hideCompleted: true } })).toBeNull();
  });
  it("offers to hide them again where they are shown and present", () => {
    expect(completedRowFor({ ...base, shownHere: true, filters: { ...NONE, hideCompleted: false } })).toBe("hide");
    expect(completedRowFor({ ...base, shownHere: true, tasks: [], filters: { ...NONE, hideCompleted: false } })).toBeNull();
  });
  it("stays away while loading, before the last page, and under a search or any status filter", () => {
    const hidden = { ...NONE, hideCompleted: true };
    expect(completedRowFor({ ...base, loading: true, filters: hidden })).toBeNull();
    expect(completedRowFor({ ...base, lastPage: false, filters: hidden })).toBeNull();
    expect(completedRowFor({ ...base, filters: { ...hidden, text: "x" } })).toBeNull();
    for (const status of ["ready", "in_progress", "completed", "wont_do"] as const) {
      expect(completedRowFor({ ...base, filters: { ...hidden, status } }), status).toBeNull();
      expect(completedRowFor({ ...base, shownHere: true, filters: { ...NONE, status } }), status).toBeNull();
    }
  });
});
