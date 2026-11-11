import type { FocusEvent } from "react";
import { X } from "lucide-react";
import type { NewTaskToastState } from "../lib/use-new-task-toast";

interface NewTaskToastProps {
  toast: NewTaskToastState;
  /** The toast was clicked: show and open this Task. */
  onOpen: (taskId: string) => void;
}

/**
 * The quiet bottom-right toast after a Task is added, with an X that closes it
 * at once. The live region is always in the page, so each new toast is announced
 * politely; the toast never takes focus. The pointer or focus anywhere on it holds it.
 */
export default function NewTaskToast({ toast, onOpen }: NewTaskToastProps) {
  // Focus moving between its two buttons stays on the toast: still held.
  const leftToast = (event: FocusEvent<HTMLDivElement>) =>
    !event.currentTarget.contains(event.relatedTarget as Node | null);
  return (
    <div className="toast-region" aria-live="polite">
      {toast.taskId && (
        <div key={toast.seq}
          className={`task-toast task-toast--new ${toast.leaving ? "task-toast--leaving" : ""}`}
          onMouseEnter={() => toast.hold("hover", true)}
          onMouseLeave={() => toast.hold("hover", false)}
          onFocus={() => toast.hold("focus", true)}
          onBlur={(event) => { if (leftToast(event)) toast.hold("focus", false); }}>
          <button type="button" className="task-toast-open"
            onClick={() => {
              const taskId = toast.open();
              if (taskId) onOpen(taskId);
            }}>
            Task added · <span className="task-toast-action">Click to edit</span>
          </button>
          <button type="button" className="task-toast-close" aria-label="Dismiss" onClick={toast.dismiss}>
            <X size={13} aria-hidden="true" />
          </button>
        </div>
      )}
    </div>
  );
}
