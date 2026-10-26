import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import DataSettings from "./components/DataSettings";
import type { Backup } from "./lib/data-api";
import { invoke as appInvoke } from "./lib/invoke";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);

/** Synthetic backups only; the rules (validation, retention) are the core's, tested in Rust. */
let backups: Backup[];
const backup = (id: string, kind: Backup["kind"], createdAt: number): Backup => ({
  id, kind, createdAt, schemaVersion: 13, sizeBytes: 4096, sha256: "0".repeat(64),
});
const OCT_26 = Date.UTC(2026, 9, 26, 9, 15);
const OCT_25 = Date.UTC(2026, 9, 25, 18, 0);

beforeEach(() => {
  backups = [backup("b-daily", "daily", OCT_26), backup("b-manual", "manual", OCT_25)];
  call.mockReset();
  call.mockImplementation(async (command, args) => {
    const body = (args ?? {}) as Record<string, any>;
    switch (command) {
      case "list_backups": return backups.map((b) => ({ ...b }));
      case "create_backup": {
        const made = backup("b-new", "manual", OCT_26 + 60_000);
        backups = [made, ...backups];
        return made;
      }
      case "restore_backup":
        return { restored: backups.find((b) => b.id === body.id), preRestore: backup("b-pre", "pre_restore", OCT_26 + 1) };
      case "export_data":
        return {
          destination: `/Users/someone/Downloads/Tasknotes export.${body.format === "json" ? "json" : "md"}`,
          format: body.format, lists: 1, tasks: 2, noteFolders: 0, notes: 3, noteLinks: 0,
          privateNotes: body.includePrivateNotes ? 1 : 0,
        };
      case "adopt_restored_data": return null;
      default: throw new Error(`Unexpected command ${command}`);
    }
  });
});

afterEach(cleanup);

describe("Backups and export settings", () => {
  it("lists backups newest first and backs up on request", async () => {
    const user = userEvent.setup();
    render(<DataSettings active onRestored={() => {}} />);
    const list = await screen.findByRole("list", { name: "Backups" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Daily");
    expect(rows[1]).toHaveTextContent("Manual");

    await user.click(screen.getByRole("button", { name: "Back up now" }));
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(3));
    expect(call).toHaveBeenCalledWith("create_backup", undefined);
  });

  it("asks inline before restoring, then reloads the window", async () => {
    const user = userEvent.setup();
    const onRestored = vi.fn();
    render(<DataSettings active onRestored={onRestored} />);
    const list = await screen.findByRole("list", { name: "Backups" });
    const second = within(list).getAllByRole("listitem")[1];
    await user.click(within(second).getByRole("button", { name: /Restore/ }));

    // Nothing is replaced until the person confirms.
    expect(call).not.toHaveBeenCalledWith("restore_backup", expect.anything());
    const question = screen.getByRole("group", { name: /Replace all current data/ });
    await user.click(within(question).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("group", { name: /Replace all current data/ })).toBeNull();

    await user.click(within(second).getByRole("button", { name: /Restore/ }));
    await user.click(screen.getByRole("button", { name: "Replace and restore" }));
    expect(call).toHaveBeenCalledWith("restore_backup", { id: "b-manual" });
    await waitFor(() => expect(onRestored).toHaveBeenCalledTimes(1));
  });

  it("shows a refused restore and keeps the window as it is", async () => {
    const user = userEvent.setup();
    const onRestored = vi.fn();
    call.mockImplementation(async (command) => {
      if (command === "list_backups") return backups;
      throw "This backup cannot be restored: the snapshot is damaged. Your current data was not changed";
    });
    render(<DataSettings active onRestored={onRestored} />);
    const list = await screen.findByRole("list", { name: "Backups" });
    await user.click(within(within(list).getAllByRole("listitem")[0]).getByRole("button", { name: /Restore/ }));
    await user.click(screen.getByRole("button", { name: "Replace and restore" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("the snapshot is damaged");
    expect(onRestored).not.toHaveBeenCalled();
  });

  it("exports without private Notes unless the person includes them", async () => {
    const user = userEvent.setup();
    render(<DataSettings active onRestored={() => {}} />);
    const include = await screen.findByRole("checkbox", { name: "Include Notes marked Exclude from AI" });
    expect(include).not.toBeChecked();

    await user.click(screen.getByRole("button", { name: "Export as JSON" }));
    expect(call).toHaveBeenCalledWith("export_data", { format: "json", includePrivateNotes: false });
    expect(await screen.findByRole("status")).toHaveTextContent("Saved to /Users/someone/Downloads/Tasknotes export.json");

    await user.click(include);
    await user.click(screen.getByRole("button", { name: "Export as Markdown" }));
    expect(call).toHaveBeenCalledWith("export_data", { format: "markdown", includePrivateNotes: true });
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("including 1 private Note"));
  });

  it("stays quiet where backups are not available", async () => {
    call.mockImplementation(async () => { throw new Error("Command list_backups not found"); });
    render(<DataSettings active onRestored={() => {}} />);
    expect(await screen.findByText("Backups are not available on this device.")).toBeInTheDocument();
  });

  it("does not ask for backups until Settings is shown", () => {
    render(<DataSettings active={false} onRestored={() => {}} />);
    expect(call).not.toHaveBeenCalled();
  });
});

describe("after a restore made elsewhere", () => {
  it("accepts the restored data and reloads instead of writing stale state", async () => {
    const reload = vi.fn();
    vi.stubGlobal("location", { ...window.location, reload });
    call.mockImplementation(async (command) => {
      if (command === "adopt_restored_data") return null;
      throw "Tasknotes data was restored from a backup since this was opened; reload Tasknotes to continue";
    });
    await expect(appInvoke("update_task", { input: {} })).rejects.toContain("restored from a backup");
    await waitFor(() => expect(reload).toHaveBeenCalledTimes(1));
    expect(call).toHaveBeenCalledWith("adopt_restored_data");
    vi.unstubAllGlobals();
  });

  it("passes every other answer through unchanged", async () => {
    call.mockImplementation(async () => "ok");
    await expect(appInvoke<string>("list_tasks")).resolves.toBe("ok");
    call.mockImplementation(async () => { throw "Revision conflict"; });
    await expect(appInvoke("update_task")).rejects.toBe("Revision conflict");
    expect(call).not.toHaveBeenCalledWith("adopt_restored_data");
  });
});
