import { useEffect, useRef, useState } from "react";
import { ChevronRight, SlidersHorizontal, Square } from "lucide-react";
import { clock, type PomodoroCommand, type PomodoroView } from "../lib/pomodoro-api";
import { taskApi, type TaskList } from "../lib/task-api";
import type { PomodoroControl } from "../lib/use-pomodoro";
import { useLocalCalendarDay } from "../lib/use-local-calendar-day";
import { usePomodoroHistory } from "../lib/use-pomodoro-history";
import PomodoroDayRail from "./PomodoroDayRail";
import PomodoroLengths from "./PomodoroLengths";
import PomodoroRecord from "./PomodoroRecord";
import PomodoroTaskPicker from "./PomodoroTaskPicker";

/** The ring's box and line, in CSS pixels; the knob is the line's width plus two each side. */
const RING = 288;
const LINE = 4;
const RADIUS = RING / 2 - 8;
const LAP_RADIUS = RADIUS - 12;
const CIRCLE = 2 * Math.PI * RADIUS;
const LAP = 2 * Math.PI * LAP_RADIUS;

type Phase = PomodoroView["phase"];
const RUNNING_FOCUS: Phase[] = ["focus", "focus_done"];

/** The one word under the time. */
function stateWord(view: PomodoroView): string {
  switch (view.phase) {
    case "idle": return "Ready";
    case "focus": return view.paused ? "Paused" : "Focus";
    case "focus_done": return "Overtime";
    case "break": return view.paused ? "Paused" : view.long ? "Long break" : "Break";
    case "break_done": return "Break over";
  }
}

/** The time in the ring and what screen readers hear for it. */
function shownTime(view: PomodoroView): { text: string; label: string } {
  switch (view.phase) {
    case "idle": {
      const minutes = view.durations.focusMinutes;
      return { text: clock(minutes * 60), label: `${minutes} minute focus` };
    }
    case "focus":
    case "break":
      return { text: clock(view.remainingSeconds), label: `${clock(view.remainingSeconds)} left` };
    case "focus_done":
      return { text: `+${clock(view.overtimeSeconds)}`, label: `Overtime ${clock(view.overtimeSeconds)}` };
    case "break_done":
      return { text: "00:00", label: "Break over" };
  }
}

/** How much of the focus or break is spent, 0 to 1, and the Overtime's own lap. */
function spent(view: PomodoroView): { arc: number; lap: number } {
  const planned = view.plannedSeconds;
  switch (view.phase) {
    case "focus":
    case "break":
      return { arc: planned ? Math.min(1, Math.max(0, 1 - view.remainingSeconds / planned)) : 0, lap: 0 };
    case "focus_done":
      return { arc: 1, lap: planned ? Math.min(1, view.overtimeSeconds / planned) : 0 };
    case "break_done":
      return { arc: 1, lap: 0 };
    default:
      return { arc: 0, lap: 0 };
  }
}

/** Fills as the focus is spent; a second, inner lap draws the Overtime. */
function Ring({ view }: { view: PomodoroView }) {
  const { arc, lap } = spent(view);
  const centre = RING / 2;
  const turn = `rotate(-90 ${centre} ${centre})`;
  const knob = (view.phase === "focus" || view.phase === "break") && arc > 0;
  const time = shownTime(view);
  return (
    <div className="pomo-ring">
      <svg width={RING} height={RING} viewBox={`0 0 ${RING} ${RING}`} aria-hidden="true">
        <circle className="pomo-ring-track" cx={centre} cy={centre} r={RADIUS} strokeWidth={LINE} />
        {/* A round cap on an empty line still draws a dot, so an empty arc is left out. */}
        {arc > 0 && (
          <circle className="pomo-ring-arc" cx={centre} cy={centre} r={RADIUS} strokeWidth={LINE}
            transform={turn} strokeDasharray={`${CIRCLE * arc} ${CIRCLE}`} />
        )}
        {lap > 0 && (
          <circle className="pomo-ring-lap" cx={centre} cy={centre} r={LAP_RADIUS} strokeWidth={2}
            transform={turn} strokeDasharray={`${LAP * lap} ${LAP}`} />
        )}
        {knob && (
          <circle className="pomo-ring-knob" cx={centre} cy={centre - RADIUS} r={LINE + 2}
            transform={`rotate(${arc * 360} ${centre} ${centre})`} />
        )}
      </svg>
      <div className="pomo-ring-face">
        <div className="pomo-clock" role="timer" aria-label={time.label}>{time.text}</div>
        <div className="pomo-word">{stateWord(view)}</div>
      </div>
    </div>
  );
}

