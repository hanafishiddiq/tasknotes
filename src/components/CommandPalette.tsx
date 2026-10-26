import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Search } from "lucide-react";

export interface Command {
  id: string;
  label: string;
  /** "Go to" or "Actions": shown at the right of the row. */
  group: string;
  /** Extra words the filter matches, e.g. "theme" for appearance. */
  keywords?: string;
  run: () => void;
}

interface CommandPaletteProps {
  commands: Command[];
  /** The command for free text, e.g. a search for what was typed. */
  textCommand: (text: string) => Command | null;
  onClose: () => void;
}

function matches(command: Command, words: string[]): boolean {
  const haystack = `${command.label} ${command.group} ${command.keywords ?? ""}`.toLowerCase();
  return words.every((word) => haystack.includes(word));
}

/**
 * ⌘K: a filterable list of places and actions. The field keeps focus; the
 * arrow keys move the highlighted command and Return runs it.
 */
export default function CommandPalette({ commands, textCommand, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const listId = useId();

  const shown = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    const found = commands.filter((command) => matches(command, words));
    const free = query.trim() ? textCommand(query.trim()) : null;
    return free ? [...found, free] : found;
  }, [commands, query, textCommand]);
  const current = Math.min(active, Math.max(shown.length - 1, 0));

  useEffect(() => { input.current?.focus(); }, []);

  function run(command: Command | undefined) {
    if (!command) return;
    onClose();
    command.run();
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (shown.length === 0) return;
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActive((current + step + shown.length) % shown.length);
    } else if (event.key === "Enter" && !event.nativeEvent.isComposing) {
      event.preventDefault();
      run(shown[current]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "Tab") {
      // The field is the only stop inside the palette.
      event.preventDefault();
    }
  }

  const optionId = (index: number) => `${listId}-${index}`;

  return (
    <div className="palette-layer" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <div className="palette" role="dialog" aria-modal="true" aria-label="Command palette">
        <label className="palette-field">
          <Search size={15} aria-hidden="true" />
          <input
            ref={input}
            role="combobox"
            aria-label="Command"
            aria-expanded="true"
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={shown.length > 0 ? optionId(current) : undefined}
            placeholder="Go to, do, or search…"
            maxLength={200}
            value={query}
            onChange={(event) => { setQuery(event.target.value); setActive(0); }}
            onKeyDown={onKeyDown}
          />
        </label>
        <ul className="palette-list" id={listId} role="listbox" aria-label="Commands">
          {shown.map((command, index) => (
            <li
              key={command.id}
              id={optionId(index)}
              role="option"
              aria-selected={index === current}
              className="palette-option"
              onMouseMove={() => setActive(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => run(command)}
            >
              <span className="palette-label">{command.label}</span>
              <span className="palette-group" aria-hidden="true">{command.group}</span>
            </li>
          ))}
          {shown.length === 0 && <li className="palette-empty" role="presentation">No matching commands</li>}
        </ul>
      </div>
    </div>
  );
}
