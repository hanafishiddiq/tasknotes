# Markdown as the canonical note representation

Status: accepted (2026-10-11)

Notes will be stored as Markdown even though the desktop editor presents a rich-text/WYSIWYG experience. We prefer a portable, diffable, agent-readable format to proprietary editor JSON or HTML-first storage because the user wants straightforward local export and AI analysis. Rich-text editing must preserve supported formatting and avoid lossy round-trips; unsupported complex embeds remain out of scope until a migration or explicit extension is approved.

## Addendum: supported formats and canonical spelling (2026-10-22)

Owner decision during Ticket 06 ("Tambah format umum"): Markdown written elsewhere should stay meaningful, so the editor also recognises numbered lists, block quotes, fenced code blocks, inline code and strikethrough, alongside headings, bold, italic, bullet lists, checklists and links.

Equivalent spellings are saved in one canonical form on the next edit: `_italic_` and `__bold__` become `*italic*` and `**bold**`, `[X]` becomes `[x]`, nested lists use four spaces, and runs of blank lines collapse to one. Literal `*`, `_`, `` ` ``, `~` and `\` are saved with a backslash (`snake\_case`). Beyond that, plain text keeps a backslash exactly where this editor would otherwise read Markdown when the Note opens again: a heading, quote, bullet or `1.` marker at the start of a paragraph line (`\# not a heading`, `1\. not a list`), a checkbox at the start of a paragraph or list item (`\[ \] not a task`), and brackets in text that would form a link (`\[draft\](later)`). Escapes render identically, and text stays text.

Syntax this editor does not edit (tables, images, raw HTML, rules, `1)` lists, setext headings, reference links) is shown as text and saved as written, so other Markdown tools keep reading it the same way. One consequence: an escape on such syntax (`\---`, `2\)`) is not kept, because once opened it cannot be told from the usual spelling, which Markdown from elsewhere nearly always uses.

The Note title is the first line as the editor shows it: block markers, inline marks, escapes and link targets removed; a Note that opens with a code block is titled by its first line of code.
