import { Folder, type LucideIcon } from "lucide-react";
import { STYLE_ICONS } from "../lib/style-icons";
import { styleColor } from "../lib/style";

interface StyleIconProps {
  icon?: string | null;
  color?: string | null;
  size?: number;
  /** Shown without an icon of its own (or one this build does not know). */
  fallback?: LucideIcon;
}

/** A List's or Note Folder's icon in its colour; without them, the default folder in the accent. */
export default function StyleIcon({ icon, color, size = 17, fallback = Folder }: StyleIconProps) {
  const Icon = (icon && STYLE_ICONS[icon]) || fallback;
  return (
    <Icon size={size} aria-hidden="true" style={{ color: styleColor(color) }}
      data-style-icon={icon ?? undefined} data-style-color={color ?? undefined} />
  );
}
