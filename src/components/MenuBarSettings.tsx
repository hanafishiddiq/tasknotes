import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  acceleratorFromKey, shellApi, type QuickCaptureStatus, type ShellView,
} from "../lib/shell-api";
import { errorMessage } from "../lib/error-message";
import { newTaskKeys } from "../lib/shortcuts";

interface MenuBarSettingsProps {
  /** Settings is on screen; the shell's settings are read the first time it is. */
  active: boolean;
}

type Loading = { kind: "loading" } | { kind: "missing" } | { kind: "ready"; view: ShellView };

/**
 * The desktop shell's settings: menu-bar mode and the global Quick Capture
 * shortcut. The shell owns and saves them; this section only shows and asks.
 */
export default function MenuBarSettings({ active }: MenuBarSettingsProps) {
  const [loading, setLoading] = useState<Loading>({ kind: "loading" });
  const [requested, setRequested] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const toggle = useRef<HTMLInputElement>(null);
  const recorder = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (recording) recorder.current?.focus();
  }, [recording]);

  useEffect(() => {
    if (!active || requested) return;
    setRequested(true);
    shellApi.get()
      .then((view) => setLoading({ kind: "ready", view }))
      // An older shell or another device without these commands: say so quietly.
      .catch(() => setLoading({ kind: "missing" }));
  }, [active, requested]);

  async function change(request: () => Promise<ShellView>): Promise<ShellView | null> {
    setSaveError(null);
    try {
      const view = await request();
      setLoading({ kind: "ready", view });
      return view;
    } catch (reason) {
      setSaveError(errorMessage(reason));
      // Show what the shell really has, so no checkbox claims a change that was undone.
      shellApi.get().then((view) => setLoading({ kind: "ready", view })).catch(() => {});
      return null;
    }
  }

  function toggleQuickCapture(view: ShellView, on: boolean) {
    if (!on) {
      setRecording(false);
      void change(() => shellApi.setQuickCapture(false));
    } else if (view.rememberedLabel) {
      void change(() => shellApi.setQuickCapture(true));
    } else {
      setRecording(true);
    }
  }

  function stopRecording() {
    setRecording(false);
    toggle.current?.focus();
  }

  async function record(event: KeyboardEvent<HTMLButtonElement>) {
    // Every key belongs to the recorder, so ⌘N and the like do not act meanwhile.
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape" && !(event.ctrlKey || event.altKey || event.shiftKey || event.metaKey)) {
      stopRecording();
      return;
    }
    const accelerator = acceleratorFromKey(event);
    if (!accelerator) return;
    const view = await change(() => shellApi.setQuickCapture(true, accelerator));
    if (view?.quickCapture.state === "active") stopRecording();
  }

  const heading = loading.kind === "ready"
    ? `${capitalized(loading.view.trayPlace)} and Quick Capture` : "Quick Capture";

  return (
    <section className="settings-section" aria-labelledby="settings-menu-bar">
      <h2 className="settings-heading" id="settings-menu-bar">{heading}</h2>
      {loading.kind === "missing" && (
        <p className="settings-hint">These settings are not available on this device.</p>
      )}
      {loading.kind === "ready" && (
        <>
          {loading.view.settingsUnreadable && (
            <p className="settings-hint" role="status">
              Tasknotes could not read these saved settings, so the defaults are in use. Changing a setting saves them again.
            </p>
          )}
          <label className="settings-choice">
            <input type="checkbox" checked={loading.view.keepInMenuBar}
              onChange={(event) => change(() => shellApi.setKeepInMenuBar(event.target.checked))} />
            <span>Keep Tasknotes in the {loading.view.trayPlace}</span>
          </label>
          <p className="settings-hint">{loading.view.closeHint}</p>

          <label className="settings-choice">
            <input ref={toggle} type="checkbox"
              checked={loading.view.quickCaptureEnabled || recording}
              onChange={(event) => toggleQuickCapture(loading.view, event.target.checked)} />
            <span>Open Quick Capture with a global shortcut</span>
          </label>
          {recording ? (
            <div className="settings-shortcut-row">
              <button ref={recorder} type="button" className="button button-small settings-recorder"
                onKeyDown={record} onBlur={() => setRecording(false)}>
                Press the new shortcut
              </button>
              <span className="settings-hint">Esc cancels.</span>
            </div>
          ) : loading.view.quickCaptureEnabled && (
            <div className="settings-shortcut-row">
              {loading.view.rememberedLabel && <kbd>{loading.view.rememberedLabel}</kbd>}
              <button type="button" className="button button-small" onClick={() => setRecording(true)}>
                Change shortcut
              </button>
            </div>
          )}
          <QuickCaptureHint view={loading.view} recording={recording} />
        </>
      )}
      {saveError && <p className="settings-hint settings-hint--error" role="alert">{saveError}</p>}
    </section>
  );
}

function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function statusLines(status: QuickCaptureStatus): string[] {
  switch (status.state) {
    case "off":
    case "active":
      return [];
    case "unavailable":
    case "rejected":
      return [status.reason, ...(status.current ? [`${status.current} still opens Quick Capture.`] : [])];
  }
}

/** What the shortcut does now, and why not when it could not be used. */
function QuickCaptureHint({ view, recording }: { view: ShellView; recording: boolean }) {
  const status = view.quickCapture;
  if (status.state === "active" && !recording) {
    return <p className="settings-hint">Press {status.label} in any app to add a task.</p>;
  }
  const problems = statusLines(status);
  if (problems.length > 0) {
    return (
      <div className="settings-hint settings-hint--error" role="status">
        {problems.map((line) => <p key={line}>{line}</p>)}
        <p>{newTaskKeys} still creates a task inside Tasknotes.</p>
      </div>
    );
  }
  if (recording) return null;
  return (
    <p className="settings-hint">
      {view.rememberedLabel
        ? `Off. ${view.rememberedLabel} is kept for when you turn it back on.`
        : `Off. ${newTaskKeys} creates a task inside Tasknotes.`}
    </p>
  );
}
