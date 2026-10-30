import { useEffect, useState } from "react";
import { Check, ClipboardCheck, Folder, LoaderCircle, NotebookText, SquareCheck, X } from "lucide-react";
import { errorMessage } from "../lib/error-message";
import { approvalApi, type ApprovalLine, type PendingBulkRequest } from "../lib/approval-api";
import { STATUS_VIEW } from "../lib/task-visuals";

const ICON: Record<ApprovalLine["kind"], typeof Folder> = {
  task: SquareCheck,
  note: NotebookText,
  folder: Folder,
};

const ASKED_AT = new Intl.DateTimeFormat(undefined, {
  month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
});

const ASKED_BY: Record<PendingBulkRequest["request"]["requestedBy"], string> = {
  mcp: "an MCP client",
  cli: "the tasknotes command",
  desktop: "this window",
};

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** One plain sentence saying what approving would do. */
export function describeRequest({ request, target, total }: PendingBulkRequest): string {
  const n = request.count ?? total;
  const tasks = plural(n, "Task", "Tasks");
  const notes = plural(n, "Note", "Notes");
  switch (request.action.action) {
    case "remove_tasks": return `Move ${tasks} to Removed`;
    case "restore_tasks": return `Restore ${tasks} from Removed`;
    case "move_tasks": return `Move ${tasks} to ${target ? `“${target}”` : "Inbox"}`;
    case "set_task_status": return `Mark ${tasks} ${STATUS_VIEW[request.action.status].label}`;
    case "remove_notes": return `Move ${notes} to Removed`;
    case "restore_notes": return `Restore ${notes} from Removed`;
    case "remove_list":
      return `Move the List “${target ?? "List"}” and ${plural(total, "Task", "Tasks")} in it to Removed`;
    case "remove_folder":
      return `Move the folder “${target ?? "Folder"}” and ${plural(total, "item", "items")} in it to Removed`;
  }
}

interface ApprovalsViewProps {
  hidden: boolean;
  /** Asked again whenever this changes (e.g. the sidebar count moved). */
  refreshKey: number;
  /** After an approval or decline, so other views and the count catch up. */
  onDecided: (approved: boolean) => void;
}

/**
 * Bulk Changes an AI client asked for (owner decision A): nothing changes
 * until the person approves here. A request that went stale can only be
 * declined; the AI then has to ask again.
 */
export default function ApprovalsView({ hidden, refreshKey, onDecided }: ApprovalsViewProps) {
  const [requests, setRequests] = useState<PendingBulkRequest[]>([]);
  const [loading, setLoading] = useState(true);
  const [deciding, setDeciding] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    if (hidden) return;
    let active = true;
    approvalApi.pending()
      .then((items) => { if (active) { setRequests(items); setError(null); } })
      .catch((reason) => { if (active) setError("Couldn't load requests: " + errorMessage(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [hidden, refreshKey, reloadKey]);

  async function decide(pending: PendingBulkRequest, approve: boolean) {
    if (deciding) return;
    setDeciding(pending.request.id);
    setError(null);
    try {
      if (approve) await approvalApi.approve(pending.request.id);
      else await approvalApi.decline(pending.request.id);
      setRequests((current) => current.filter((other) => other.request.id !== pending.request.id));
      onDecided(approve);
    } catch (reason) {
      setError(errorMessage(reason));
      // The core may have marked it stale or expired: ask again.
      setReloadKey((key) => key + 1);
      onDecided(false);
    } finally {
      setDeciding(null);
    }
  }

  return (
    <main className="main-view" hidden={hidden}>
      <div className="main-scroll">
        <section className="content-wrap" aria-label="Waiting for approval">
          <div className="page-title"><h1>Waiting for approval</h1></div>
          <p className="removed-hint">
            Changes an AI client asked for. Nothing changes until you approve; a request expires after a day.
          </p>
          {error && <div role="alert" className="error-banner"><span>{error}</span></div>}
          {loading ? (
            <div className="loading-state" role="status"><LoaderCircle className="spin" size={16} /> Loading requests...</div>
          ) : requests.length === 0 ? (
            <div className="empty-state">
              <ClipboardCheck size={26} strokeWidth={1.5} aria-hidden="true" />
              <h3>Nothing waiting for approval</h3>
            </div>
          ) : (
            <ul className="approval-list">
              {requests.map((pending) => {
                const sentence = describeRequest(pending);
                const more = pending.total - pending.items.length;
                return (
                  <li key={pending.request.id} className="approval" aria-label={sentence}>
                    <h2 className="approval-title">{sentence}</h2>
                    <p className="approval-meta">
                      Asked by {ASKED_BY[pending.request.requestedBy]} · {ASKED_AT.format(pending.request.createdAt)}
                      {" "}· expires {ASKED_AT.format(pending.request.expiresAt)}
                    </p>
                    <ul className="removed-list">
                      {pending.items.map((line) => {
                        const Icon = ICON[line.kind];
                        return (
                          <li key={line.id} className="removed-item">
                            <Icon size={16} aria-hidden="true" />
                            <span className="removed-item-text">
                              <span className="removed-item-title">{line.title}</span>
                              {line.excludedFromAi && (
                                <span className="removed-item-meta">Excluded from AI</span>
                              )}
                            </span>
                          </li>
                        );
                      })}
                    </ul>
                    {more > 0 && <p className="approval-meta">and {plural(more, "more", "more")}</p>}
                    {pending.stale && (
                      <p className="approval-stale">
                        Something changed since this was asked, so it can't be approved as it is.
                      </p>
                    )}
                    <div className="approval-actions">
                      <button type="button" className="button button-small" disabled={deciding !== null}
                        aria-label={`Decline: ${sentence}`} onClick={() => void decide(pending, false)}>
                        <X size={13} aria-hidden="true" /> Decline
                      </button>
                      <button type="button" className="button button-small button-primary"
                        disabled={deciding !== null || pending.stale}
                        aria-label={`Approve: ${sentence}`} onClick={() => void decide(pending, true)}>
                        <Check size={13} aria-hidden="true" /> Approve
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </div>
    </main>
  );
}
