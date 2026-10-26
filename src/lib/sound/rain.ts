/**
 * Soft rain, synthesized in code: no recording is bundled, so there is no
 * third-party license to review. The provenance is this file.
 *
 * Each channel is one seamless loop of:
 * - a bed of pink noise (Paul Kellet's filter), softened by a gentle low-pass
 *   and cleared of rumble by a high-pass, with a slow swell that repeats a whole
 *   number of times per loop;
 * - sparse drops: short sine ticks with a fast exponential decay at random
 *   pitches and loudness, placed by a seeded random process.
 * The tail is cross-faded into the head so the loop has no click, and the result
 * is scaled to a quiet, unclipped level. Everything runs once, before playback;
 * the audio thread then only replays the buffer.
 */

const BED_LOW_PASS_HZ = 1_600;
const BED_HIGH_PASS_HZ = 140;
const DROPS_PER_SECOND = 14;
const SWELLS_PER_LOOP = 2;
const CROSSFADE_SECONDS = 0.25;
const TARGET_RMS = 0.16;
const PEAK_LIMIT = 0.85;

/** A small seeded generator, so the same seed always renders the same rain. */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

function onePole(cutoffHz: number, sampleRate: number): number {
  return 1 - Math.exp((-2 * Math.PI * cutoffHz) / sampleRate);
}

interface Drop { phase: number; step: number; level: number; decay: number }

function renderChannel(sampleRate: number, length: number, seed: number): Float32Array {
  const next = random(seed);
  const extra = Math.round(sampleRate * CROSSFADE_SECONDS);
  const raw = new Float32Array(length + extra);
  const lowPass = onePole(BED_LOW_PASS_HZ, sampleRate);
  const rumble = onePole(BED_HIGH_PASS_HZ, sampleRate);
  const dropChance = DROPS_PER_SECOND / sampleRate;
  let b0 = 0, b1 = 0, b2 = 0, soft = 0, low = 0;
  const drops: Drop[] = [];

  for (let i = 0; i < raw.length; i += 1) {
    const white = next() * 2 - 1;
    b0 = 0.99765 * b0 + white * 0.099;
    b1 = 0.963 * b1 + white * 0.2965;
    b2 = 0.57 * b2 + white * 1.0526;
    const pink = (b0 + b1 + b2 + white * 0.1848) * 0.2;
    soft += lowPass * (pink - soft);
    low += rumble * (soft - low);
    const swell = 1 + 0.12 * Math.sin((2 * Math.PI * SWELLS_PER_LOOP * i) / length);
    let sample = (soft - low) * swell;

    if (next() < dropChance) {
      drops.push({
        phase: 0,
        step: (2 * Math.PI * (700 + next() * 1_500)) / sampleRate,
        level: 0.05 + next() * 0.12,
        decay: Math.exp(-1 / (sampleRate * (0.004 + next() * 0.01))),
      });
    }
    for (let d = drops.length - 1; d >= 0; d -= 1) {
      const drop = drops[d];
      sample += Math.sin(drop.phase) * drop.level;
      drop.phase += drop.step;
      drop.level *= drop.decay;
      if (drop.level < 1e-4) drops.splice(d, 1);
    }
    raw[i] = sample;
  }

  // Equal-power cross-fade: the loop's first samples blend from what follows its
  // last sample into its own start, so the wrap-around is continuous.
  const out = raw.slice(0, length);
  for (let i = 0; i < extra && i < length; i += 1) {
    const t = i / extra;
    out[i] = raw[i] * Math.sin(t * Math.PI / 2) + raw[length + i] * Math.cos(t * Math.PI / 2);
  }

  let sum = 0, peak = 0;
  for (const value of out) {
    sum += value * value;
    peak = Math.max(peak, Math.abs(value));
  }
  const rms = Math.sqrt(sum / out.length) || 1;
  const scale = Math.min(TARGET_RMS / rms, PEAK_LIMIT / (peak || 1));
  for (let i = 0; i < out.length; i += 1) out[i] *= scale;
  return out;
}

/** Renders a stereo loop of `seconds` of soft rain; channels differ so the rain feels wide. */
export function renderRain(sampleRate: number, seconds: number, seed: number): Float32Array[] {
  const length = Math.round(sampleRate * seconds);
  return [renderChannel(sampleRate, length, seed * 2), renderChannel(sampleRate, length, seed * 2 + 1)];
}
