---
name: Tasknotes
description: A local-first task and progress workspace dressed in Apple Notes' flat grays and Notes yellow, dark by default.
colors:
  ink-gray-canvas: "#1e1e1e"
  graphite-sidebar: "#282828"
  hover-gray: "#2c2c2c"
  folder-selected-gray: "#464646"
  muted-gold-selection: "#67551a"
  selection-text: "#ffffff"
  field-gray: "#2a2a2a"
  hairline: "#353535"
  text-primary: "#ececec"
  text-secondary: "#b2b2b7"
  text-tertiary: "#98989d"
  notes-yellow: "#ffd60a"
  notes-yellow-text: "#ffd60a"
  accent-ink: "#1e1e1e"
  alert-red: "#ff6b61"
  caution-orange: "#ffa53d"
  focus-ring: "rgba(255, 214, 10, 0.55)"
  light-paper-canvas: "#ffffff"
  light-sidebar: "#f0f0ef"
  light-hover: "#f4f4f3"
  light-folder-selected: "#dcdcdb"
  light-butter-selection: "#ffe27a"
  light-selection-text: "#1d1d1f"
  light-field: "#f4f4f3"
  light-hairline: "#e4e4e2"
  light-text-primary: "#1d1d1f"
  light-text-secondary: "#55555a"
  light-text-tertiary: "#68686d"
  light-ochre-accent: "#b58500"
  light-ochre-text: "#8a6400"
  light-accent-ink: "#1d1d1f"
  light-alert-red: "#d1281c"
  light-caution-orange: "#b85400"
  light-focus-ring: "rgba(209, 156, 0, 0.5)"
typography:
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "22px"
    fontWeight: 700
    letterSpacing: "-0.01em"
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.3
    letterSpacing: "-0.01em"
  title-small:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "15px"
    fontWeight: 600
  body-page:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.4
  body-strong:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "13px"
    fontWeight: 700
    lineHeight: 1.4
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "12px"
    fontWeight: 400
  label-strong:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "12px"
    fontWeight: 700
  section-label:
    fontFamily: "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Segoe UI\", Roboto, \"Helvetica Neue\", sans-serif"
    fontSize: "11px"
    fontWeight: 700
rounded:
  hair: "2px"
  xs: "5px"
  sm: "6px"
  row: "7px"
  md: "8px"
  capsule: "13px"
  full: "50%"
spacing:
  xxs: "4px"
  xs: "6px"
  sm: "8px"
  row: "10px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  xxl: "24px"
  tail: "72px"
components:
  button:
    textColor: "{colors.text-secondary}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "28px"
  button-hover:
    backgroundColor: "{colors.hover-gray}"
    textColor: "{colors.text-primary}"
  button-small:
    typography: "{typography.label}"
    padding: "0 10px"
    height: "26px"
  button-primary:
    backgroundColor: "{colors.notes-yellow}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "28px"
  button-primary-disabled:
    backgroundColor: "{colors.field-gray}"
    textColor: "{colors.text-tertiary}"
  icon-button:
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.sm}"
    width: "32px"
    height: "28px"
  folder:
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "0 8px"
    height: "28px"
  folder-hover:
    backgroundColor: "{colors.hover-gray}"
  folder-selected:
    backgroundColor: "{colors.folder-selected-gray}"
  task-row:
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.row}"
    padding: "8px 10px"
  task-row-hover:
    backgroundColor: "{colors.hover-gray}"
  task-row-selected:
    backgroundColor: "{colors.muted-gold-selection}"
    textColor: "{colors.selection-text}"
  property-capsule:
    backgroundColor: "{colors.field-gray}"
    textColor: "{colors.text-secondary}"
    typography: "{typography.label}"
    rounded: "{rounded.capsule}"
    padding: "4px 10px"
    height: "26px"
  capture-row:
    typography: "{typography.body-strong}"
    rounded: "{rounded.md}"
    padding: "8px 10px"
  capture-row-focus:
    backgroundColor: "{colors.hover-gray}"
  progress-number:
    backgroundColor: "{colors.field-gray}"
    rounded: "{rounded.xs}"
    padding: "2px 6px"
  progress-note-input:
    backgroundColor: "{colors.field-gray}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.sm}"
    padding: "6px 8px"
