import { renderRainInSlices } from "./rain";
import type { SoundSettings } from "./sound-settings";

/** off: nothing to play; waiting-for-gesture: the WebView blocked autoplay. */
export type SoundStatus = "off" | "waiting-for-gesture" | "playing" | "unavailable";

/** A moment an interaction sound marks. */
export type Cue = "create" | "complete";

export interface SoundEngine {
  apply(settings: SoundSettings): void;
  /**
   * A click or key press: the moment WebKit lets audio start. It also ends a quit
   * fade, for a quit that was held back. A minimized window or one hidden to the
   * menu bar changes nothing: only quitting silences the rain.
   */
  gesture(): void;
  play(cue: Cue): void;
  /** Fades the rain out before the app quits. */
  fadeOut(): Promise<void>;
  status(): SoundStatus;
  onStatus(listener: () => void): () => void;
  dispose(): void;
}

export interface SoundEngineOptions {
  createContext: () => AudioContext;
  /** Renders the rain loop; the engine starts it as soon as the rain is wanted, before any click. */
  renderAmbient?: (sampleRate: number) => Promise<Float32Array<ArrayBuffer>[]>;
  wait?: (ms: number) => Promise<void>;
}

/** Fade from silence whenever the rain starts. */
const FADE_IN_SECONDS = 2;
/** Volume changes and mute: quick enough to feel instant, slow enough not to click. */
const ADJUST_SECONDS = 0.08;
const QUIT_FADE_SECONDS = 0.3;
const RAIN_LOOP_SECONDS = 12;

/** Each cue is one or two soft sine notes with a quick rise and a short decay. */
const CUES: Record<Cue, { hz: number; at: number; seconds: number }[]> = {
  create: [{ hz: 659.25, at: 0, seconds: 0.16 }],
  complete: [{ hz: 523.25, at: 0, seconds: 0.2 }, { hz: 783.99, at: 0.08, seconds: 0.26 }],
};
const CUE_PEAK = 0.7;

