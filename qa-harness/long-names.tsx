// QA page: the real App against the in-memory World, with very long names everywhere.
// Run `pnpm exec vite --config qa-harness/vite.config.ts`, open
// http://localhost:1440/long-names.html, walk the views and call `scanOverflow()`
// in the console: it lists text that spills, is cut without "…", or covers other text.
import { World, fixtureTask, fixtureList, FIXTURE_NOW } from "../src/test/a11y-world";

const LONG = "Synthetic folder name that goes on far longer than any column is wide";
const WORD = "Syntheticnamewithoutasinglespaceanywhereinitatalljusttobesuresure";
const world = new World();
world.lists = [fixtureList("l1", LONG), fixtureList("l2", WORD)];
world.tasks = [
  fixtureTask("a", { title: `${LONG} — Maintenance and review of every single step`, listId: "l1", status: "in_progress", progress: 40, nextStep: LONG, tags: ["alpha-with-a-long-tag-name", "beta"], dueDate: "2026-12-01" }),
  fixtureTask("b", { title: WORD + WORD, listId: "l1" }),
  fixtureTask("c", { title: "Short", listId: "l2", status: "completed", progress: 100 }),
];
const parent = world.notes.folder("Synthetic parent " + LONG);
const child = world.notes.folder(LONG, parent);
const child2 = world.notes.folder(WORD, parent);
world.notes.seed(`# ${LONG} ${LONG}\n\nBody`, `${LONG} ${LONG}`, { folderId: child.id });
world.notes.seed(`# ${WORD}\n\nBody`, WORD, { folderId: child2.id });
// A plain first line: the editor shows it as the title (batch 4).
world.notes.seed("Synthetic meeting notes on a plain first line\n\nBody text under the title.\n\n- First point\n- Second point", "Synthetic meeting notes on a plain first line");
world.tasksOfNote = [{ id: "a", title: `${LONG} — Maintenance and review of every single step`, status: "completed", revision: 1, removed: false } as any];
world.notesOfTask = [{ id: "note-1", title: `${LONG} ${LONG}`, revision: 1, removed: false } as any];
world.removed = [{ kind: "task", id: "r", title: WORD + " " + LONG, revision: 1, removedAt: FIXTURE_NOW, inRemovedContainer: false }];
const T = `${LONG} — Maintenance and review of every single step`;
world.pomodoroDays = [{ date: "2026-10-26", finishedSessions: 1, focusMinutes: 25, stoppedSessions: 0, sessions: [
  { id: "s1", startedAt: FIXTURE_NOW - 3_600_000, endedAt: FIXTURE_NOW - 2_000_000, utcOffsetSeconds: 25_200, outcome: "finished", plannedMinutes: 25, overtimeMinutes: 0, stoppedAfterMinutes: null, taskId: "a", taskTitle: T, taskRemoved: false }] }] as any;
world.pomodoroView = { title: T, plannedSeconds: 1500, finishedInCycle: 0, focusStartedAt: null, lastTaskId: "a", silent: false, saveError: null,
  durations: { focusMinutes: 25, shortBreakMinutes: 5, longBreakMinutes: 15, sessionsBeforeLongBreak: 4 },
  limits: { focusMinutes: { min: 1, max: 180 }, shortBreakMinutes: { min: 1, max: 60 }, longBreakMinutes: { min: 1, max: 60 }, sessionsBeforeLongBreak: { min: 1, max: 12 } },
  phase: "focus", taskId: "a", remainingSeconds: 800, paused: true } as any;