/** One bead per focus before the long break: done, now, or still to come. */
function Beads({ view }: { view: PomodoroView }) {
  const total = view.durations.sessionsBeforeLongBreak;
  // A long break begins a new cycle in the shell; until it ends, the beads still show the cycle it closed.
  const longBreak = (view.phase === "break" || view.phase === "break_done") && view.long;
  const done = longBreak ? total : Math.min(view.finishedInCycle, total);
  const now = RUNNING_FOCUS.includes(view.phase) && done < total;
  const label = now ? `Session ${done + 1} of ${total} before the long break`
    : `${done} of ${total} sessions before the long break`;
  return (
    <div className="pomo-beads" role="img" aria-label={label} title={label}>
      {Array.from({ length: total }, (_, index) => (
        <span key={index} className="pomo-bead"
          data-state={index < done ? "done" : index === done && now ? "now" : "next"} />
      ))}
    </div>
  );
}

interface Choices {
  main: { label: string; command: PomodoroCommand; waits: boolean };
  second?: { label: string; command: PomodoroCommand };
  stop: boolean;
}

/** The buttons the timer waits for (Q40): yellow only while it waits for the person. */
function choices(view: PomodoroView, taskId: string | null): Choices {
  switch (view.phase) {
    case "idle":
      return { main: { label: "Start", command: { action: "start_focus", taskId }, waits: true }, stop: false };
    case "focus":
      return {
        main: view.paused
          ? { label: "Resume", command: { action: "resume" }, waits: true }
          : { label: "Pause", command: { action: "pause" }, waits: false },
        second: { label: "Start break", command: { action: "break_early" } },
        stop: true,
      };
    case "focus_done":
      return {
        main: { label: "Start break", command: { action: "start_break" }, waits: true },
        second: { label: `Count +${clock(view.overtimeSeconds)}`, command: { action: "count_overtime" } },
        stop: true,
      };
    case "break":
      return {
        main: view.paused
          ? { label: "Resume", command: { action: "resume" }, waits: true }
          : { label: "Pause", command: { action: "pause" }, waits: false },
        second: { label: "Skip to focus", command: { action: "start_next_focus" } },
        stop: true,
      };
    case "break_done":
      return { main: { label: "Start focus", command: { action: "start_next_focus" }, waits: true }, stop: true };
  }
}

/** The title of the Task a focus is for, read from the core when it is not known yet. */
function useTaskTitle(taskId: string | null, known: Map<string, string>): string | null {
  const [titles, setTitles] = useState<Map<string, string>>(new Map());
  const title = taskId === null ? null : known.get(taskId) ?? titles.get(taskId) ?? null;
  useEffect(() => {
    if (taskId === null || title !== null) return;
    let current = true;
    taskApi.get(taskId).then((task) => {
      if (current && task) setTitles((before) => new Map(before).set(task.id, task.title));
    }).catch(() => {});
    return () => { current = false; };
  }, [taskId, title]);
  return title;
}

interface PomodoroPageProps {
  hidden: boolean;
  /** Read the history again whenever this changes: a session was stored, or something changed outside. */
  refreshKey: number;
  pomodoro: PomodoroControl;
  lists: TaskList[];
  /** Titles the window already holds, by Task id. */
  knownTitles: Map<string, string>;
  onOpenTask: (taskId: string) => void;
}

/**
 * The Pomodoro page (batch 3, Ticket 04, layout A). The first screen is the
 * timer alone: the Task line, the ring with the time, the cycle beads and
 * the buttons the timer waits for; today's day rail sits under it and the
 * focus record below. The timer runs in the shell, which sends what to show;
 * the page keeps no clock or rule of its own and only sends clicks.
 */
