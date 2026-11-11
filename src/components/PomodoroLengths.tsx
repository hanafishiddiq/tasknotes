import { useEffect, useRef, useState, type FormEvent } from "react";
import { Minus, Plus } from "lucide-react";
import type { DurationLimits, PomodoroDurations } from "../lib/pomodoro-api";
import type { PomodoroControl } from "../lib/use-pomodoro";
import PomodoroSheet from "./PomodoroSheet";

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

interface PomodoroLengthsProps {
  anchor: HTMLElement;
  pomodoro: PomodoroControl;
  onClose: (refocus: boolean) => void;
}

/**
 * The Pomodoro lengths and Silent (batch 3, Q27 and Q39), in a sheet behind
 * the icon at the top of the page. The shell keeps both and sends the limits;
 * a new length applies from the next focus or break. Lengths wait for Save;
 * Silent applies at once, as it did in Settings.
 */
export default function PomodoroLengths({ anchor, pomodoro, onClose }: PomodoroLengthsProps) {
  const saved = pomodoro.view?.durations ?? null;
  const limits = pomodoro.view?.limits ?? null;
  const [draft, setDraft] = useState<PomodoroDurations | null>(saved);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [silentError, setSilentError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => { first.current?.focus(); }, []);

  if (!saved || !draft || !limits || !pomodoro.view) return null;
  const silent = pomodoro.view.silent;
  const changed = FIELDS.some(({ key }) => draft[key] !== saved[key]);
  const valid = FIELDS.every(({ key }) => Number.isInteger(draft[key])
    && draft[key] >= limits[key].min && draft[key] <= limits[key].max);

  const set = (key: keyof PomodoroDurations, value: number) => {
    setRefusal(null);
    setDraft({ ...draft, [key]: value });
  };

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!draft || !changed || !valid) return;
    setSaving(true);
    const answer = await pomodoro.setDurations(draft);
    setSaving(false);
    if (answer) setRefusal(answer);
    else onClose(true);
  }

  return (
    <PomodoroSheet anchor={anchor} align="end" label="Pomodoro lengths" className="pomo-lengths" onClose={onClose}>
      <form onSubmit={(event) => void save(event)}>
        {FIELDS.map(({ key, label, unit }, index) => {
          const value = draft[key];
          const { min, max } = limits[key];
          return (
            <div key={key} className="pomo-length">
              <label htmlFor={`pomo-length-${key}`}>{label}</label>
              <span className="pomo-stepper">
                <button type="button" aria-label={`Less ${label.toLowerCase()}`} tabIndex={-1}
                  disabled={!Number.isInteger(value) || value <= min} onClick={() => set(key, value - 1)}>
                  <Minus size={12} aria-hidden="true" />
                </button>
                <input id={`pomo-length-${key}`} ref={index === 0 ? first : undefined} type="number"
                  inputMode="numeric" min={min} max={max} step={1}
                  value={Number.isNaN(value) ? "" : value}
                  onChange={(event) => set(key, event.target.valueAsNumber)} />
                <button type="button" aria-label={`More ${label.toLowerCase()}`} tabIndex={-1}
                  disabled={!Number.isInteger(value) || value >= max} onClick={() => set(key, value + 1)}>
                  <Plus size={12} aria-hidden="true" />
                </button>
              </span>
              <span className="pomo-length-unit">{unit}</span>
            </div>
          );
        })}
        {changed && !valid && <p className="pomo-lengths-error">{limitsHint(limits)}</p>}
        {refusal && <p className="pomo-lengths-error" role="alert">{refusal}</p>}
        <div className="task-menu-separator" role="separator" />
        <label className="pomo-silent">
          <input type="checkbox" checked={silent}
            onChange={async (event) => setSilentError(await pomodoro.setSilent(event.target.checked))} />
          <span>Silent notifications</span>
        </label>
        {silentError && <p className="pomo-lengths-error" role="alert">{silentError}</p>}
        {changed && (
          <div className="pomo-lengths-actions">
            <button type="button" className="button button-small" onClick={() => onClose(true)}>Cancel</button>
            <button type="submit" className="button button-small button-primary" disabled={!valid || saving}>
              {saving ? "Saving..." : "Save"}
            </button>
          </div>
        )}
      </form>
    </PomodoroSheet>
  );
}
