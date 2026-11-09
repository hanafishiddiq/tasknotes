import { useEffect, useState, type FormEvent } from "react";
import { DURATION_LIMITS, type PomodoroDurations } from "../lib/pomodoro-api";
import type { PomodoroControl } from "../lib/use-pomodoro";

const FIELDS: { key: keyof PomodoroDurations; label: string; unit: string }[] = [
  { key: "focusMinutes", label: "Focus", unit: "min" },
  { key: "shortBreakMinutes", label: "Break", unit: "min" },
  { key: "longBreakMinutes", label: "Long break", unit: "min" },
  { key: "sessionsBeforeLongBreak", label: "Long break after", unit: "sessions" },
];

/** The Pomodoro lengths, kept by the shell; a change applies from the next focus or break. */
export default function PomodoroSettings({ pomodoro }: { pomodoro: PomodoroControl }) {
  const saved = pomodoro.view?.durations ?? null;
  const [draft, setDraft] = useState<PomodoroDurations | null>(saved);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => { setDraft(saved); }, [saved?.focusMinutes, saved?.shortBreakMinutes,
    saved?.longBreakMinutes, saved?.sessionsBeforeLongBreak]);

  if (!saved || !draft) return null;
  const changed = FIELDS.some(({ key }) => draft[key] !== saved[key]);
  const valid = FIELDS.every(({ key }) => Number.isInteger(draft[key])
    && draft[key] >= DURATION_LIMITS[key].min && draft[key] <= DURATION_LIMITS[key].max);

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
            <input type="number" inputMode="numeric" min={DURATION_LIMITS[key].min} max={DURATION_LIMITS[key].max}
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
        {changed && !valid && (
          <p className="settings-hint settings-hint--error">
            Focus 1–180 minutes, breaks 1–60 minutes, a long break after 1–12 sessions.
          </p>
        )}
        {message && (
          <p className={`settings-hint${message.error ? " settings-hint--error" : ""}`}
            role={message.error ? "alert" : "status"}>{message.text}</p>
        )}
        <p className="settings-hint">The focus end shows one notification, silent while interaction sounds are muted. Ambient rain is not affected.</p>
      </form>
    </>
  );
}