---

# Design System: Tasknotes

## Overview

**Creative North Star: "The Notes Folder, Played Straight"**

Tasknotes wears Apple Notes' design language without irony or reinterpretation: a flat graphite folder sidebar, one quiet content column, bold one-line titles over gray second lines, inset hairline separators, and Notes yellow as the only voice of color. It is built to sit beside an editor and terminal for a whole working day, so the surface is calm, dense and small-typed (13px base), and nothing on it competes for attention without carrying state.

Dark is the default appearance (`data-theme="dark"` on `<html>`), with a light appearance that mirrors it on white paper and a pale warm-gray sidebar. The user switches from the bottom of the sidebar or picks Dark, Light or Match system in Settings; the choice persists per window in `localStorage` under `tasknotes.theme` (`dark`, `light` or `system`), Match system follows `prefers-color-scheme` live, and the app falls back to dark when storage is unavailable. Both themes share every rule in this file; only token values change.

The world borrows Notes' palette, type and density but not its window chrome: there are no traffic lights, title-bar blur or vibrancy, because the product is meant to travel to other platforms. It refuses the SaaS dashboard vocabulary outright: no hero, no cards, no gradients, no colored badges.

**Key Characteristics:**
- Flat tonal layering: canvas, sidebar and hover grays do all the depth work; there are no shadows.
- One accent family (Notes yellow) reserved for state and the single primary action.
- Notes list cells: bold title, gray second line, metadata right-aligned as plain colored text.
- Inline expansion: the selected row turns gold and opens a Notes-style page beneath it, one at a time.
- System font at 13px, bold for hierarchy, tabular numerals wherever numbers align.

## Colors

Neutral grays carry everything; a single yellow, split by theme into fill and text strengths, marks state and action.

### Primary
- **Notes Yellow** (`notes-yellow`; light: **Ochre Accent** `light-ochre-accent`): fills and strokes. Folder icons in the sidebar, the in-progress status ring and its center dot, the completed status disc, the progress track fill, the primary button fill, the Next Step field's focused underline.
- **Notes Yellow Text** (`notes-yellow-text`; light: **Ochre Text** `light-ochre-text`): accent as running text. Tags (`#tag`) and inline text links in progress history. In dark it equals the fill; in light it is a darker ochre so small text passes contrast on white.
- **Accent Ink** (`accent-ink`; light `light-accent-ink`): text and check glyphs set on a yellow fill.
- **Muted Gold Selection** (`muted-gold-selection`, with `selection-text`; light: **Butter Selection** `light-butter-selection` with `light-selection-text`): the expanded/selected task row, Notes' own selected-note highlight. Everything inside the selected row (title, subtitle, metadata, status ring) inherits the selection text color.

### Secondary
- **Caution Orange** (`caution-orange`; light `light-caution-orange`): High and Very High priority, "Due today", and the metadata guard hint under the editor.
- **Alert Red** (`alert-red`; light `light-alert-red`): "Overdue", error banners and inline errors.

### Neutral
- **Ink Gray Canvas** (`ink-gray-canvas`; light **Paper** `light-paper-canvas`): the main content background and `theme-color`.
- **Graphite Sidebar** (`graphite-sidebar`; light `light-sidebar`): the folder sidebar, one step lighter than the canvas in dark, one step grayer in light.
- **Hover Gray** (`hover-gray`; light `light-hover`): hover on rows, folders and buttons; also the background of the focused capture row and error banner.
- **Folder Selected Gray** (`folder-selected-gray`; light `light-folder-selected`): the current sidebar folder; also the scrollbar thumb.
- **Field Gray** (`field-gray`; light `light-field`): property capsules, the progress number box, the progress note textarea, the disabled primary button.
- **Hairline** (`hairline`; light `light-hairline`): every 1px separator and border, and the empty progress track.
- **Text Primary / Secondary / Tertiary** (`text-primary`, `text-secondary`, `text-tertiary` and light counterparts): titles and values / second lines, metadata and quiet controls / counts, dates, placeholders and section labels.
- **Focus Ring** (`focus-ring`; light `light-focus-ring`): translucent yellow for every `:focus-visible` outline.

