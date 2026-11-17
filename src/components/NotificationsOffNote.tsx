import { BellOff } from "lucide-react";
import type { PomodoroControl } from "../lib/use-pomodoro";

/**
 * Pomodoro runs with notifications off, but cannot say when a focus or break
 * ends; this says so wherever the timer is, until they are turned on.
 */
export default function NotificationsOffNote({ pomodoro }: { pomodoro: PomodoroControl }) {
  if (!pomodoro.notificationsOff) return null;
  return (
    <div className="pomodoro-tip notifications-off" role="note" aria-label="Notifications are off">
      <p>
        <BellOff size={14} aria-hidden="true" />
        Notifications are off, so Pomodoro won't tell you when a focus or break ends.
      </p>
      <div className="pomodoro-actions">
        <button type="button" className="button button-small"
          onClick={() => void pomodoro.turnOnNotifications()}>Turn on notifications</button>
      </div>
    </div>
  );
}
