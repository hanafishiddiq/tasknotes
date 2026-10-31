/**
 * A stand-in for the WebView's Web Audio, recording what the sound engine asks
 * of it. It renders nothing; it only says what would be heard and when.
 */

/** The last item of a list (the project's ES target predates Array#at). */
export function last<T>(items: readonly T[]): T | undefined {
  return items[items.length - 1];
}

export interface ParamEvent { type: "set" | "ramp" | "cancel"; value?: number; time: number }

export class FakeParam {
  events: ParamEvent[] = [];
  constructor(public value = 1) {}
  setValueAtTime(value: number, time: number) {
    this.events.push({ type: "set", value, time });
    this.value = value;
    return this;
  }
  linearRampToValueAtTime(value: number, time: number) {
    this.events.push({ type: "ramp", value, time });
    this.value = value;
    return this;
  }
  exponentialRampToValueAtTime(value: number, time: number) {
    return this.linearRampToValueAtTime(value, time);
  }
  cancelScheduledValues(time: number) {
    this.events.push({ type: "cancel", time });
    return this;
  }
}

class FakeNode {
  connections: unknown[] = [];
  connect(target: unknown) {
    this.connections.push(target);
    return target;
  }
  disconnect() {
    this.connections = [];
  }
}

export class FakeGain extends FakeNode {
  gain = new FakeParam(1);
}

export class FakeSource extends FakeNode {
  buffer: unknown = null;
  loop = false;
  startedAt: number | null = null;
  stoppedAt: number | null = null;
  start(time = 0) {
    this.startedAt = time;
  }
  stop(time = 0) {
    this.stoppedAt = time;
  }
}

export class FakeOscillator extends FakeSource {
  type = "sine";
  frequency = new FakeParam(440);
}

export class FakeAudioContext {
  static instances: FakeAudioContext[] = [];
  /** What `resume()` does: "allow" starts audio; "block" leaves it suspended, as WebKit does without a gesture. */
  static resumePolicy: "allow" | "block" = "allow";
  /** The state a new context starts in. */
  static initialState: "running" | "suspended" = "running";

  state: "running" | "suspended" | "closed" | "interrupted" = FakeAudioContext.initialState;
  currentTime = 0;
  sampleRate = 48_000;
  destination = { kind: "destination" };
  gains: FakeGain[] = [];
  sources: FakeSource[] = [];
  oscillators: FakeOscillator[] = [];
  resumeCalls = 0;
  private listeners = new Set<() => void>();

  constructor() {
    FakeAudioContext.instances.push(this);
  }

  static reset() {
    FakeAudioContext.instances = [];
    FakeAudioContext.resumePolicy = "allow";
    FakeAudioContext.initialState = "running";
  }

  static latest(): FakeAudioContext {
    const context = last(FakeAudioContext.instances);
    if (!context) throw new Error("No audio context was created");
    return context;
  }

  addEventListener(_: "statechange", listener: () => void) {
    this.listeners.add(listener);
  }
  removeEventListener(_: "statechange", listener: () => void) {
    this.listeners.delete(listener);
  }
  /** The system changing the state on its own, e.g. another app taking the audio device. */
  setState(state: FakeAudioContext["state"]) {
    this.state = state;
    for (const listener of this.listeners) listener();
  }

  async resume() {
    this.resumeCalls += 1;
    if (this.state === "closed") throw new Error("closed");
    if (FakeAudioContext.resumePolicy === "allow" && this.state !== "running") this.setState("running");
  }
  async suspend() {
    if (this.state === "running") this.setState("suspended");
  }
  async close() {
    this.setState("closed");
  }

  createGain() {
    const gain = new FakeGain();
    this.gains.push(gain);
    return gain;
  }
  createBuffer(channels: number, length: number, sampleRate: number) {
    const data = Array.from({ length: channels }, () => new Float32Array(length));
    return {
      numberOfChannels: channels, length, sampleRate,
      getChannelData: (channel: number) => data[channel],
      copyToChannel: (source: Float32Array, channel: number) => data[channel].set(source),
    };
  }
  createBufferSource() {
    const source = new FakeSource();
    this.sources.push(source);
    return source;
  }
  createOscillator() {
    const oscillator = new FakeOscillator();
    this.oscillators.push(oscillator);
    return oscillator;
  }

  /** The gains between a node and the speakers, nearest first. */
  gainsAfter(node: FakeNode): FakeGain[] {
    const path: FakeGain[] = [];
    let current: unknown = node.connections[0];
    while (current instanceof FakeNode) {
      if (current instanceof FakeGain) path.push(current);
      current = current.connections[0];
    }
    return current === this.destination ? path : [];
  }

  /** How loud a node reaches the speakers now (0 when it is not connected to them). */
  levelOf(node: FakeNode): number {
    const path = this.gainsAfter(node);
    return path.length === 0 ? 0 : path.reduce((level, gain) => level * gain.gain.value, 1);
  }

  /** Every gain change on the way from a node to the speakers, in time order. */
  changesAfter(node: FakeNode): ParamEvent[] {
    return this.gainsAfter(node).flatMap((gain) => gain.gain.events).sort((a, b) => a.time - b.time);
  }

  /** Sources still looping: started and not stopped. */
  playingSources(): FakeSource[] {
    return this.sources.filter((source) => source.startedAt !== null && source.stoppedAt === null);
  }
}