### Named Rules
**The One Voice Rule.** Yellow is the only hue that appears without a state reason. It marks folders, status, tags, progress, the selected row and the one primary button. Never use it for decoration, headings or borders.

**The Split Accent Rule.** In light appearance the accent has three jobs with three values: fills and rings use the ochre accent, accent-colored text uses the darker ochre text, and anything placed on a yellow fill uses near-black accent ink. Never set small text in the fill value.

**The Plain Text Status Rule.** Priority, deadline and tag metadata are colored text with an icon or `#` prefix, never filled badges. Priority always shows its icon and its label together; color is a second channel, not the only one.

## Typography

**Display Font:** none; the world has no display tier.
**Body Font:** the platform system UI stack (`-apple-system`, BlinkMacSystemFont, SF Pro Text, Segoe UI, Roboto, Helvetica Neue, sans-serif).

**Character:** Notes' own voice: one family, small sizes, hierarchy made by weight (700 for titles and headings) and by stepping text color down rather than by size. Font synthesis is off and numerals are tabular wherever counts, percentages or dates stack.

### Hierarchy
- **Headline** (700, 22px, -0.01em): the page title ("All tasks", a List name), followed by a tertiary-gray count on the same baseline.
- **Title** (700, 20px, 1.3, -0.01em): the editable task title at the top of the expanded Notes page.
- **Title Small** (600, 15px): empty-state headings only.
- **Body Page** (400, 14px, 1.5): the task description and the Next Step field inside the expanded page.
- **Body** (400, 13px, 1.4): the base for everything: rows, folders, buttons. Row titles and group headings use **Body Strong** (700); completed row titles drop to 600 in secondary gray. The in-progress percentage in a row's second line is 500 in primary text.
- **Label** (400, 12px): metadata, folder counts, timestamps, history entries, small buttons. **Label Strong** (700, 12px) names editor fields ("Progress", "Next step") and the progress history heading.
- **Section Label** (700, 11px, sentence case, tertiary gray): sidebar group names, as Notes labels its account sections.

### Named Rules
**The Weight Not Size Rule.** Hierarchy within a list comes from bold versus regular and from the three text grays. Do not add sizes between 13px and 20px for list content.

## Layout

A two-column shell: a fixed 210px folder sidebar and a fluid main view. The main view is a 52px toolbar (sort control and compose icon, right-aligned, hairline below) over a scrolling region whose content column is centered at a 720px maximum with 22px top, 24px side and 72px bottom padding. Inside the column, rows and headings share a 10px inner inset so titles, group headings and the capture row align on one left edge; the status ring column is 18px plus a 10px gap, so secondary content (separators, the expanded page) starts at 38px.

Vertical rhythm is tight: 1px between sidebar folders, 4px under group headings, 8px row padding, 16px under the capture row, 20px between status groups. The expanded editor caps its inner content at 600px.

At 720px and below the sidebar becomes a horizontal, scrollable strip across the top (section labels, counts, spacer and hover actions hidden), the content padding drops to 16px, and each row's metadata wraps onto its own line indented under the title.

## Elevation & Depth

Flat. Depth comes only from tonal steps: canvas, then sidebar, then hover, then field and selected grays, separated by 1px hairlines. No element casts a shadow. The single inset ring (`inset 0 0 0 1px` hairline) on the progress number box at hover is an outline, not elevation.

### Named Rules
**The No-Lift Rule.** Nothing floats. State is shown by a background step (hover, selected, focused capture) or a focus outline, never by shadow or translation.

## Shapes

