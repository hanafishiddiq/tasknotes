import { invoke } from "./invoke";

export type BackupKind = "daily" | "manual" | "pre_restore";

/** A Backup as its manifest describes it (the core's `backup::Backup`). */
export interface Backup {
  id: string;
  kind: BackupKind;
  createdAt: number;
  schemaVersion: number;
  sizeBytes: number;
  sha256: string;
}

export interface RestoreOutcome {
  restored: Backup;
  preRestore: Backup;
}

export type ExportFormat = "json" | "markdown";

export interface ExportSummary {
  destination: string;
  format: ExportFormat;
  lists: number;
  tasks: number;
  noteFolders: number;
  notes: number;
  noteLinks: number;
  privateNotes: number;
}

export const dataApi = {
  listBackups: () => invoke<Backup[]>("list_backups"),
  createBackup: () => invoke<Backup>("create_backup"),
  restoreBackup: (id: string) => invoke<RestoreOutcome>("restore_backup", { id }),
  /** Written to the Downloads folder under a new name; never overwrites. */
  exportData: (format: ExportFormat, includePrivateNotes: boolean) =>
    invoke<ExportSummary>("export_data", { format, includePrivateNotes }),
};
