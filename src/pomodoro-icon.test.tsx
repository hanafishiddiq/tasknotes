import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  window.localStorage.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command) => {
    if (command === "list_lists" || command === "list_tasks") return { items: [], nextCursor: null };
    return null;
  });
});
afterEach(() => cleanup());

describe("the Pomodoro place in the sidebar", () => {
  // The page holds the whole timer now, not only its history (owner, 2026-11-17),
  // so it shows the same stopwatch as the timer block above it.
  it("shows a stopwatch, not a history clock", async () => {
    render(<App />);
    const place = await screen.findByRole("button", { name: "Pomodoro" });
    expect(place.querySelector("svg.lucide-timer")).not.toBeNull();
    expect(place.querySelector("svg.lucide-history")).toBeNull();
  });
});
