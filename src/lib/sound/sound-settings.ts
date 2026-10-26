/** The two sound channels, each with its own volume and mute. */
export type SoundChannel = "interaction" | "ambient";

export interface ChannelSettings {
  /** Percent, 0–100. */
  volume: number;
  muted: boolean;
}

export type SoundSettings = Record<SoundChannel, ChannelSettings>;

/** Both channels start on, quietly: interaction 12%, ambient 8%. */
export const DEFAULT_SOUND_SETTINGS: SoundSettings = {
  interaction: { volume: 12, muted: false },
  ambient: { volume: 8, muted: false },
};

const STORAGE_KEY = "tasknotes.sound";

function channel(value: unknown, fallback: ChannelSettings): ChannelSettings {
  const raw = (value ?? {}) as Partial<Record<keyof ChannelSettings, unknown>>;
  const volume = typeof raw.volume === "number" && Number.isFinite(raw.volume)
    ? Math.min(100, Math.max(0, Math.round(raw.volume))) : fallback.volume;
  return { volume, muted: typeof raw.muted === "boolean" ? raw.muted : fallback.muted };
}

// A per-window preference like the theme: when storage is unavailable or the
// value is unreadable, the quiet defaults apply.
export function loadSoundSettings(): SoundSettings {
  try {
    const stored = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? "null") as Partial<SoundSettings> | null;
    if (!stored || typeof stored !== "object") return DEFAULT_SOUND_SETTINGS;
    return {
      interaction: channel(stored.interaction, DEFAULT_SOUND_SETTINGS.interaction),
      ambient: channel(stored.ambient, DEFAULT_SOUND_SETTINGS.ambient),
    };
  } catch {
    return DEFAULT_SOUND_SETTINGS;
  }
}

export function saveSoundSettings(settings: SoundSettings): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Keep the in-memory choice for this session.
  }
}
