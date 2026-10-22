# Markdown as the canonical note representation

Status: accepted (2026-10-11)

Notes will be stored as Markdown even though the desktop editor presents a rich-text/WYSIWYG experience. We prefer a portable, diffable, agent-readable format to proprietary editor JSON or HTML-first storage because the user wants straightforward local export and AI analysis. Rich-text editing must preserve supported formatting and avoid lossy round-trips; unsupported complex embeds remain out of scope until a migration or explicit extension is approved.

## Addendum: supported formats and canonical spelling (2026-10-22)

Owner decision during Ticket 06 ("Tambah format umum"): Markdown written elsewhere should stay meaningful, so the editor also recognises numbered lists, block quotes, fenced code blocks, inline code and strikethrough, alongside headings, bold, italic, bullet lists, checklists and links.

Equivalent spellings are saved in one canonical form on the next edit: `_italic_` and `__bold__` become `*italic*` and `**bold**`, `[X]` becomes `[x]`, nested lists use four spaces, and runs of blank lines collapse to one. Literal `*`, `_`, `` ` ``, `~`, `\`, `[` and `]` characters are saved with a backslash (`snake\_case`, `\[WIP\]`), and so is a marker at the start of a line that would otherwise begin a heading, quote, list or rule (`\# not a heading`, `1\. not a list`). Both render identically, and text stays text when the Note is opened again. The Note title is read from the first line with the same rules, so escapes and marks never show in it.

Tables, images and raw HTML are not edited as rich text; they are kept as text, byte-for-byte.
