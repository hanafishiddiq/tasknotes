import { renderRain } from "./rain";
import type { SoundSettings } from "./sound-settings";

/**
 * Where the app is, as far as sound cares. Only quitting silences the rain:
 * a minimized window, or one hidden to the menu bar, keeps it playing.
 */
export type Presence = "open" | "minimized" | "menu-bar" | "quitting";

/** off: nothing to play; waiting-for-gesture: the WebView blocked autoplay. */
export type SoundStatus = "off" | "waiting-for-gesture" | "playing" | "unavailable";

/** A moment an interaction sound marks. */
export type Cue = "create" | "complete";

export interface SoundEngine {
  apply(settings: SoundSettings): void;
  setPresence(presence: Presence): void;
  /** A click or key press: the moment WebKit lets audio start. */
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
  renderAmbient?: (sampleRate: number) => Float32Array[];
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

const defaultRain = (sampleRate: number) => renderRain(sampleRate, RAIN_LOOP_SECONDS, 1);
const defaultWait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export function createSoundEngine(options: SoundEngineOptions): SoundEngine {
  const renderAmbient = options.renderAmbient ?? defaultRain;
  const wait = options.wait ?? defaultWait;
  const listeners = new Set<() => void>();
  let settings: SoundSettings | null = null;
  let presence: Presence = "open";
  let context: AudioContext | null = null;
  let unavailable = false;
  let ambientGain: GainNode | null = null;
  let interactionGain: GainNode | null = null;
  let rain: AudioBuffer | null = null;
  let sources: AudioBufferSourceNode[] = [];
  let current: SoundStatus = "off";

  function setStatus(next: SoundStatus) {
    if (next === current) return;
    current = next;
    for (const listener of listeners) listener();
  }

  const running = () => context?.state === "running";

  function ambientTarget(): number {
    if (!settings || settings.ambient.muted || presence === "quitting") return 0;
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
    interactionGain.connect(context.destination);
    return context;
  }

  function startRain(audio: AudioContext, gain: GainNode, target: number) {
    if (!rain) {
      const channels = renderAmbient(audio.sampleRate);
      rain = audio.createBuffer(channels.length, channels[0].length, audio.sampleRate);
      channels.forEach((samples, index) => rain!.copyToChannel(samples as Float32Array<ArrayBuffer>, index));
    }
    const now = audio.currentTime;
    gain.gain.cancelScheduledValues(now);
    gain.gain.setValueAtTime(0, now);
    gain.gain.linearRampToValueAtTime(target, now + FADE_IN_SECONDS);
    const source = audio.createBufferSource();
    source.buffer = rain;
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
    const ambient = ambientTarget();
    const audible = ambient > 0 || interactionTarget() > 0;
    if (!audible && !context) return setStatus(unavailable ? "unavailable" : "off");
    if (!audible) return quiet();
    const audio = ensureContext();
    if (!audio || !ambientGain || !interactionGain) return;
    if (audio.state === "closed") return;
    if (!running()) {
      setStatus("waiting-for-gesture");
      audio.resume().then(() => { if (running()) sync(); }, () => undefined);
      return;
    }
    if (ambient > 0 && sources.length === 0) startRain(audio, ambientGain, ambient);
    else if (ambient > 0) moveTo(audio, ambientGain, ambient, ADJUST_SECONDS);
    else stopRain(audio, ambientGain, presence === "quitting" ? QUIT_FADE_SECONDS : ADJUST_SECONDS);
    interactionGain.gain.setValueAtTime(interactionTarget(), audio.currentTime);
    setStatus("playing");
  }

  /** Nothing is audible: let the rain fade, then suspend the audio device entirely. */
  function quiet() {
    const audio = context;
    setStatus(unavailable ? "unavailable" : "off");
    if (!audio || !ambientGain || audio.state !== "running") return;
    stopRain(audio, ambientGain, presence === "quitting" ? QUIT_FADE_SECONDS : ADJUST_SECONDS);
    void wait(ADJUST_SECONDS * 1000 + 20).then(() => {
      const stillQuiet = ambientTarget() === 0 && interactionTarget() === 0;
      if (stillQuiet && context === audio && audio.state === "running") return audio.suspend();
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
      tone.addEventListener?.("ended", () => envelope.disconnect());
    }
  }

  return {
    apply(next) {
      settings = next;
      sync();
    },
    setPresence(next) {
      presence = next;
      sync();
    },
    gesture() {
      if (presence === "quitting") presence = "open";
      sync();
    },
    play,
    async fadeOut() {
      presence = "quitting";
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
      sources = [];
      if (audio) {
        audio.removeEventListener("statechange", sync);
        void audio.close().catch(() => undefined);
      }
      setStatus(unavailable ? "unavailable" : "off");
    },
  };
}
