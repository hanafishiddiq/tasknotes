/**
 * The countdown shared by the new Task toast and "Task completed · Undo": the
 * pointer or focus holds it, and letting go always leaves a moment to act.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { RELEASE_GRACE_MS, useHoldTimer } from "./lib/use-hold-timer";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function timer() {
  const expired = vi.fn();
  const hook = renderHook(({ onExpire }) => useHoldTimer(onExpire), { initialProps: { onExpire: expired } });
  return { expired, hook, t: () => hook.result.current };
}

describe("the hold timer", () => {
  it("runs out once after its time, and a new start counts afresh", () => {
    const { expired, t } = timer();
    act(() => t().start(6000));
    act(() => vi.advanceTimersByTime(5999));
    expect(expired).not.toHaveBeenCalled();
    act(() => t().start(6000));
    act(() => vi.advanceTimersByTime(5999));
    expect(expired).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(expired).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(60_000));
    expect(expired).toHaveBeenCalledTimes(1);
  });

  it("stands still while the pointer or focus holds it, then counts on with the time left", () => {
    const { expired, t } = timer();
    act(() => t().start(6000));
    act(() => vi.advanceTimersByTime(4000));
    expect(t().hold("hover", true)).toBe(true);
    act(() => vi.advanceTimersByTime(60_000));
    // Focus takes over before the pointer leaves: still held.
    expect(t().hold("focus", true)).toBe(true);
    expect(t().hold("hover", false)).toBe(true);
    act(() => vi.advanceTimersByTime(60_000));
    expect(expired).not.toHaveBeenCalled();
    expect(t().hold("focus", false)).toBe(false);
    act(() => vi.advanceTimersByTime(1999));
    expect(expired).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(expired).toHaveBeenCalledTimes(1);
  });

  it("leaves at least a moment after letting go, when the time was nearly or already up", () => {
    const { expired, t } = timer();
    act(() => t().start(6000));
    act(() => vi.advanceTimersByTime(5800));
    act(() => { t().hold("hover", true); });
    act(() => { t().hold("hover", false); });
    act(() => vi.advanceTimersByTime(RELEASE_GRACE_MS - 1));
    expect(expired).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(expired).toHaveBeenCalledTimes(1);
    expect(RELEASE_GRACE_MS).toBe(1000);

    // Caught after it ran out (a toast fading out): held, then a moment again.
    expect(t().hold("hover", true)).toBe(true);
    act(() => vi.advanceTimersByTime(60_000));
    expect(expired).toHaveBeenCalledTimes(1);
    act(() => { t().hold("hover", false); });
    act(() => vi.advanceTimersByTime(RELEASE_GRACE_MS));
    expect(expired).toHaveBeenCalledTimes(2);
  });

  it("stops without running out, ignores holds until the next start, and a start lets go of an earlier hold", () => {
    const { expired, t } = timer();
    act(() => t().start(6000));
    act(() => t().stop());
    expect(t().hold("hover", true)).toBe(false);
    act(() => vi.advanceTimersByTime(60_000));
    expect(expired).not.toHaveBeenCalled();

    act(() => t().start(6000));
    act(() => { t().hold("focus", true); });
    act(() => t().start(3000));
    act(() => vi.advanceTimersByTime(3000));
    expect(expired).toHaveBeenCalledTimes(1);
  });

  it("stays the same across renders and calls the latest onExpire", () => {
    const { expired, hook, t } = timer();
    const first = t();
    const later = vi.fn();
    hook.rerender({ onExpire: later });
    expect(t()).toBe(first);
    act(() => t().start(10));
    act(() => vi.advanceTimersByTime(10));
    expect(expired).not.toHaveBeenCalled();
    expect(later).toHaveBeenCalledTimes(1);
  });
});
