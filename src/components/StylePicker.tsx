import { useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { Folder } from "lucide-react";
import StyleIcon from "./StyleIcon";
import { STYLE_COLORS, searchIcons, styleColor, styleLabel, type Style } from "../lib/style";

/**
 * The most swatches and icons a row shows. A narrow sidebar wraps them into fewer
 * (App.css keeps every cell at least 22px), so the arrow keys move by the row the
 * window actually lays out, and by these only where there is no layout.
 */
export const SWATCH_COLUMNS = 7;
export const ICON_COLUMNS = 8;

interface StylePickerProps {
  /** The List or Note Folder being styled, for names read aloud. */
  name: string;
  style: Style;
  busy: boolean;
  /** Resolves true once the core saved it. */
  onSave: (style: Style) => Promise<boolean>;
  onCancel: () => void;
}

/**
 * Chooses a List's or Note Folder's colour and icon inline under its row
 * (nothing floats), at the sidebar's full width whatever the row's depth. Both
 * are a draft until Save; each group is a radio group whose arrow keys move
 * across and down its grid. Escape cancels. While saving nothing is disabled, so
 * focus stays where it was (on Save): the controls are aria-disabled instead.
 */
export default function StylePicker({ name, style, busy, onSave, onCancel }: StylePickerProps) {
  const [draft, setDraft] = useState<Style>(style);
  const [search, setSearch] = useState("");
  const iconGroup = useRef<HTMLDivElement>(null);
  const icons = searchIcons(search);
  const changed = draft.icon !== style.icon || draft.color !== style.color;

  async function save() {
    if (busy || !changed) return;
    await onSave(draft);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onCancel();
    }
  }

  function intoIcons(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "ArrowDown") return;
    event.preventDefault();
    iconGroup.current?.querySelector<HTMLElement>('[tabindex="0"]')?.focus();
  }

  return (
    <div className="style-picker" role="group" aria-label={`Icon and colour for ${name}`}
      aria-busy={busy || undefined} onKeyDown={onKeyDown}>
      <div className="style-picker-preview" aria-hidden="true">
        <StyleIcon icon={draft.icon} color={draft.color} />
        <span className="folder-name">{name}</span>
      </div>
      <RadioGrid label="Colour" className="style-swatches" columns={SWATCH_COLUMNS}
        options={[null, ...STYLE_COLORS]} chosen={draft.color} disabled={busy}
        onChoose={(color) => setDraft((current) => ({ ...current, color }))}
        render={(color) => ({
          label: color ? styleLabel(color) : "No colour",
          content: <span className={color ? "style-swatch" : "style-swatch style-swatch--none"}
            style={{ background: styleColor(color) }} />,
        })} />
      <input className="style-search" type="search" aria-label="Search icons" placeholder="Search icons"
        value={search} readOnly={busy} onChange={(event) => setSearch(event.target.value)} onKeyDown={intoIcons} />
      <RadioGrid label="Icon" className="style-icons" columns={ICON_COLUMNS} groupRef={iconGroup}
        options={search.trim() ? icons : [null, ...icons]} chosen={draft.icon} disabled={busy}
        onChoose={(icon) => setDraft((current) => ({ ...current, icon }))}
        render={(icon) => ({
          label: icon ? styleLabel(icon) : "Default icon",
          content: <StyleIcon icon={icon} color={draft.color} size={15} fallback={Folder} />,
        })} />
      {search.trim() && icons.length === 0 && <p className="style-empty">No icons match "{search.trim()}"</p>}
      <div className="style-picker-actions">
        <button type="button" className="button button-small button-primary" disabled={!changed && !busy}
          aria-disabled={busy || undefined} onClick={() => void save()}>Save</button>
        <button type="button" className="button button-small" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

interface RadioGridProps {
  label: string;
  className: string;
  columns: number;
  /** `null` is the default look. */
  options: (string | null)[];
  chosen: string | null;
  disabled: boolean;
  onChoose: (option: string | null) => void;
  render: (option: string | null) => { label: string; content: ReactNode };
  groupRef?: RefObject<HTMLDivElement | null>;
}

/** A radio group laid out as a grid: one tab stop, arrows move and choose (WAI-ARIA radio group). */
function RadioGrid({ label, className, columns, options, chosen, disabled, onChoose, render, groupRef }: RadioGridProps) {
  const own = useRef<HTMLDivElement>(null);
  const group = groupRef ?? own;
  // The chosen option is the tab stop; when a search hides it, the first shown one is.
  const stop = options.includes(chosen) ? chosen : options[0];
  const radios = () => [...(group.current?.querySelectorAll<HTMLElement>('[role="radio"]') ?? [])];

  /** Options in the first row as laid out; `columns` when nothing is laid out (tests). */
  function perRow(): number {
    const [first, ...rest] = radios();
    if (!first || first.offsetWidth === 0) return columns;
    const below = rest.findIndex((radio) => radio.offsetTop !== first.offsetTop);
    return below === -1 ? rest.length + 1 : below + 1;
  }

  function choose(option: string | null) {
    if (!disabled) onChoose(option);
  }

  function onKeyDown(event: KeyboardEvent<HTMLElement>, at: number) {
    const row = perRow();
    const step: Record<string, number> = {
      ArrowRight: 1, ArrowLeft: -1, ArrowDown: row, ArrowUp: -row,
      Home: -at, End: options.length - 1 - at,
    };
    if (!(event.key in step)) return;
    event.preventDefault();
    if (disabled) return;
    const next = Math.min(options.length - 1, Math.max(0, at + step[event.key]));
    onChoose(options[next]);
    radios()[next]?.focus();
  }

  return (
    <div ref={group} role="radiogroup" aria-label={label} className={className}
      style={{ "--columns": columns } as CSSProperties}>
      {options.map((option, at) => {
        const { label: name, content } = render(option);
        return (
          <button key={option ?? ""} type="button" role="radio" aria-checked={option === chosen}
            aria-label={name} title={name} tabIndex={option === stop ? 0 : -1}
            aria-disabled={disabled || undefined} onClick={() => choose(option)} onKeyDown={(event) => onKeyDown(event, at)}>
            {content}
          </button>
        );
      })}
    </div>
  );
}