world.notificationPermission = new URLSearchParams(location.search).get("notifications") as any ?? "allowed";
// A checklist on the first Task: one long item, one done (batch 4).
world.subtasks = [
  { id: "sub-a", taskId: "a", text: "Book the synthetic train", done: true, removed: false, revision: 1, createdAt: FIXTURE_NOW, updatedAt: FIXTURE_NOW },
  { id: "sub-b", taskId: "a", text: `${LONG}, and then a second sentence that keeps going so the item wraps onto more lines`, done: false, removed: false, revision: 1, createdAt: FIXTURE_NOW, updatedAt: FIXTURE_NOW },
  { id: "sub-c", taskId: "a", text: WORD, done: false, removed: false, revision: 1, createdAt: FIXTURE_NOW, updatedAt: FIXTURE_NOW },
];
// An image in the first Task's notes (batch 4), drawn here so no file is needed.
{
  const canvas = document.createElement("canvas");
  canvas.width = 640;
  canvas.height = 300;
  const pen = canvas.getContext("2d")!;
  const fill = pen.createLinearGradient(0, 0, 640, 300);
  fill.addColorStop(0, "#3a7bd5");
  fill.addColorStop(1, "#ffd60a");
  pen.fillStyle = fill;
  pen.fillRect(0, 0, 640, 300);
  pen.fillStyle = "#000";
  pen.font = "bold 36px -apple-system, sans-serif";
  pen.fillText("Synthetic image", 40, 160);
  const blob = await new Promise<Blob>((done) => canvas.toBlob((made) => done(made!), "image/png"));
  world.taskImages.set("image-seed", await blob.arrayBuffer());
  // The first Task is also shown in the second List (batch 4).
  world.tasks[0].alsoIn = ["l2"];
  world.tasks[0].description = "Measure the synthetic shelf first.\n\n![image](tasknotes-image:image-seed)\n\nThen cut along the line.";
}
(window as any).world = world;

let next = 1;
const callbacks = new Map<number, (v: unknown) => void>();
const listeners = new Map<string, number[]>();
(window as any).__TAURI_INTERNALS__ = {
  transformCallback(cb: (v: unknown) => void) { const id = next++; callbacks.set(id, cb); return id; },
  unregisterCallback(id: number) { callbacks.delete(id); },
  async invoke(cmd: string, args: any) {
    if (cmd === "plugin:event|listen") { listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]); return next++; }
    if (cmd.startsWith("plugin:")) return null;
    try { return world.handle(cmd, args ?? {}); }
    catch (e) { console.warn("fake core:", cmd, e); throw e; }
  },
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
};
await import("../src/main");

// Finds text that spills, is cut without an ellipsis, or sits on other text.
// Floating layers (menus, dialogs, popovers, toasts) are meant to cover the page.
const FLOATING = '[role=menu],[role=dialog],[role=listbox],.task-menu,.toast,.new-task-toast,.style-picker,dialog';
(window as any).scanOverflow = function () {
  const visible = (el: Element) => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && !el.closest("[hidden],.sr-only"); };
  const label = (el: any) => el.tagName.toLowerCase() + (typeof el.className === "string" && el.className ? "." + el.className.trim().split(/\s+/).join(".") : "") + ' "' + (el.textContent || el.value || el.placeholder || "").trim().slice(0, 40) + '"';
  const leaves = [...document.querySelectorAll("body *")].filter((el) => visible(el) && ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent!.trim()) || /INPUT|TEXTAREA|SELECT/.test(el.tagName)));
  const problems: string[] = [];
  for (const el of leaves) {
    if (/INPUT|TEXTAREA|SELECT/.test(el.tagName)) continue;
    const s = getComputedStyle(el);
    if (el.scrollWidth > el.clientWidth + 1 && s.textOverflow !== "ellipsis" && !/auto|scroll/.test(s.overflowX)) problems.push(`CLIP/SPILL ${label(el)} scroll=${el.scrollWidth} box=${el.clientWidth}`);
  }
  // A closed menu shows only its chosen name; measure that against the menu's width.
  const pen = document.createElement("canvas").getContext("2d")!;
  for (const el of leaves.filter((each) => each.tagName === "SELECT") as HTMLSelectElement[]) {
    const s = getComputedStyle(el);
    pen.font = `${s.fontWeight} ${s.fontSize} ${s.fontFamily}`;
    const text = el.options[el.selectedIndex]?.text ?? "";
    if (pen.measureText(text).width > el.clientWidth - 24 && s.textOverflow !== "ellipsis") problems.push(`CLIP select ${label(el)} "${text.slice(0, 40)}"`);
  }
  const rects = leaves.map((el) => [el, el.getBoundingClientRect()] as const);
  for (let i = 0; i < rects.length; i++) for (let j = i + 1; j < rects.length; j++) {
    const [a, ra] = rects[i], [b, rb] = rects[j];
    if (a.contains(b) || b.contains(a)) continue;
    if (!!a.closest(FLOATING) !== !!b.closest(FLOATING)) continue;
    const w = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left), h = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
    if (w > 2 && h > 2) problems.push(`OVERLAP ${label(a)} <-> ${label(b)} (${Math.round(w)}x${Math.round(h)})`);
  }
  if (document.documentElement.scrollWidth > innerWidth) problems.push("PAGE scrolls sideways");
  return problems;
};