const defaultRain = (sampleRate: number) => renderRainInSlices(sampleRate, RAIN_LOOP_SECONDS, 1);
const defaultWait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export function createSoundEngine(options: SoundEngineOptions): SoundEngine {
  const renderAmbient = options.renderAmbient ?? defaultRain;
  const wait = options.wait ?? defaultWait;
  const listeners = new Set<() => void>();
  let settings: SoundSettings | null = null;
  let quitting = false;
  let context: AudioContext | null = null;
  let unavailable = false;
  let ambientGain: GainNode | null = null;
  let interactionGain: GainNode | null = null;
  let rain: AudioBuffer | null = null;
  let rendering = false;
  let renderFailed = false;
  /** Set when the engine itself suspended the context, so waking it is not a blocked autoplay. */
  let suspendedByUs = false;
  let sources: AudioBufferSourceNode[] = [];
  let current: SoundStatus = "off";

  function setStatus(next: SoundStatus) {
    if (next === current) return;
    current = next;
    for (const listener of listeners) listener();
  }

  const running = () => context?.state === "running";
  const fadeSeconds = () => (quitting ? QUIT_FADE_SECONDS : ADJUST_SECONDS);

  function ambientTarget(): number {
    if (!settings || settings.ambient.muted || quitting) return 0;
    return settings.ambient.volume / 100;
  }

  function interactionTarget(): number {
    if (!settings || settings.interaction.muted) return 0;
    return settings.interaction.volume / 100;
  }

  function ensureContext(): AudioContext | null {
    if (context || unavailable) return context;
    try {
      context = options.createContext();
    } catch {
      unavailable = true;
      setStatus("unavailable");
      return null;
    }
    context.addEventListener("statechange", sync);
    ambientGain = context.createGain();
    ambientGain.gain.setValueAtTime(0, context.currentTime);
    ambientGain.connect(context.destination);
    interactionGain = context.createGain();
    interactionGain.gain.setValueAtTime(0, context.currentTime);
    interactionGain.connect(context.destination);
    return context;
  }

  /** Renders the rain off the critical path, so the first click only has to start it. */
  function prepareRain(audio: AudioContext) {
    if (rain || rendering || renderFailed) return;
    rendering = true;
    renderAmbient(audio.sampleRate).then((channels) => {
      if (context !== audio) return;
      const buffer = audio.createBuffer(channels.length, channels[0].length, audio.sampleRate);
      channels.forEach((samples, index) => buffer.copyToChannel(samples, index));
      rain = buffer;
    }, () => { renderFailed = true; }).finally(() => {
      rendering = false;
      sync();
    });
  }

  function startRain(audio: AudioContext, gain: GainNode, target: number, buffer: AudioBuffer) {
    const now = audio.currentTime;
    gain.gain.cancelScheduledValues(now);
    gain.gain.setValueAtTime(0, now);
    gain.gain.linearRampToValueAtTime(target, now + FADE_IN_SECONDS);
    const source = audio.createBufferSource();
    source.buffer = buffer;
    source.loop = true;
    source.connect(gain);
    source.start(now);
    sources = [source];
  }

  function moveTo(audio: AudioContext, gain: GainNode, target: number, seconds: number) {
    const now = audio.currentTime;
    gain.gain.cancelScheduledValues(now);
    gain.gain.setValueAtTime(gain.gain.value, now);
    gain.gain.linearRampToValueAtTime(target, now + seconds);
  }

  function stopRain(audio: AudioContext, gain: GainNode, seconds: number) {
    if (sources.length === 0) return;
    moveTo(audio, gain, 0, seconds);
    for (const source of sources) source.stop(audio.currentTime + seconds);
    sources = [];
  }

  function sync() {
    if (!settings) return;
    const audible = ambientTarget() > 0 || interactionTarget() > 0;
    if (!audible && !context) return setStatus(unavailable ? "unavailable" : "off");
    if (!audible) return quiet();
    const audio = ensureContext();
    if (!audio || audio.state === "closed") return;
    if (ambientTarget() > 0) prepareRain(audio);
    if (!running()) return wake(audio);
    suspendedByUs = false;
    applyLevels(audio);
    setStatus("playing");
  }

  /** The context is not running: ask it to, and say "waiting" only if it stays blocked. */
  function wake(audio: AudioContext) {
    if (!suspendedByUs) setStatus("waiting-for-gesture");
    const stillBlocked = () => { if (!running()) setStatus("waiting-for-gesture"); };
    audio.resume().then(() => (running() ? sync() : stillBlocked()), stillBlocked);
  }

  function applyLevels(audio: AudioContext) {
    if (!ambientGain || !interactionGain) return;
    const ambient = ambientTarget();
    if (ambient > 0 && sources.length === 0) {
      if (rain) startRain(audio, ambientGain, ambient, rain);
    } else if (ambient > 0) moveTo(audio, ambientGain, ambient, ADJUST_SECONDS);
    else stopRain(audio, ambientGain, fadeSeconds());
    moveTo(audio, interactionGain, interactionTarget(), ADJUST_SECONDS);
  }

  /** Nothing is audible: let the rain fade, then suspend the audio device entirely. */
  function quiet() {
    const audio = context;
    setStatus(unavailable ? "unavailable" : "off");
    if (!audio || !ambientGain || audio.state !== "running") return;
    stopRain(audio, ambientGain, fadeSeconds());
    void wait(ADJUST_SECONDS * 1000 + 20).then(() => {
      const stillQuiet = ambientTarget() === 0 && interactionTarget() === 0;
      if (stillQuiet && context === audio && audio.state === "running") {
        suspendedByUs = true;
        return audio.suspend();
      }
    }).catch(() => undefined);
  }

  function play(cue: Cue) {
    const audio = context;
    if (!audio || !interactionGain || !running() || interactionTarget() === 0) return;
    const start = audio.currentTime;
    for (const note of CUES[cue]) {
      const at = start + note.at;
      const envelope = audio.createGain();
      envelope.gain.setValueAtTime(0, at);
      envelope.gain.linearRampToValueAtTime(CUE_PEAK, at + 0.01);
      envelope.gain.exponentialRampToValueAtTime(0.0001, at + note.seconds);
      envelope.connect(interactionGain);
      const tone = audio.createOscillator();
      tone.type = "sine";
      tone.frequency.setValueAtTime(note.hz, at);
      tone.connect(envelope);
      tone.start(at);
      tone.stop(at + note.seconds + 0.02);
      tone.onended = () => envelope.disconnect();
    }
  }

  return {
    apply(next) {
      settings = next;
      sync();
    },
    gesture() {
      quitting = false;
      sync();
    },
    play,
    async fadeOut() {
      quitting = true;
      sync();
      await wait(QUIT_FADE_SECONDS * 1000);
    },
    status: () => current,
    onStatus(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    dispose() {
      const audio = context;
      context = null;
      ambientGain = interactionGain = rain = null;
      rendering = suspendedByUs = false;
      sources = [];
      if (audio) {
        audio.removeEventListener("statechange", sync);
        void audio.close().catch(() => undefined);
      }
      setStatus(unavailable ? "unavailable" : "off");
    },
  };
}
