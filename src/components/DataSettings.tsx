import { useEffect, useState } from "react";
import { dataApi, type Backup, type BackupKind, type ExportFormat, type ExportSummary } from "../lib/data-api";
import { errorMessage } from "../lib/error-message";

interface DataSettingsProps {
  /** Settings is on screen; backups are read the first time it is. */
  active: boolean;
  /** The data was replaced: the window must reload so nothing stale is kept. */
  onRestored: () => void;
}

type Loading = { kind: "loading" } | { kind: "missing" } | { kind: "ready"; backups: Backup[] };

const KIND_LABELS: Record<BackupKind, string> = {
  daily: "Daily",
  manual: "Manual",
  pre_restore: "Before a restore",
};

const when = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/**
 * Backups, restore and export. The core takes, checks and restores backups
 * and writes exports; this section only shows and asks.
 */
export default function DataSettings({ active, onRestored }: DataSettingsProps) {
  const [loading, setLoading] = useState<Loading>({ kind: "loading" });
  const [requested, setRequested] = useState(false);
  const [confirming, setConfirming] = useState<Backup | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [includePrivate, setIncludePrivate] = useState(false);
  const [exported, setExported] = useState<ExportSummary | null>(null);

  useEffect(() => {
    if (!active || requested) return;
    setRequested(true);
    dataApi.listBackups()
      // An older shell without these commands answers nothing useful: say so quietly.
      .then((backups) => setLoading(Array.isArray(backups) ? { kind: "ready", backups } : { kind: "missing" }))
      .catch(() => setLoading({ kind: "missing" }));
  }, [active, requested]);

  async function act<T>(request: () => Promise<T>): Promise<T | null> {
    setError(null);
    setBusy(true);
    try {
      return await request();
    } catch (reason) {
      setError(errorMessage(reason));
      return null;
    } finally {
      setBusy(false);
    }
  }

  async function backUpNow() {
    if (await act(dataApi.createBackup)) {
      const backups = await dataApi.listBackups().catch(() => null);
      if (Array.isArray(backups)) setLoading({ kind: "ready", backups });
    }
  }

  async function restore(backup: Backup) {
    setConfirming(null);
    if (await act(() => dataApi.restoreBackup(backup.id))) onRestored();
  }

  async function exportAs(format: ExportFormat) {
    setExported(null);
    const summary = await act(() => dataApi.exportData(format, includePrivate));
    if (summary) setExported(summary);
  }

  return (
    <section className="settings-section" aria-labelledby="settings-data">
      <h2 className="settings-heading" id="settings-data">Backups and export</h2>
      {loading.kind === "missing" && (
        <p className="settings-hint">Backups are not available on this device.</p>
      )}
      {loading.kind === "ready" && (
        <>
          <p className="settings-hint">
            Tasknotes keeps a backup each day it runs: the last 7 days, 4 weeks and 3 months.
          </p>
          <ul className="settings-backups" aria-label="Backups">
            {loading.backups.map((backup) => (
              <li key={backup.id} className="settings-backup">
                <span>{when.format(backup.createdAt)}</span>
                <span className="settings-backup-kind">{KIND_LABELS[backup.kind]}</span>
                <button type="button" className="button button-small" disabled={busy}
                  aria-label={`Restore the backup from ${when.format(backup.createdAt)}`}
                  onClick={() => setConfirming(backup)}>
                  Restore…
                </button>
              </li>
            ))}
          </ul>
          {loading.backups.length === 0 && <p className="settings-hint">No backups yet.</p>}
          {confirming && (
            <div className="settings-confirm" role="group"
              aria-label={`Replace all current data with the backup from ${when.format(confirming.createdAt)}?`}>
              <p className="settings-hint">
                Replace all current data with the backup from {when.format(confirming.createdAt)}?
                A backup of the current data is kept first.
              </p>
              <div className="settings-shortcut-row">
                <button type="button" className="button button-small button-primary" disabled={busy}
                  onClick={() => restore(confirming)}>
                  Replace and restore
                </button>
                <button type="button" className="button button-small" onClick={() => setConfirming(null)}>
                  Cancel
                </button>
              </div>
            </div>
          )}
          <div className="settings-shortcut-row">
            <button type="button" className="button button-small" disabled={busy} onClick={backUpNow}>
              Back up now
            </button>
          </div>
        </>
      )}

      <label className="settings-choice">
        <input type="checkbox" checked={includePrivate}
          onChange={(event) => setIncludePrivate(event.target.checked)} />
        <span>Include Notes marked Exclude from AI</span>
      </label>
      <div className="settings-shortcut-row">
        <button type="button" className="button button-small" disabled={busy} onClick={() => exportAs("json")}>
          Export as JSON
        </button>
        <button type="button" className="button button-small" disabled={busy} onClick={() => exportAs("markdown")}>
          Export as Markdown
        </button>
      </div>
      <p className="settings-hint">
        Exports go to your Downloads folder. Items in Removed are included and labeled.
      </p>
      {exported && (
        <p className="settings-hint" role="status">
          Saved to {exported.destination}
          {exported.privateNotes > 0 &&
            `, including ${exported.privateNotes} private Note${exported.privateNotes === 1 ? "" : "s"}`}
          .
        </p>
      )}
      {error && <p className="settings-hint settings-hint--error" role="alert">{error}</p>}
    </section>
  );
}
