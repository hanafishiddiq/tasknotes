import type { Backup } from "../lib/data-api";
import type { PendingBulkRequest } from "../lib/approval-api";
import type { LinkedNote, LinkedTask } from "../lib/link-api";
import type { RemovedItem } from "../lib/removed-api";
import type { QuickCaptureStatus, ShellView } from "../lib/shell-api";
import type { ProgressEntry, Task, TaskList } from "../lib/task-api";
import { NoteBackend } from "./note-backend";

const DAY = 86_400_000;
/** A fixed instant, so no fixture depends on when the test runs. */
export const FIXTURE_NOW = Date.UTC(2026, 9, 26, 9, 15);

export function fixtureTask(id: string, fields: Partial<Task> = {}): Task {
  return {
    id, title: `Synthetic task ${id}`, description: "", status: "ready", priority: "medium",
    progress: 0, nextStep: "", revision: 1, createdAt: FIXTURE_NOW, updatedAt: FIXTURE_NOW,
    listId: null, dueDate: null, tags: [], ...fields,
  };
}

export function fixtureList(id: string, name: string): TaskList {
  return { id, name, revision: 1, createdAt: FIXTURE_NOW, updatedAt: FIXTURE_NOW, removed: false };
}

/** Every look a closed row can take: all statuses, priorities, deadlines and tag counts. */
export function everyRowKind(): Task[] {
  return [
    fixtureTask("a", { title: "Synthetic draft", status: "in_progress", progress: 40, nextStep: "Write the outline", priority: "very_high", dueDate: "2020-01-01", tags: ["alpha", "beta", "gamma", "delta"] }),
    fixtureTask("b", { title: "Synthetic review", status: "in_progress", progress: 10, description: "Longer context", priority: "high", dueDate: "2999-01-01", tags: ["alpha"] }),
    fixtureTask("c", { title: "Synthetic errand", priority: "low" }),
    fixtureTask("d", { title: "Synthetic filing", priority: "very_low" }),
    fixtureTask("e", { title: "Synthetic archive", status: "completed", progress: 100 }),
  ];
}

export function fixtureRequest(id: string, fields: Partial<PendingBulkRequest> = {}): PendingBulkRequest {
  return {
    request: {
      id, action: { action: "remove_tasks" }, count: 2, requestedBy: "mcp", createdAt: FIXTURE_NOW,
      expiresAt: FIXTURE_NOW + DAY, status: "pending", decidedAt: null,
    },
    stale: false, target: null, total: 2,
    items: [
      { kind: "task", id: "a", title: "Synthetic draft", excludedFromAi: false },
      { kind: "note", id: "n", title: "Synthetic private note", excludedFromAi: true },
    ],
    ...fields,
  };
}

export function fixtureBackup(id: string, kind: Backup["kind"], createdAt = FIXTURE_NOW): Backup {
  return { id, kind, createdAt, schemaVersion: 13, sizeBytes: 4096, sha256: "0".repeat(64) };
}

/** Markdown holding every block the Note editor styles. */
export const RICH_NOTE = [
  "# Synthetic heading", "", "## Synthetic subheading", "", "Plain text with **bold**, *italic*, ~~struck~~, `code` and a [link](https://example.com/).",
  "", "- First bullet", "- Second bullet", "", "1. First number", "2. Second number", "", "> A synthetic quote", "",
  "```", "const synthetic = true;", "```",
].join("\n");

/**
 * The whole app's far side of the Tauri seam, in memory, for accessibility
 * checks that show every view with synthetic data. It mimics only what a window
 * observes; the rules are the core's and are tested in Rust. Anything it does
 * not know throws, so a view asking for something new fails loudly.
 */
export class World {
  notes = new NoteBackend();
  tasks: Task[] = [];
  private removedTasks: Task[] = [];
  /** Set to make the Task list say another page follows. */
  taskNextCursor: string | null = null;
  lists: TaskList[] = [];
  removed: RemovedItem[] = [];
  pending: PendingBulkRequest[] = [];
  backups: Backup[] = [fixtureBackup("daily", "daily"), fixtureBackup("manual", "manual", FIXTURE_NOW - DAY)];
  history: ProgressEntry[] = [];
  notesOfTask: LinkedNote[] = [];
  tasksOfNote: LinkedTask[] = [];
  launchNotice: string | null = null;
  shell: ShellView = {
    keepInMenuBar: false, trayPlace: "menu bar", closeHint: "Synthetic hint: closing quits.",
    quickCaptureEnabled: false, quickCapture: { state: "off" }, rememberedLabel: null, settingsUnreadable: false,
  };
  /** What the shell answers to the next set_quick_capture. */
  nextCapture: QuickCaptureStatus = { state: "active", label: "⌃⌥Space" };
  /** The shell's queued Quick Capture, collected by the page when it is told. */
  pendingCapture = false;
  /** Commands that answer with this refusal, as the core would. */
  refuse = new Map<string, string>();
  private clock = FIXTURE_NOW;