Gently rounded rectangles at small radii, sized to their control: 5px for tiny icon actions and the progress number, 6px for buttons, folders and the focus outline, 7px for task rows, 8px for the capture row and banners, 13px capsules for editor properties, and true circles for status rings. Borders are rare; the only structural lines are the 1px sidebar edge, toolbar underline, inset row separators and the Next Step underline. Icons are Lucide outlines at 13-18px, inheriting text color except folder icons, which take the accent.

## Components

### Buttons
Quiet until needed; only the primary button carries color.
- **Shape:** gently rounded (6px), 28px tall (26px small).
- **Default:** transparent with secondary-gray text; hover steps to hover gray and primary text.
- **Primary:** Notes yellow fill with accent ink, weight 600. Hover keeps the fill and brightens it slightly (`brightness(1.05)`). Disabled primary becomes a field-gray box with tertiary text at weight 500 rather than a faded yellow.
- **Icon button:** 32x28, secondary-gray icon, same hover as default.
- **Text link:** small accent-text label with no box (history "Load more", retry).

### Sidebar Folders
- **Style:** 28px rows, 8px inset, Lucide icon (17px) in the accent, name truncating, tabular count in tertiary gray pushed right.
- **States:** hover is hover gray; current page is folder-selected gray. Rename and remove actions (22px icon buttons) appear only on row hover or keyboard focus. Quiet folders (removed-lists toggle, appearance toggle) use secondary gray for both icon and text.
- **New List:** an inline field styled as a folder row with a plus icon; its confirm action appears on focus.

### Task Row (signature)
A Notes list cell.
- **Structure:** 18px status ring, then a bold single-line title over a gray second line ("60%" in primary text, then the Next Step or description preview), metadata right-aligned at 12px capped at 45% width.
- **Status ring:** Ready is a tertiary-gray outline; In Progress is an accent outline with a 6px accent dot; Completed is a filled accent disc with an ink check.
- **Separators:** 1px hairline inset from 38px to 10px from the right, hidden next to a hovered or expanded row.
- **Selected:** the row fills with the selection color and every child, including the ring, switches to the selection text color.

### Expanded Task Page
The selected row opens a page underneath it, indented to the 38px content edge: a tertiary timestamp line, the 20px editable title, a 14px borderless description, then property capsules, the progress editor and progress history. Fields are borderless and blend into the page; Save actions appear only when there is something to save.

### Property Capsules
- **Style:** 13px-radius capsules on field gray, 12px secondary text, a 13px leading icon (status ring, priority icon, folder, calendar, tag) and a bare select or input with no native chrome.
- **Focus:** the whole capsule takes the focus outline.

### Inputs / Fields
- **Capture row:** a dashed 18px ring and a bold borderless title field; the row takes hover gray when focused or open and reveals a description field and actions indented at 38px.
- **Next Step:** borderless 14px field with a 1px hairline underline that turns accent on focus.
- **Progress note:** field-gray textarea, 6px radius, vertically resizable.
- **Progress:** a 4px hairline track with an accent fill, beside a small field-gray number box.
- **Focus:** a 2px translucent-yellow outline offset by 1px on every focusable element.

### Progress History
A compact 12px timeline: tertiary date, then status and "from% -> to%" in primary text with tabular numerals, then note and "Next:" lines in secondary gray, each entry separated by a hairline.

### Notes Workspace
Apple Notes columns: the folder sidebar, a Note list, then the editor.
- **Note list:** a Headline "Notes" with the compose icon, then 7px-radius cells of a bold single-line title over a 12px tertiary date; the open Note takes the selected row color. Older Notes load from a small accent-text "Load more Notes" action under the list.
- **Editor:** a borderless sheet centered at 720px with 32px side padding (16px at 720px and below). Note text is a reading size above Body Page: 15px/1.55, headings 24, 19, 16 and 15px at 700. Checklist items draw an 18px circle that fills with the accent and an ink tick when done; quotes take a 3px folder-selected bar and secondary text; code is monospace on field gray (6px radius for blocks, 5px inline).
- **Banners:** save errors use the error banner; "changed somewhere else" uses the notice banner with its two choices as small buttons.

