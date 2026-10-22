import "@testing-library/jest-dom/vitest";

// jsdom gaps that the Lexical Note editor relies on. Without getTargetRanges
// Lexical ignores beforeinput, so typed text would never reach the editor.
(InputEvent.prototype as unknown as { getTargetRanges: () => StaticRange[] }).getTargetRanges = () => [];
Range.prototype.getBoundingClientRect = () => new DOMRect();
Range.prototype.getClientRects = () =>
  ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
