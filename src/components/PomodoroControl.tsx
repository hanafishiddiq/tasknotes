import { useState } from "react";
import { Timer } from "lucide-react";
import { clock, type PomodoroView } from "../lib/pomodoro-api";
import type { PomodoroControl as Control } from "../lib/use-pomodoro";
import type { Task } from "../lib/task-api";

interface PomodoroControlProps {
  pomodoro: Control;
  /** The Tasks on the current page; a focus can be for one of the unfinished ones. */
  tasks: Task[];
  /** The open Task, offered first. */
  openTaskId: string | null;
}

const NO_TASK = "";

function statusText(view: PomodoroView, taskTitle: string | null): string {
  const forTask = taskTitle ? ` · ${taskTitle}` : "";
  switch (view.phase) {
    case "idle": return "Ready to focus";
    case "focus": return `${view.paused ? "Paused" : "Focus"}${forTask}`;
    case "focus_done": return `Focus done${forTask}`;
    case "break": return view.paused ? "Break paused" : view.long ? "Long break" : "Break";
    case "break_done": return "Break over";
  }
}

/** The time as the shell counts it, with what it means for screen readers. */
function shownTime(view: PomodoroView): { text: string; label: string } | null {
  switch (view.phase) {
    case "focus":
    case "break":
      return { text: clock(view.remainingSeconds), label: `${clock(view.remainingSeconds)} left` };
    case "focus_done":
      return { text: `+${clock(view.overtimeSeconds)}`, label: `Overtime ${clock(view.overtimeSeconds)}` };
    case "break_done":
      return { text: "Done", label: "Break done" };
    default:
      return null;
  }
}

/**
 * A compact Pomodoro control at the foot of the sidebar: start with or
 * without a Task, then the choices the timer waits for. The timer runs in
 * the shell and sends what to show; every switch waits for a click here, in
 * the menu bar or on a notification.
 */
export default function PomodoroControl({ pomodoro, tasks, openTaskId }: PomodoroControlProps) {
  const { view, error, tip, run } = pomodoro;
  const unfinished = tasks.filter((task) => task.status !== "completed");
  const [choice, setChoice] = useState<string | null>(null);
  if (!view) return null;

  const offered = choice ?? (openTaskId && unfinished.some((task) => task.id === openTaskId) ? openTaskId : NO_TASK);
  const sessionTask = "taskId" in view ? view.taskId : null;
  const taskTitle = sessionTask
    ? tasks.find((task) => task.id === sessionTask)?.title ?? "a Task"
    : null;
  const time = shownTime(view);

  return (
    <section className="pomodoro" aria-label="Pomodoro" data-phase={view.phase}>
      <div className="pomodoro-head">
        <Timer size={16} aria-hidden="true" />
        <span className="pomodoro-name">Pomodoro</span>
        {time && <span className="pomodoro-time" role="timer" aria-label={time.label}>{time.text}</span>}
      </div>
      <p className="pomodoro-status" role="status">{statusText(view, taskTitle)}</p>

      {view.phase === "idle" && (
        <div className="pomodoro-start">
          <select aria-label="Task for this Pomodoro" value={offered}
            onChange={(event) => setChoice(event.target.value)}>
            <option value={NO_TASK}>No Task</option>
            {unfinished.map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}
          </select>
          <button type="button" className="button button-small button-primary"
            onClick={() => void run({ action: "start_focus", taskId: offered || null })}>
            Start focus
          </button>
        </div>
      )}

      {view.phase !== "idle" && (
        <div className="pomodoro-actions">
          {(view.phase === "focus" || view.phase === "break") && view.paused && (
            <button type="button" className="button button-small button-primary"
              onClick={() => void run({ action: "resume" })}>Resume</button>
          )}
          {view.phase === "focus_done" && (
            <>
              <button type="button" className="button button-small button-primary"
                onClick={() => void run({ action: "start_break" })}>Start break</button>
              <button type="button" className="button button-small"
                onClick={() => void run({ action: "count_overtime" })}>Count +{clock(view.overtimeSeconds)}</button>
            </>
          )}
          {(view.phase === "break" || view.phase === "break_done") && (
            <button type="button"
              className={`button button-small${view.phase === "break_done" ? " button-primary" : ""}`}
              onClick={() => void run({ action: "start_next_focus" })}>Start focus</button>
          )}
          <button type="button" className="button button-small"
            onClick={() => void run({ action: "stop" })}>Stop</button>
        </div>
      )}

      {tip && (
        <div className="pomodoro-tip" role="note" aria-label="Notification tip">
          <p>To keep the Pomodoro buttons on screen until you answer, set Tasknotes notifications to Persistent in System Settings.</p>
          <div className="pomodoro-actions">
            <button type="button" className="button button-small"
              onClick={() => void pomodoro.openNotificationSettings()}>Open Notification Settings</button>
            <button type="button" className="button button-small" onClick={pomodoro.dismissTip}>Dismiss</button>
          </div>
        </div>
      )}
      {view.saveError && (
        <div className="pomodoro-unsaved" role="alert">
          <p className="pomodoro-error">{view.saveError}</p>
          <div className="pomodoro-actions">
            <button type="button" className="button button-small button-primary"
              onClick={() => void run({ action: "retry_save" })}>Try again</button>
            <button type="button" className="button button-small"
              onClick={() => void run({ action: "discard_unsaved" })}>Discard</button>
          </div>
        </div>
      )}
      {error && <p className="pomodoro-error" role="alert">{error}</p>}
    </section>
  );
}
