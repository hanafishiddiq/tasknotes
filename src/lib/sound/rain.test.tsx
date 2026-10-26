import { describe, expect, it } from "vitest";
import { renderRain, renderRainInSlices } from "./rain";

const RATE = 16_000; // A low rate keeps the test fast; the shape does not depend on it.

function rms(samples: Float32Array): number {
  let sum = 0;
  for (const value of samples) sum += value * value;
  return Math.sqrt(sum / samples.length);
}

describe("synthesized soft rain", () => {
  it("renders the same loop for the same seed, and a different one for another seed", () => {
    const a = renderRain(RATE, 2, 7);
    const b = renderRain(RATE, 2, 7);
    const c = renderRain(RATE, 2, 8);
    expect(a.length).toBe(2);
    expect(a[0]).toHaveLength(RATE * 2);
    expect(Array.from(a[0].slice(0, 64))).toEqual(Array.from(b[0].slice(0, 64)));
    expect(Array.from(a[0].slice(0, 64))).not.toEqual(Array.from(c[0].slice(0, 64)));
  });

  it("is a gentle, bounded signal with no silence or clipping", () => {
    for (const channel of renderRain(RATE, 3, 1)) {
      expect(channel.every(Number.isFinite)).toBe(true);
      expect(Math.max(...channel.map(Math.abs))).toBeLessThanOrEqual(0.9);
      const level = rms(channel);
      expect(level).toBeGreaterThan(0.08);
      expect(level).toBeLessThan(0.3);
    }
  });

  it("loops without a click: across seeds, the wrap-around jump stays an ordinary step", () => {
    // Two unrelated points of this noise differ by more than the 99th-percentile step
    // about 45% of the time, so an un-blended seam would fail here across 12 channels.
    for (const seed of [1, 2, 3, 4, 5, 6]) {
      for (const channel of renderRain(RATE, 3, seed)) {
        const steps: number[] = [];
        for (let i = 1; i < channel.length; i += 1) steps.push(Math.abs(channel[i] - channel[i - 1]));
        steps.sort((a, b) => a - b);
        const p99 = steps[Math.floor(steps.length * 0.99)];
        expect(Math.abs(channel[0] - channel[channel.length - 1])).toBeLessThan(p99);
      }
    }
  });

  it("keeps its energy low in the treble, like rain heard through a window", () => {
    const [left] = renderRain(RATE, 3, 5);
    // The sample-to-sample difference carries the treble; white noise would be about √2 × RMS.
    let diff = 0;
    for (let i = 1; i < left.length; i += 1) diff += (left[i] - left[i - 1]) ** 2;
    const trebleRatio = Math.sqrt(diff / (left.length - 1)) / rms(left);
    expect(trebleRatio).toBeLessThan(0.9);
  });
});

describe("rain rendered in slices", () => {
  it("gives the same samples as the one-go render, pausing often so no single stretch is long", async () => {
    let pauses = 0;
    const sliced = await renderRainInSlices(RATE, 6, 7, async () => { pauses += 1; });
    const whole = renderRain(RATE, 6, 7);
    expect(sliced).toHaveLength(2);
    sliced.forEach((channel, index) => expect(channel).toEqual(whole[index]));
    // 6 s at 16 kHz is about 100,000 samples a channel: several slices each, not one stretch.
    expect(pauses).toBeGreaterThan(8);
  });
});
