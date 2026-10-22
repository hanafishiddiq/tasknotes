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

Found and fixed during QA: editor list items shared the class `note-list-item` with the Note list buttons, whose `display: flex` hid the bullets (CSS only, so jsdom could not see it). Not a bug: the sidebar briefly showed the old highlight; reading `AXARIACurrent` from the live WebView gave `All notes = page`, `All tasks = false`, and the paint caught up on the next edit. Switch-before-autosave ordering is proven by the Vitest test only, since each native step took longer than the 600 ms pause. Typing `- [ ] ` stays a bullet with literal text until reopen; recorded for slice 3.

Checks at the slice 2 commit: 78 Vitest tests (stable over repeated runs), typecheck, build, 57 Rust core tests, core and Tauri Clippy `-D warnings`, fmt.