  handle(command: string, args: Record<string, any>): unknown {
    const refusal = this.refuse.get(command);
    if (refusal) throw refusal;
    switch (command) {
      case "list_lists": return { items: this.lists.map((list) => ({ ...list })), nextCursor: null };
      case "list_tasks": return { items: this.matchingTasks(args), nextCursor: args.cursor ? null : this.taskNextCursor };
      case "create_task": {
        const created = fixtureTask(`new-${++this.clock}`, { title: args.input.title, description: args.input.description });
        this.tasks.push(created);
        return created;
      }
      case "update_task":
      case "change_task":
      case "record_progress":
      case "update_task_details": {
        const found = this.tasks.find((task) => task.id === args.input.id)!;
        const { id: _id, expectedRevision: _revision, note: _note, ...fields } = args.input;
        Object.assign(found, fields, { revision: found.revision + 1 });
        return { ...found };
      }
      case "set_task_removed": {
        const found = [...this.tasks, ...this.removedTasks].find((task) => task.id === args.input.id)!;
        found.revision++;
        if (args.input.removed) {
          this.tasks = this.tasks.filter((task) => task !== found);
          this.removedTasks.push(found);
          this.removed.push({ kind: "task", id: found.id, title: found.title, revision: found.revision, removedAt: ++this.clock, inRemovedContainer: false });
        } else {
          this.tasks.push(found);
          this.removedTasks = this.removedTasks.filter((task) => task !== found);
          this.removed = this.removed.filter((item) => item.id !== found.id);
        }
        return { ...found };
      }
      case "set_list_removed": {
        const found = this.lists.find((list) => list.id === args.input.id)!;
        Object.assign(found, { removed: args.input.removed, revision: found.revision + 1 });
        return { ...found };
      }
      case "rename_list": {
        const found = this.lists.find((list) => list.id === args.input.id)!;
        Object.assign(found, { name: args.input.name, revision: found.revision + 1 });
        return { ...found };
      }
      case "list_progress_entries": return { items: this.history, nextCursor: null };
      case "list_task_notes": return { items: this.notesOfTask, nextCursor: null };
      case "list_note_tasks": return { items: this.tasksOfNote, nextCursor: null };
      case "list_removed": {
        const words = String(args.text ?? "").toLowerCase().split(/\W+/).filter(Boolean);
        const items = this.removed.filter((item) => words.every((word) => item.title.toLowerCase().includes(word)));
        return { items, nextCursor: null };
      }
      case "count_pending_bulk_requests": return this.pending.length;
      case "list_pending_bulk_requests": return this.pending.map((item) => structuredClone(item));
      case "approve_bulk_request":
      case "decline_bulk_request": {
        const found = this.pending.find((item) => item.request.id === args.id)!;
        this.pending = this.pending.filter((item) => item !== found);
        return found.request;
      }
      case "get_shell_settings": return { ...this.shell };
      case "set_keep_in_menu_bar":
        this.shell = { ...this.shell, keepInMenuBar: args.enabled };
        return { ...this.shell };
      case "set_quick_capture": {
        const status: QuickCaptureStatus = args.input.enabled ? this.nextCapture : { state: "off" };
        this.shell = {
          ...this.shell, quickCapture: status, quickCaptureEnabled: status.state === "active",
          rememberedLabel: status.state === "active" ? status.label : this.shell.rememberedLabel,
        };
        return { ...this.shell };
      }
      case "take_quick_capture": {
        const pending = this.pendingCapture;
        this.pendingCapture = false;
        return pending;
      }
      case "take_launch_notice": {
        const notice = this.launchNotice;
        this.launchNotice = null;
        return notice;
      }
      case "list_backups": return this.backups.map((backup) => ({ ...backup }));
      case "create_backup": return this.backups[0];
      case "export_data":
        return {
          destination: "/Users/someone/Downloads/Tasknotes export.json", format: args.format, lists: 1, tasks: 2,
          noteFolders: 0, notes: 3, noteLinks: 0, privateNotes: args.includePrivateNotes ? 1 : 0,
        };
      case "quit_ready": return null;
      default: return this.notes.handle(command, args);
    }
  }

  private matchingTasks(args: Record<string, any>): Task[] {
    return this.tasks
      .filter((task) => !args.taskId || task.id === args.taskId)
      .filter((task) => args.view?.kind !== "list" || task.listId === args.view.listId)
      .filter((task) => args.view?.kind !== "inbox" || task.listId === null)
      .filter((task) => !args.status || task.status === args.status)
      .filter((task) => !args.priority || task.priority === args.priority)
      .filter((task) => !args.tag || task.tags.includes(String(args.tag)))
      .filter((task) => !args.text || task.title.toLowerCase().includes(String(args.text).toLowerCase()))
      .map((task) => ({ ...task }));
  }
}