### Settings
A page like Removed: the Headline "Settings", then Label Strong section names ("Appearance", "Keyboard") in secondary gray. Appearance is a radio group of 28px folder-style rows (Lucide icon, label) with the accent as the radio color; keyboard focus outlines the whole row. Keyboard lists each shortcut as a hairline-separated row, label left and the keys right in secondary gray. Settings is a quiet folder at the bottom of the sidebar, above the appearance toggle.

"Menu bar and Quick Capture" (between Appearance and Keyboard) uses the same 28px rows with checkboxes: "Keep Tasknotes in the menu bar" and "Open Quick Capture with a global shortcut", each followed by a tertiary hint saying what happens now. Recording a shortcut is a small "Press the new shortcut" button indented under its row; the chosen combination shows as `kbd` beside "Change shortcut". A refused or taken combination is plain text: the reason in red, then secondary gray lines naming what still works ("⌘N still creates a task inside Tasknotes."). No banners or dialogs.

### Command Palette (⌘K)
A 560px sheet on the sidebar gray, 72px from the top of the window, with a 1px hairline edge and 8px radius; no shadow and no dimmed backdrop. A search field (accent underline when focused) over 28px command rows: label left, its group ("Go to", "Actions") right in tertiary gray. The highlighted row takes the selected-row color. Free text adds a last row, "Search for "text"".

### Undo Notice
After anything moves to Removed, a flat bar in the window's last row (under the main column, beside the sidebar): hover-gray fill, hairline top, secondary text ""Title" moved to Removed" and a small Undo button. It stays in flow (never floats) and the bar leaves on the next navigation, but the last 10 moves to Removed stay undoable, newest first: ⌘Z outside a text field, or Undo in the ⌘K palette, takes back the newest even after moving between views.

### Virtualized Task Rows
Only rows near the visible part of the list are in the page; spacers stand in for the rest, so scrolling, row heights and group headings look exactly as if every row were there. The space above a later status group belongs to its heading (`task-group-heading--later`) so it is measured. A closed row still holding typed text says "Unsaved edits" in italic secondary gray in its metadata.

### Task–Note Links
A quiet list, styled like the progress history: a Label Strong heading ("Notes" on the Task page, "Tasks" above an open Note), then 28px rows of a tertiary icon and a single-line title (a Task also shows its work status in tertiary text), separated by hairlines. Each row has a 22px unlink "x" in tertiary gray; hover is hover gray. "Link a Note" / "Link a Task" is an accent-text link that opens a field-gray picker (8px radius): recent Notes, or a search field over Tasks. Unlinking leaves a one-line notice, "Unlinked "Title". The Note itself is unchanged." with an accent-text Undo. Opening a linked Task shows it alone, expanded, under a notice banner with "Show all tasks".

## Do's and Don'ts

### Do:
- **Do** keep the canvas, sidebar, hover and field grays as the only surface steps, separated by 1px hairlines.
- **Do** reserve Notes yellow for folder icons, status rings, tags, progress fill, focus, the selected row and the single primary action.
- **Do** use the darker accent-text value for any yellow text in light appearance and accent ink on any yellow fill.
- **Do** show priority as icon plus label, deadlines as words ("Overdue", "Due today", "Due Oct 22"), tags as `#name`.
- **Do** build list hierarchy with bold titles and the three text grays at 13px, with tabular numerals for counts and percentages.
- **Do** design every new surface in both appearances, starting from dark.
- **Do** keep motion to 120ms background fades, a 120ms fade-in of an opening Task page and a 240ms progress fill, and drop them under Reduce Motion.

### Don't:
- **Don't** add hero sections, cards, gradients or filled badges; this is the SaaS-dashboard look the owner rejected.
- **Don't** reproduce macOS window chrome (traffic lights, title-bar vibrancy, translucent sidebars); the product must make sense on other platforms.
- **Don't** add shadows or lift; depth is tonal only.
- **Don't** introduce a second accent hue or a display typeface.
- **Don't** convey priority, deadline or status by color alone.
