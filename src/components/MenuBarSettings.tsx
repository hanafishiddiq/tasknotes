import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  acceleratorFromKey, shellApi, type QuickCaptureStatus, type ShellView, type ShortcutProblem,
} from "../lib/shell-api";
import { errorMessage } from "../lib/error-message";

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

  return (
    <section className="settings-section" aria-labelledby="settings-menu-bar">
      <h2 className="settings-heading" id="settings-menu-bar">Menu bar and Quick Capture</h2>
      {loading.kind === "missing" && (
        <p className="settings-hint">Menu bar and Quick Capture are not available on this device.</p>
      )}
      {loading.kind === "ready" && (
        <>
          <label className="settings-choice">
            <input type="checkbox" checked={loading.view.keepInMenuBar}
              onChange={(event) => change(() => shellApi.setKeepInMenuBar(event.target.checked))} />
            <span>Keep Tasknotes in the menu bar</span>
          </label>
          <p className="settings-hint">
            {!loading.view.keepInMenuBar ? "Closing the window quits Tasknotes."
              : loading.view.menuBarUnavailable
                ? "Tasknotes could not show its menu-bar item here, so closing the window still quits."
                : "Closing the window leaves Tasknotes in the menu bar. Choose Quit Tasknotes there, or press ⌘Q, to quit."}
          </p>

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

function problemText(problem: ShortcutProblem): string {
  switch (problem.kind) {
    case "needsTwoModifiers":
      return "Use at least two modifier keys (⌃ ⌥ ⇧ ⌘) with a letter, number or Space, or a function key on its own. A single modifier would take over shortcuts other apps use, like ⌘C, and Tasknotes' own ⌘N.";
    case "reservedBySystem":
      return `macOS uses this for ${problem.usedFor}. Pick another combination.`;
    case "unsupportedKey":
      return "Media keys cannot be used. Pick another key.";
    case "invalid":
      return "Record a key combination to turn Quick Capture on.";
  }
}

function statusLines(status: QuickCaptureStatus): string[] {
  switch (status.state) {
    case "off":
    case "active":
      return [];
    case "unavailable":
      return [`${status.label} is already used by another app or by macOS, or the system refused it. Pick another combination.`,
        ...(status.current ? [`${status.current} still opens Quick Capture.`] : [])];
    case "rejected":
      return [problemText(status.problem),
        ...(status.current ? [`${status.current} still opens Quick Capture.`] : [])];
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
        <p>⌘N still creates a task inside Tasknotes.</p>
      </div>
    );
  }
  if (recording) return null;
  return (
    <p className="settings-hint">
      {view.rememberedLabel
        ? `Off. ${view.rememberedLabel} is kept for when you turn it back on.`
        : "Off. ⌘N creates a task inside Tasknotes."}
    </p>
  );
}
