import { useEffect, useState, useSyncExternalStore } from "react";
import { saveBeforeQuit } from "../quit";
import { createSoundEngine, type Cue, type SoundEngine, type SoundStatus } from "./sound-engine";
import { loadSoundSettings, saveSoundSettings, type SoundChannel, type SoundSettings } from "./sound-settings";

export interface SoundControl {
  settings: SoundSettings;
  status: SoundStatus;
  setVolume: (channel: SoundChannel, percent: number) => void;
  toggleMute: (channel: SoundChannel) => void;
  play: (cue: Cue) => void;
}

function webAudio(): AudioContext {
  const Context = window.AudioContext ??
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Context) throw new Error("Web Audio is not available");
  return new Context();
}

/** The app's sound: one engine for the window, its settings, and the lifecycle it follows. */
export function useSound(): SoundControl {
  const [engine] = useState<SoundEngine>(() => createSoundEngine({ createContext: webAudio }));
  const [settings, setSettings] = useState<SoundSettings>(loadSoundSettings);
  const status = useSyncExternalStore(engine.onStatus, engine.status);

  useEffect(() => {
    engine.apply(settings);
    saveSoundSettings(settings);
  }, [engine, settings]);

  useEffect(() => {
    // WebKit only lets audio start inside a user gesture; capture runs before any handler.
    const gesture = () => engine.gesture();
    window.addEventListener("pointerdown", gesture, true);
    window.addEventListener("keydown", gesture, true);
    const stopQuitFade = saveBeforeQuit(() => engine.fadeOut().then(() => true));
    return () => {
      window.removeEventListener("pointerdown", gesture, true);
      window.removeEventListener("keydown", gesture, true);
      stopQuitFade();
      engine.dispose();
    };
  }, [engine]);

  return {
    settings,
    status,
    setVolume: (channel, percent) =>
      setSettings((current) => ({ ...current, [channel]: { ...current[channel], volume: percent } })),
    toggleMute: (channel) =>
      setSettings((current) => ({ ...current, [channel]: { ...current[channel], muted: !current[channel].muted } })),
    play: engine.play,
  };
}
