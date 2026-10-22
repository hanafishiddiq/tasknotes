# 06: Standalone Markdown Notes

**What to build:** Notes view and first Note end-to-end: rich-text editor creates/updates canonical Markdown, autosave persists and reloads; basic headings, lists, checklists, formatting and links round-trip safely. Uses a focused core Note interface.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** in progress — owner started Ticket 06 on 2026-10-22 after Ticket 05 merged. [Plan](../../../docs/plans/ticket-06-markdown-notes-2026-10-22.md).

## Acceptance criteria

- [ ] Users can create, edit and reopen a standalone Note without any associated Task.
- [ ] Headings, bold, italic, bullet lists, checklists and links round-trip through WYSIWYG editing and canonical Markdown without loss in supported cases.
- [ ] Autosave persists typing without manual Save and does not silently overwrite a more recent conflicting edit.
- [ ] Note creation and updates use the same Rust core and local SQLite persistence as desktop Tasks.
- [ ] Invalid content or interrupted writes leave the last consistent saved version recoverable.

## Verification

- [ ] Run Markdown round-trip cases, forced autosave/restart scenarios and core persistence tests on synthetic Notes.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments


## Implementation checkpoint — slices 1–2 (2026-10-22)

**Slice 1, core Note store (schema v6), `426acb3`:** standalone Notes as canonical Markdown with a title from the first line, revision-checked saves (a stale revision is a conflict, never an overwrite), a 200,000-character limit that leaves the stored version intact, keyset pages of summaries in strictly increasing edit order, and a v6 migration that rolls back whole. 8 core tests; the burst-ordering test was RED (equal millisecond stamps swapped Notes) before the edit stamp fix; the rollback test fails when the failed migration is committed (mutation check).

**Slice 2, first Note end to end, `1c98c93` plus the fixes below:** Tauri `list_notes` / `get_note` / `create_note` / `update_note`; "All notes" in the sidebar opens a Note list beside a Lexical editor, most recently edited first; "+" opens a draft stored only once text is typed; autosave after a 600 ms pause and before switching Notes, always with the expected revision; a failed save keeps the text and retries on the next edit; the editor chunk loads only when Notes first open (main bundle back to 265 kB). 8 App tests through the mocked Tauri seam; six mutation checks (eager create, whitespace draft, no flush on switch, no auto-open, no error shown, error never cleared) each make a test fail.

**Native QA** on an isolated bundle (`store.adamaja.tasknotes.ticket06qa`) seeded with a copy of the synthetic Ticket 05 QA database (schema v5, 65 Tasks). Driven through Cua Driver in direct mode (`cua-driver mcp --direct` over stdio, `attribution: host`, no daemon), because background `app_*` typing dropped characters in the WebView. The owner's database was never opened (mtime still 2026-10-13).

| Case | Result |
| --- | --- |
| v5 → v6 migration on the synthetic copy: 65 Tasks intact, `integrity_check` ok | PASS |
| First typed text creates exactly one Note (revision 1); further typing updates it | PASS |
| Quit and relaunch: Notes open with the most recent Note and its text | PASS |
| "+" then leaving without typing stores nothing | PASS |
| `- Milk` / `Eggs` makes a bullet list saved as `- Milk\n- Eggs` | PASS after fix |
| Switching to another Note opens it | PASS |

Found and fixed during QA: editor list items shared the class `note-list-item` with the Note list buttons, whose `display: flex` hid the bullets (CSS only, so jsdom could not see it). Not a bug: the sidebar briefly showed the old highlight; reading `AXARIACurrent` from the live WebView gave `All notes = page`, `All tasks = false`. Slice 3 QA narrowed it down: while the window is inactive the WebView does not repaint the sidebar at all (not even the hover background), and once the window is active the highlight moves to "All notes" at once. Switch-before-autosave ordering is proven by the Vitest test only, since each native step took longer than the 600 ms pause. Typing `- [ ] ` stays a bullet with literal text until reopen; recorded for slice 3.

Checks at the slice 2 commit: 78 Vitest tests (stable over repeated runs), typecheck, build, 57 Rust core tests, core and Tauri Clippy `-D warnings`, fmt.

## Implementation checkpoint — slice 3, Markdown fidelity (2026-10-22)

**Owner decision ("Tambah format umum"):** besides headings, bold, italic, bullet lists, checklists and links, the editor also recognises numbered lists, quotes, fenced code blocks, inline code and strikethrough, and reads `_italic_` / `__bold__`. Recorded as an addendum to ADR-0003. This widens the acceptance criterion above; nothing in it is dropped.

