import type { ReactNode } from "react";
import { Monitor, Moon, Sun } from "lucide-react";
import { APPEARANCES, APPEARANCE_LABELS, type ThemePreference } from "../lib/use-theme";
import { SHORTCUTS } from "../lib/shortcuts";
import MenuBarSettings from "./MenuBarSettings";
import DataSettings from "./DataSettings";

const APPEARANCE_ICONS: Record<ThemePreference, typeof Sun> = { dark: Moon, light: Sun, system: Monitor };

interface SettingsViewProps {
  hidden: boolean;
  preference: ThemePreference;
  onPreference: (next: ThemePreference) => void;
  /** Sections owned by other features (e.g. Sound), shown after Appearance. */
  children?: ReactNode;
}

/** Per-window preferences. Later tickets add menu-bar, shortcut and sound settings here. */
export default function SettingsView({ hidden, preference, onPreference, children }: SettingsViewProps) {
  return (
    <main className="main-view" hidden={hidden}>
      <div className="main-scroll">
        <section className="content-wrap settings" aria-label="Settings">
          <div className="page-title"><h1>Settings</h1></div>

          <h2 className="settings-heading" id="settings-appearance">Appearance</h2>
          <div className="settings-choices" role="radiogroup" aria-labelledby="settings-appearance">
            {APPEARANCES.map((value) => {
              const Icon = APPEARANCE_ICONS[value];
              return (
                <label key={value} className="settings-choice">
                  <input type="radio" name="appearance" value={value}
                    checked={preference === value} onChange={() => onPreference(value)} />
                  <Icon size={15} aria-hidden="true" />
                  <span>{APPEARANCE_LABELS[value]}</span>
                </label>
              );
            })}
          </div>
          <p className="settings-hint">Match system follows the macOS appearance as it changes.</p>

          {children}

          <MenuBarSettings active={!hidden} />

          <DataSettings active={!hidden} onRestored={() => window.location.reload()} />

          <h2 className="settings-heading">Keyboard</h2>
          <dl className="settings-shortcuts">
            {SHORTCUTS.map(({ keys, label }) => (
              <div key={keys} className="settings-shortcut">
                <dt>{label}</dt>
                <dd><kbd>{keys}</kbd></dd>
              </div>
            ))}
          </dl>
        </section>
      </div>
    </main>
  );
}
