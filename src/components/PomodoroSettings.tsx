import { useEffect, useState, type FormEvent } from "react";
import type { DurationLimits, PomodoroDurations } from "../lib/pomodoro-api";
import type { PomodoroControl } from "../lib/use-pomodoro";

const FIELDS: { key: keyof PomodoroDurations; label: string; unit: string }[] = [
  { key: "focusMinutes", label: "Focus", unit: "min" },
  { key: "shortBreakMinutes", label: "Break", unit: "min" },
  { key: "longBreakMinutes", label: "Long break", unit: "min" },
  { key: "sessionsBeforeLongBreak", label: "Long break after", unit: "sessions" },
];

/** The limits in words, from the numbers the shell sends. */
function limitsHint(limits: DurationLimits): string {
  const range = ({ min, max }: { min: number; max: number }) => `${min}–${max}`;
  const breaks = limits.shortBreakMinutes.max === limits.longBreakMinutes.max
    && limits.shortBreakMinutes.min === limits.longBreakMinutes.min
    ? `breaks ${range(limits.shortBreakMinutes)} minutes`
    : `breaks ${range(limits.shortBreakMinutes)} minutes, long breaks ${range(limits.longBreakMinutes)} minutes`;
  return `Focus ${range(limits.focusMinutes)} minutes, ${breaks}, a long break after ${range(limits.sessionsBeforeLongBreak)} sessions.`;
}

/**
 * The Pomodoro lengths and Silent, kept by the shell; a new length applies
 * from the next focus or break. The limits come from the shell too.
 */
export default function PomodoroSettings({ pomodoro }: { pomodoro: PomodoroControl }) {
  const saved = pomodoro.view?.durations ?? null;
  const limits = pomodoro.view?.limits ?? null;
  const [draft, setDraft] = useState<PomodoroDurations | null>(saved);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [silentError, setSilentError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => { setDraft(saved); }, [saved?.focusMinutes, saved?.shortBreakMinutes,
    saved?.longBreakMinutes, saved?.sessionsBeforeLongBreak]);

  if (!saved || !draft || !limits || !pomodoro.view) return null;
  const silent = pomodoro.view.silent;
  const changed = FIELDS.some(({ key }) => draft[key] !== saved[key]);
  const valid = FIELDS.every(({ key }) => Number.isInteger(draft[key])
    && draft[key] >= limits[key].min && draft[key] <= limits[key].max);

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!draft || !changed || !valid) return;
    setSaving(true);
    const refusal = await pomodoro.setDurations(draft);
    setSaving(false);
    setMessage(refusal ? { text: refusal, error: true } : { text: "Saved. New lengths start with the next focus or break.", error: false });
  }

  return (
    <>
      <h2 className="settings-heading" id="settings-pomodoro">Pomodoro</h2>
      <form className="settings-pomodoro" aria-labelledby="settings-pomodoro" onSubmit={(event) => void save(event)}>
        {FIELDS.map(({ key, label, unit }) => (
          <label key={key} className="settings-pomodoro-row">
            <span>{label}</span>
            <input type="number" inputMode="numeric" min={limits[key].min} max={limits[key].max}
              step={1} value={Number.isNaN(draft[key]) ? "" : draft[key]}
              onChange={(event) => {
                setMessage(null);
                setDraft({ ...draft, [key]: event.target.valueAsNumber });
              }} />
            <span className="settings-pomodoro-unit">{unit}</span>
          </label>
        ))}
        {changed && (
          <button type="submit" className="button button-small button-primary" disabled={!valid || saving}>
            {saving ? "Saving..." : "Save lengths"}
          </button>
        )}
        {changed && !valid && <p className="settings-hint settings-hint--error">{limitsHint(limits)}</p>}
        {message && (
          <p className={`settings-hint${message.error ? " settings-hint--error" : ""}`}
            role={message.error ? "alert" : "status"}>{message.text}</p>
        )}
        <label className="settings-choice">
          <input type="checkbox" checked={silent}
            onChange={async (event) => setSilentError(await pomodoro.setSilent(event.target.checked))} />
          <span>Silent notifications</span>
        </label>
        {silentError && <p className="settings-hint settings-hint--error" role="alert">{silentError}</p>}
        <p className="settings-hint">
          The focus end and the break end each show one notification, without sound when Silent is on.
          Interaction sounds and Ambient rain are separate.
        </p>
      </form>
    </>
  );
}
