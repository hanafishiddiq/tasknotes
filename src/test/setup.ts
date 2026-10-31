import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";
import { tauriEvents } from "./tauri-events";

// Every App test renders the shell's event listeners; tests send events through tauriEvents.
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, handler: Parameters<typeof tauriEvents.listen>[1]) =>
    tauriEvents.listen(name, handler),
}));

// jsdom gaps that the Lexical Note editor relies on. Without getTargetRanges
// Lexical ignores beforeinput, so typed text would never reach the editor.
(InputEvent.prototype as unknown as { getTargetRanges: () => StaticRange[] }).getTargetRanges = () => [];
Range.prototype.getBoundingClientRect = () => new DOMRect();
Range.prototype.getClientRects = () =>
  ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
