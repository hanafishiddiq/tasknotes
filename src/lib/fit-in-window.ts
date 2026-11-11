/**
 * Where a sheet of this size fits in the window, as near the asked point as
 * it can, keeping `edge` pixels clear of each side: the Task menu and the
 * Pomodoro page's sheets are placed with it.
 */
export function fitInWindow(x: number, y: number, width: number, height: number, edge: number) {
  return {
    left: Math.max(edge, Math.min(x, window.innerWidth - width - edge)),
    top: Math.max(edge, Math.min(y, window.innerHeight - height - edge)),
  };
}
