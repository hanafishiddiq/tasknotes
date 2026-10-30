import { invoke } from "@tauri-apps/api/core";
import type { WorkStatus } from "./task-api";

/** What an AI interface asked to do, by the core's action names. */
export type BulkAction =
  | { action: "remove_tasks" | "restore_tasks" | "remove_notes" | "restore_notes" | "remove_list" | "remove_folder" }
  | { action: "move_tasks"; list_id: string | null }
  | { action: "set_task_status"; status: WorkStatus };

export type BulkRequestStatus = "pending" | "applied" | "declined" | "stale" | "expired";

export interface BulkRequest {
  id: string;
  action: BulkAction;
  /** How many items it names; null for a List or folder removal. */
  count: number | null;
  requestedBy: "desktop" | "cli" | "mcp";
  createdAt: number;
  expiresAt: number;
  status: BulkRequestStatus;
  decidedAt: number | null;
}

export interface ApprovalLine {
  kind: "task" | "note" | "folder";
  id: string;
  title: string;
  excludedFromAi: boolean;
}

/** A request waiting for the person, with what it would change or hide. */
export interface PendingBulkRequest {
  request: BulkRequest;
  /** Something changed since it was asked: approving would be refused. */
  stale: boolean;
  /** The target List of a move, or the List or folder being removed. */
  target: string | null;
  items: ApprovalLine[];
  total: number;
}

// Approving and declining exist only here, in the window: an AI interface
// can ask for a Bulk Change but never decide it. The rules are the core's.
export const approvalApi = {
  pending(): Promise<PendingBulkRequest[]> {
    return invoke("list_pending_bulk_requests");
  },
  count(): Promise<number> {
    return invoke("count_pending_bulk_requests");
  },
  approve(id: string): Promise<BulkRequest> {
    return invoke("approve_bulk_request", { id });
  },
  decline(id: string): Promise<BulkRequest> {
    return invoke("decline_bulk_request", { id });
  },
};