**What changed:** the transformer set in `src/lib/note-markdown.ts`; theme classes and CSS for the new formats; a typed-checklist shortcut inside bullet items, so typing `- [ ] Passport` makes a checkbox at once, as reopening already did; the core title now reads backslash escapes and drops inline marks (`5 \* 3` → `5 * 3`, `snake\_case` → `snake_case`, `~~old~~` → `old`).

**Tests:** 24 round-trip cases (17 kept byte-for-byte, 6 normalised once then stable, empty Note); 3 App tests that check recognition through roles (headings, `strong`, `em`, `code`, `ol`, `blockquote`, checkbox with `aria-checked`, link) and that an edit saves the rest unchanged; 1 core title test with 7 cases. Mutation checks, each making a test fail: no code blocks, no underscore emphasis, no strikethrough, bullet before checklist, underscore forms before star forms, checklist plugin removed, whole bullet list turned into a checklist, and four in the title (no escaped-bracket skip, all underscores dropped, lone `*` dropped, escapes kept as `\`).

**Native QA** on the same isolated bundle, typing through Cua Driver (direct, `attribution: host`):

| Case | Result |
| --- | --- |
| Typed `Formats`, `- [ ] Passport`, `Tickets`, `1. First`, `Second`, `> Quote`, inline code, `~~old~~`, a code block: saved exactly as canonical Markdown (`- [ ] Passport\n- [ ] Tickets`, `1. First\n2. Second`, `> Quote`, `` `pnpm test` ``, `~~old~~`, a fenced block) | PASS |
| Every format renders as formatting: circles, numbers, quote bar, monospace code, struck text | PASS |
| Clicking the Passport circle saves `- [x] Passport` (revision 18 → 19) | PASS via `app_click` |
| Switching away and back, then quit and relaunch: same rendering, revision still 19 (opening writes nothing) | PASS |

The circle click needs a real pointer event: Cua's background pixel click became an AX press at the item's centre and its foreground click did not reach the page, while `app_click` (raw input) toggled it.

**Finding (fixed below) — escaped literals lost their escape on save.** Lexical reads every CommonMark backslash escape but writes back only `*`, `_`, `` ` ``, `~` and `\`. Text that only looks like Markdown is saved without its escape and becomes formatting on the next open (probe through `roundTripNoteMarkdown`, two saves each):

| Stored | After one edit | Next open shows |
| --- | --- | --- |
| `\# Not a heading` | `# Not a heading` | a heading |
| `\- Not a bullet`, `1\. Not a list`, `\> Not a quote` | marker unescaped | a list or quote |
| `\[ \] Not a task`, `- \[ \] Literal` | `[ ] …`, `- [ ] …` | a checkbox |
| `\[draft\](later) notes` | `\\[draft\\](later) notes` | a backslash and a link |

The editor never writes these escapes itself, so today this needs Markdown from elsewhere (CLI/MCP writes, later import) or literal text the shortcuts left alone (for example after undoing one; pasted text not yet checked). Owner decision 2026-10-22: fix it, so text that only looks like Markdown stays text after saving.

**Fix — literal text keeps its escapes:** on save, plain text (not code) also escapes `[` and `]`, and a marker at the start of a line in a paragraph, list item or quote (`#`, `>`, `-`, `+`, `1.`, `1)`, a `---` rule), including after a soft line break and after leading spaces; `#tag`, `-5 degrees` and `1.5 litres` stay as typed. On open, an escaped `\[` no longer starts a link. ADR-0003 addendum updated. 16 new round-trip cases (12 RED before the fix, 2 normalised once, 2 guards against needless escapes) and 1 App test (open a Note of literal text: no heading, list, checkbox or link; one typed character saves the rest byte-for-byte; RED before the fix because `\[draft\](later)` opened as a link). 12 mutation checks each make a test fail. Native: the editor loads in WKWebView with the new link pattern; a Note seeded with the five literal lines in the isolated QA database showed them as text, and one typed character (through Cua, after background `app_type` did not reach the WebView) saved revision 2 with every escape unchanged. Checks: 115 Vitest tests, typecheck, build, 58 Rust core tests, Clippy, fmt.

Checks at the slice 3 commit: 98 Vitest tests, typecheck, build, 58 Rust core tests, core and Tauri Clippy `-D warnings`, fmt. The owner's database was not opened (mtime still 2026-10-13).
