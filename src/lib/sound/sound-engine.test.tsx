import { beforeEach, describe, expect, it } from "vitest";
import { FakeAudioContext, last } from "../../test/fake-audio";
import { createSoundEngine, type SoundEngine } from "./sound-engine";
import { DEFAULT_SOUND_SETTINGS, type SoundSettings } from "./sound-settings";

const quietRain = async (rate: number) => [new Float32Array(rate / 10), new Float32Array(rate / 10)];

function engine(): SoundEngine {
  return createSoundEngine({
    createContext: () => new FakeAudioContext() as unknown as AudioContext,
    renderAmbient: quietRain,
    wait: async () => undefined,
  });
}

/** Lets resume() promises settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function ambientNow(context: FakeAudioContext): number {
  const [source] = context.playingSources();
  return source ? context.levelOf(source) : 0;
}

beforeEach(() => FakeAudioContext.reset());

describe("ambient rain", () => {
  it("starts by default at 8%, rising gently from silence rather than starting loud", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    expect(source.loop).toBe(true);
    expect(ambientNow(context)).toBeCloseTo(0.08);
    const changes = context.changesAfter(source);
    expect(changes[0]).toMatchObject({ type: "set", value: 0 });
    const ramp = changes.find((change) => change.type === "ramp");
    expect(ramp).toMatchObject({ value: 0.08 });
    expect(ramp!.time - changes[0].time).toBeGreaterThanOrEqual(1);
    expect(sound.status()).toBe("playing");
  });

  it("waits silently while autoplay is blocked, then fades in from silence on the first gesture", async () => {
    FakeAudioContext.initialState = "suspended";
    FakeAudioContext.resumePolicy = "block";
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    expect(context.playingSources()).toHaveLength(0);
    expect(sound.status()).toBe("waiting-for-gesture");

    context.currentTime = 30;
    FakeAudioContext.resumePolicy = "allow";
    sound.gesture();
    await settle();
    const [source] = context.playingSources();
    const changes = context.changesAfter(source);
    // No ramp was scheduled while blocked, so nothing jumps in at full level later.
    expect(changes.filter((change) => change.type === "ramp").every((ramp) => ramp.time > 30)).toBe(true);
    expect(changes).toContainEqual({ type: "set", value: 0, time: 30 });
    expect(ambientNow(context)).toBeCloseTo(0.08);
    expect(sound.status()).toBe("playing");
  });
});

function withChannel(channel: keyof SoundSettings, change: Partial<SoundSettings["ambient"]>,
  from: SoundSettings = DEFAULT_SOUND_SETTINGS): SoundSettings {
  return { ...from, [channel]: { ...from[channel], ...change } };
}

describe("rain preparation", () => {
  it("renders the rain before the first gesture, so the first click only has to start it", async () => {
    FakeAudioContext.initialState = "suspended";
    FakeAudioContext.resumePolicy = "block";
    let renders = 0;
    const sound = createSoundEngine({
      createContext: () => new FakeAudioContext() as unknown as AudioContext,
      renderAmbient: async (rate) => { renders += 1; return quietRain(rate); },
      wait: async () => undefined,
    });
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    expect(renders).toBe(1);
    FakeAudioContext.resumePolicy = "allow";
    sound.gesture();
    // Playing in the same turn as the click: nothing was left to render.
    expect(renders).toBe(1);
    await settle();
    expect(FakeAudioContext.latest().playingSources()).toHaveLength(1);
    expect(renders).toBe(1);
  });

  it("starts the rain as soon as a slow render finishes, without blocking in between", async () => {
    let finish: (channels: Float32Array<ArrayBuffer>[]) => void = () => undefined;
    const sound = createSoundEngine({
      createContext: () => new FakeAudioContext() as unknown as AudioContext,
      renderAmbient: () => new Promise((resolve) => { finish = resolve; }),
      wait: async () => undefined,
    });
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    expect(context.playingSources()).toHaveLength(0);
    finish(await quietRain(context.sampleRate));
    await settle();
    expect(ambientNow(context)).toBeCloseTo(0.08);
  });

  it("does not render the rain while the ambient channel is muted", async () => {
    let renders = 0;
    const sound = createSoundEngine({
      createContext: () => new FakeAudioContext() as unknown as AudioContext,
      renderAmbient: async (rate) => { renders += 1; return quietRain(rate); },
      wait: async () => undefined,
    });
    sound.apply(withChannel("ambient", { muted: true }));
    await settle();
    expect(renders).toBe(0);
  });
});

describe("independent volume and mute", () => {
  it("ramps the interaction channel instead of jumping, so muting mid-cue does not click", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    sound.play("complete");
    const [tone] = context.oscillators;
    const gain = last(context.gainsAfter(tone))!.gain;
    context.currentTime = 4;
    sound.apply(withChannel("interaction", { muted: true }));
    const after = gain.events.filter((event) => event.time >= 4);
    expect(after.some((event) => event.type === "set" && event.value === 0)).toBe(false);
    const ramp = last(gain.events)!;
    expect(ramp).toMatchObject({ type: "ramp", value: 0 });
    expect(ramp.time - 4).toBeGreaterThan(0);
    expect(ramp.time - 4).toBeLessThanOrEqual(0.1);
  });

  it("mutes the rain within a fraction of a second and stops the loop", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    context.currentTime = 10;
    sound.apply(withChannel("ambient", { muted: true }));
    const ramp = last(context.changesAfter(source))!;
    expect(ramp).toMatchObject({ type: "ramp", value: 0 });
    expect(ramp.time - 10).toBeLessThanOrEqual(0.1);
    expect(source.stoppedAt).toBeLessThanOrEqual(10.1);
  });

  it("changes the rain's volume without restarting it, and leaves interaction sound alone", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    sound.apply(withChannel("ambient", { volume: 30 }));
    expect(context.playingSources()).toEqual([source]);
    expect(ambientNow(context)).toBeCloseTo(0.3);
    sound.play("create");
    const [tone] = context.oscillators;
    // The cue's envelope peaks at 1, so its level is the interaction channel's.
    expect(Math.max(...context.changesAfter(tone).map((change) => change.value ?? 0))).toBeLessThanOrEqual(1);
    expect(last(context.gainsAfter(tone))!.gain.value).toBeCloseTo(0.12);
  });

  it("plays a short, soft cue at 12% by default, and nothing when interaction sound is muted", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    sound.play("complete");
    expect(context.oscillators.length).toBeGreaterThan(0);
    for (const tone of context.oscillators) {
      expect(tone.stoppedAt! - tone.startedAt!).toBeLessThanOrEqual(0.5);
      expect(last(context.gainsAfter(tone))!.gain.value).toBeCloseTo(0.12);
    }
    const before = context.oscillators.length;
    sound.apply(withChannel("interaction", { muted: true }));
    sound.play("complete");
    expect(context.oscillators).toHaveLength(before);
  });

  it("drops a cue rather than delaying it while autoplay is blocked", async () => {
    FakeAudioContext.initialState = "suspended";
    FakeAudioContext.resumePolicy = "block";
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    sound.play("complete");
    FakeAudioContext.resumePolicy = "allow";
    sound.gesture();
    await settle();
    expect(FakeAudioContext.latest().oscillators).toHaveLength(0);
  });

  it("suspends audio entirely when both channels are muted, so it costs no CPU", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    sound.apply({ interaction: { volume: 12, muted: true }, ambient: { volume: 8, muted: true } });
    await settle();
    expect(context.state).toBe("suspended");
    expect(context.playingSources()).toHaveLength(0);
    expect(sound.status()).toBe("off");

    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    expect(context.state).toBe("running");
    expect(ambientNow(context)).toBeCloseTo(0.08);
  });
});

describe("status", () => {
  it("does not flash the waiting hint when sound comes back after the engine's own suspend", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    sound.apply({ interaction: { volume: 12, muted: true }, ambient: { volume: 8, muted: true } });
    await settle();
    const seen: string[] = [];
    sound.onStatus(() => seen.push(sound.status()));
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    expect(sound.status()).toBe("playing");
    expect(seen).not.toContain("waiting-for-gesture");
  });
});

describe("app lifecycle", () => {
  it("keeps the rain playing while the window is minimized or hidden to the menu bar", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    // Neither a minimized window nor the menu bar tells the engine anything: no reason to stop.
    sound.gesture();
    expect(context.playingSources()).toEqual([source]);
    expect(ambientNow(context)).toBeCloseTo(0.08);
  });

  it("fades the rain out before quitting, and brings it back if the app stays open", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    const [source] = context.playingSources();
    context.currentTime = 5;
    await sound.fadeOut();
    expect(source.stoppedAt).toBeGreaterThan(5);
    expect(source.stoppedAt).toBeLessThanOrEqual(5.5);
    expect(context.playingSources()).toHaveLength(0);

    // A Note that could not save keeps the app open; the next click restores the rain.
    sound.gesture();
    await settle();
    expect(ambientNow(context)).toBeCloseTo(0.08);
  });

  it("resumes after the system interrupts audio, once the person is back", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const context = FakeAudioContext.latest();
    FakeAudioContext.resumePolicy = "block";
    context.setState("interrupted");
    await settle();
    expect(sound.status()).toBe("waiting-for-gesture");
    FakeAudioContext.resumePolicy = "allow";
    sound.gesture();
    await settle();
    expect(sound.status()).toBe("playing");
    expect(context.playingSources()).toHaveLength(1);
  });

  it("reports unavailable audio and otherwise does nothing", async () => {
    const sound = createSoundEngine({
      createContext: () => { throw new Error("no Web Audio here"); },
      renderAmbient: quietRain,
    });
    sound.apply(DEFAULT_SOUND_SETTINGS);
    sound.gesture();
    sound.play("complete");
    await sound.fadeOut();
    expect(sound.status()).toBe("unavailable");
  });

  it("closes its audio when disposed, and can start again afterwards", async () => {
    const sound = engine();
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    const first = FakeAudioContext.latest();
    sound.dispose();
    expect(first.state).toBe("closed");
    sound.apply(DEFAULT_SOUND_SETTINGS);
    await settle();
    expect(FakeAudioContext.latest()).not.toBe(first);
    expect(ambientNow(FakeAudioContext.latest())).toBeCloseTo(0.08);
  });
});
