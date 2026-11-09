import type { NewTaskToastState } from "../lib/use-new-task-toast";

interface NewTaskToastProps {
  toast: NewTaskToastState;
  /** The toast was clicked: show and open this Task. */
  onOpen: (taskId: string) => void;
}

/**
 * The quiet bottom-right toast after a Task is added. The live region is always
 * in the page, so each new toast is announced politely; the toast never takes focus.
 */
export default function NewTaskToast({ toast, onOpen }: NewTaskToastProps) {
  return (
    <div className="toast-region" aria-live="polite">
      {toast.taskId && (
        <button key={toast.seq} type="button"
          className={`task-toast ${toast.leaving ? "task-toast--leaving" : ""}`}
          onMouseEnter={() => toast.hold("hover", true)}
          onMouseLeave={() => toast.hold("hover", false)}
          onFocus={() => toast.hold("focus", true)}
          onBlur={() => toast.hold("focus", false)}
          onClick={() => {
            const taskId = toast.open();
            if (taskId) onOpen(taskId);
          }}>
          Task added · <span className="task-toast-action">Click to edit</span>
        </button>
      )}
    </div>
  );
}
