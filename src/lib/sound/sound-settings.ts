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