export default function PomodoroPage({ hidden, refreshKey, pomodoro, lists, knownTitles, onOpenTask }: PomodoroPageProps) {
  const { view, error, run } = pomodoro;
  const today = useLocalCalendarDay();
  const history = usePomodoroHistory(hidden, refreshKey);
  const [chosen, setChosen] = useState<string | null | undefined>(undefined);
  const [picked, setPicked] = useState<Map<string, string>>(new Map());
  const [sheet, setSheet] = useState<"picker" | "lengths" | null>(null);
  const taskLine = useRef<HTMLButtonElement>(null);
  const lengthsButton = useRef<HTMLButtonElement>(null);

  const titles = new Map([...knownTitles, ...picked]);
  // Until the person chooses, the next focus is for the last one's Task.
  const nextTask = chosen === undefined ? view?.lastTaskId ?? null : chosen;
  const sessionTask = view && "taskId" in view ? view.taskId
    : view && (view.phase === "break" || view.phase === "break_done") ? view.lastTaskId : nextTask;
  const title = useTaskTitle(sessionTask, titles);

  // A sheet never outlives the page or the state it was opened in.
  useEffect(() => { if (hidden) setSheet(null); }, [hidden]);
  const idle = view?.phase === "idle";
  useEffect(() => { if (!idle) setSheet((open) => (open === "picker" ? null : open)); }, [idle]);

  const todayDay = history.days[0]?.date === today ? history.days[0] : undefined;
  const record = <PomodoroRecord history={history} today={today} onOpenTask={onOpenTask} />;
  // Without the shell's timer (or before it answers) the page is the record alone.
  if (!view) {
    return (
      <main className="main-view pomo-page" hidden={hidden} aria-label="Pomodoro">
        <h1 className="sr-only">Pomodoro</h1>
        <div className="main-scroll">{record}</div>
      </main>
    );
  }

  const offer = choices(view, nextTask);
  const closeSheet = (refocus: boolean) => {
    const opener = sheet === "picker" ? taskLine.current : lengthsButton.current;
    setSheet(null);
    if (refocus) opener?.focus();
  };

  let taskText: string;
  if (view.phase === "idle") taskText = title ?? "Focus";
  else if (view.phase === "break" || view.phase === "break_done") taskText = `Next: ${title ?? "No Task"}`;
  else taskText = title ?? "No Task";

  return (
    <main className="main-view pomo-page" hidden={hidden} aria-label="Pomodoro" data-phase={view.phase}
      data-paused={"paused" in view && view.paused ? "" : undefined}>
      <h1 className="sr-only">Pomodoro</h1>
      <button ref={lengthsButton} type="button" className="icon-button pomo-lengths-button"
        aria-label="Pomodoro lengths" title="Lengths and Silent" aria-haspopup="dialog"
        aria-expanded={sheet === "lengths"} onClick={() => setSheet(sheet === "lengths" ? null : "lengths")}>
        <SlidersHorizontal size={16} aria-hidden="true" />
      </button>
      <div className="main-scroll">
        <section className="pomo-stage" aria-label="Timer">
          {view.phase === "idle" ? (
            <button ref={taskLine} type="button" className={`pomo-task${title ? " pomo-task--set" : ""}`}
              aria-haspopup="dialog" aria-expanded={sheet === "picker"}
              aria-label={title ? `Focus on “${title}”, choose a Task` : "Focus without a Task, choose a Task"}
              onClick={() => setSheet(sheet === "picker" ? null : "picker")}>
              <span className="pomo-task-title">{taskText}</span>
              <ChevronRight size={14} aria-hidden="true" />
            </button>
          ) : (
            <p className="pomo-task pomo-task--still"><span className="pomo-task-title">{taskText}</span></p>
          )}
          <Ring view={view} />
          <Beads view={view} />
          <div className="pomo-actions">
            <span />
            {/* Keyed by what it does, so a new choice is a new button and never fades from the last one's colour. */}
            <button key={offer.main.label} type="button" className={`pomo-pill${offer.main.waits ? " pomo-pill--go" : ""}`}
              onClick={() => void run(offer.main.command)}>{offer.main.label}</button>
            {offer.stop ? (
              <button type="button" className="pomo-stop" aria-label="Stop" title="Stop"
                onClick={() => void run({ action: "stop" })}>
                <Square size={12} fill="currentColor" strokeWidth={0} aria-hidden="true" />
              </button>
            ) : <span />}
          </div>
          <div className="pomo-second">
            {offer.second && (
              <button type="button" className="pomo-second-button"
                onClick={() => void run(offer.second!.command)}>{offer.second.label}</button>
            )}
          </div>
          {view.saveError && (
            <div className="pomo-unsaved" role="alert">
              <p className="pomo-error">{view.saveError}</p>
              <button type="button" className="button button-small button-primary"
                onClick={() => void run({ action: "retry_save" })}>Try again</button>
              <button type="button" className="button button-small"
                onClick={() => void run({ action: "discard_unsaved" })}>Discard</button>
            </div>
          )}
          {error && <p className="pomo-error" role="alert">{error}</p>}
        </section>
        <PomodoroDayRail day={todayDay} view={view} now={new Date()} />
        {record}
      </div>
      {sheet === "picker" && taskLine.current && (
        <PomodoroTaskPicker anchor={taskLine.current} chosen={nextTask} lists={lists} onClose={closeSheet}
          onChoose={(taskId, chosenTitle) => {
            setChosen(taskId);
            if (taskId && chosenTitle) setPicked((before) => new Map(before).set(taskId, chosenTitle));
          }} />
      )}
      {sheet === "lengths" && lengthsButton.current && (
        <PomodoroLengths anchor={lengthsButton.current} pomodoro={pomodoro} onClose={closeSheet} />
      )}
    </main>
  );
}
