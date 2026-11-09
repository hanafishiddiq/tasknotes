import { useEffect, useState } from "react";
import { Timer } from "lucide-react";
import { shownTime, type PomodoroView } from "../lib/pomodoro-api";
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

/** The shell's clock now, ticking once a second while time is counted. */
function useShellNow(view: PomodoroView | null, skew: number): number {
  const counting = view?.phase === "focus" || view?.phase === "break" || view?.phase === "overtime";
  const [now, setNow] = useState(() => Date.now() + skew);
  useEffect(() => {
    setNow(Date.now() + skew);
    if (!counting) return;
    const timer = window.setInterval(() => setNow(Date.now() + skew), 1000);
    return () => window.clearInterval(timer);
  }, [counting, skew, view]);
  return now;
}

function statusText(view: PomodoroView, taskTitle: string | null): string {
  const forTask = taskTitle ? ` · ${taskTitle}` : "";
  switch (view.phase) {
    case "idle": return "Ready to focus";
    case "focus": return `Focus${forTask}`;
    case "focus_done": return `Focus done${forTask}`;
    case "overtime": return `Overtime${forTask}`;
    case "break": return view.long ? "Long break" : "Break";
    case "break_done": return "Break over";
  }
}

/**
 * A compact Pomodoro control at the foot of the sidebar: start with or
 * without a Task, then the choices the timer waits for. The timer runs in
 * the shell; every switch waits for a click here, in the menu bar or on the
 * notification.
 */
export default function PomodoroControl({ pomodoro, tasks, openTaskId }: PomodoroControlProps) {
  const { view, skew, error, tip, run } = pomodoro;
  const now = useShellNow(view, skew);
  const unfinished = tasks.filter((task) => task.status !== "completed");
  const [choice, setChoice] = useState<string | null>(null);
  if (!view) return null;

  const offered = choice ?? (openTaskId && unfinished.some((task) => task.id === openTaskId) ? openTaskId : NO_TASK);
  const sessionTask = "taskId" in view ? view.taskId : null;
  const taskTitle = sessionTask
    ? tasks.find((task) => task.id === sessionTask)?.title ?? "a Task"
    : null;
  const time = shownTime(view, now);
  const nextTask = view.lastTaskId && tasks.some((task) => task.id === view.lastTaskId) ? view.lastTaskId : null;

  return (
    <section className="pomodoro" aria-label="Pomodoro" data-phase={view.phase}>
      <div className="pomodoro-head">
        <Timer size={16} aria-hidden="true" />
        <span className="pomodoro-name">Pomodoro</span>
        {time && <span className="pomodoro-time" aria-label={view.phase === "overtime" ? `Overtime ${time}` : `${time} left`}>{time}</span>}
        {(view.phase === "focus_done" || view.phase === "break_done") && <span className="pomodoro-time">Done</span>}
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
          {(view.phase === "focus_done" || view.phase === "overtime") && (
            <button type="button" className="button button-small button-primary"
              onClick={() => void run({ action: "start_break" })}>Start break</button>
          )}
          {view.phase === "focus_done" && (
            <button type="button" className="button button-small"
              onClick={() => void run({ action: "keep_focusing" })}>Keep focusing</button>
          )}
          {(view.phase === "break" || view.phase === "break_done") && (
            <button type="button"
              className={`button button-small${view.phase === "break_done" ? " button-primary" : ""}`}
              onClick={() => void run({ action: "start_focus", taskId: nextTask })}>Start focus</button>
          )}
          <button type="button" className="button button-small"
            onClick={() => void run({ action: "stop" })}>Stop</button>
        </div>
      )}

      {tip && (
        <div className="pomodoro-tip" role="note" aria-label="Notification tip">
          <p>To keep Start break and Keep focusing on screen until you answer, set Tasknotes notifications to Persistent in System Settings.</p>
          <div className="pomodoro-actions">
            <button type="button" className="button button-small"
              onClick={() => void pomodoro.openNotificationSettings()}>Open Notification Settings</button>
            <button type="button" className="button button-small" onClick={pomodoro.dismissTip}>Dismiss</button>
          </div>
        </div>
      )}
      {error && <p className="pomodoro-error" role="alert">{error}</p>}
    </section>
  );
}
