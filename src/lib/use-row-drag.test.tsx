import { describe, expect, it } from "vitest";
import { anchorFor } from "./use-row-drag";

describe("anchorFor", () => {
  const siblings = ["a", "b", "c"];

  it("names the sibling to sit after, or null for first", () => {
    expect(anchorFor(siblings, "c", "b", "before")).toBe("a");
    expect(anchorFor(siblings, "c", "a", "before")).toBeNull();
    expect(anchorFor(siblings, "a", "b", "after")).toBe("b");
  });

  it("refuses a target that is not among the loaded siblings instead of moving the row first", () => {
    expect(anchorFor(siblings, "a", "elsewhere", "before")).toBeUndefined();
    expect(anchorFor(siblings, "a", "elsewhere", "after")).toBeUndefined();
  });
});
