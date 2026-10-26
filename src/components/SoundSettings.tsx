import { Volume2, VolumeX } from "lucide-react";
import type { SoundControl } from "../lib/sound/use-sound";
import type { SoundChannel } from "../lib/sound/sound-settings";

const CHANNELS: { channel: SoundChannel; label: string }[] = [
  { channel: "interaction", label: "Interaction sounds" },
  { channel: "ambient", label: "Ambient rain" },
];

const STATUS_HINTS: Partial<Record<SoundControl["status"], string>> = {
  unavailable: "Sound isn't available in this window. Everything else works as usual.",
  "waiting-for-gesture": "Sound starts after your first click or key press.",
};

/** Two independent channels, each with a one-step mute and its own volume. */
export default function SoundSettings({ sound }: { sound: SoundControl }) {
  const hint = STATUS_HINTS[sound.status];
  return (
    <>
      <h2 className="settings-heading" id="settings-sound">Sound</h2>
      <div className="settings-sound" role="group" aria-labelledby="settings-sound">
        {CHANNELS.map(({ channel, label }) => {
          const { volume, muted } = sound.settings[channel];
          const Icon = muted ? VolumeX : Volume2;
          const id = `sound-${channel}`;
          return (
            <div key={channel} className="settings-sound-row" data-muted={muted || undefined}>
              <button type="button" className="settings-mute" aria-pressed={muted}
                aria-label={`Mute ${label.toLowerCase()}`} onClick={() => sound.toggleMute(channel)}>
                <Icon size={15} aria-hidden="true" />
              </button>
              <label htmlFor={id}>{label}</label>
              <input id={id} type="range" min={0} max={100} step={1} value={volume}
                aria-label={`${label} volume`} aria-valuetext={`${volume}%`}
                onChange={(event) => sound.setVolume(channel, Number(event.target.value))} />
              <output htmlFor={id}>{volume}%</output>
            </div>
          );
        })}
        {hint && <p className="settings-hint">{hint}</p>}
      </div>
    </>
  );
}
