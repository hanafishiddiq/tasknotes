# Ticket 06 — Standalone Markdown Notes: plan (2026-10-22)

Owner instruction 2026-10-22: start Ticket 06 after Ticket 05 merged (PR #18).

## Owner decisions

- **Editor:** Lexical (already started in the earlier Ticket 06 WIP; stable 0.52.0).
- **Layout:** Apple Notes style. Sidebar, then a Note list column, then the editor on the right.
- **Title:** the first line of the Note is its title; there is no separate title field.
- **New Note:** "+" opens an empty draft. The Note is stored only when the first character is typed, so abandoned drafts never pile up as empty Notes (no deletion needed).
- **Conflict:** when autosave finds the Note changed elsewhere, autosave pauses and a banner offers "Load latest" or "Save mine as a new Note". Nothing is overwritten.
- **Test seams (agreed):** core Rust Note interface; the Markdown round-trip function; the desktop App through the mocked Tauri seam; native QA on an isolated QA bundle (manual, not automated).

## Design

**Schema v6 (forward-only, one transaction):**
- `notes(id, markdown, title, revision, created_at, updated_at)`, index on `(updated_at DESC, id DESC)`.
- `title` is derived by the core from the first non-empty line on every write (Markdown markers and link syntax stripped, at most 100 characters, "New Note" when empty), stored so list pages never load full documents.
- Not in v6: folders, pins and Note search (Ticket 07), Removed for Notes (Ticket 09), Exclude from AI (Ticket 11).

**Core interface (`notes.rs`, on the shared store):** `create_note`, `get_note`, `update_note` (requires the expected revision; a stale revision is a conflict, never a silent overwrite), `list_notes` (keyset page of summaries, most recently edited first). Markdown is limited to 200,000 characters; a rejected write leaves the stored Note unchanged. A single UPDATE is atomic, so an interrupted write leaves the previous version.

**Desktop:** "Notes" in the sidebar; list column of summaries; Lexical editor importing and exporting canonical Markdown with the ADR-0003 subset (headings, bold, italic, bullet lists, checklists, links; owner extension 2026-10-22: numbered lists, quotes, code blocks, inline code, strikethrough). Autosave after a short idle pause and when leaving the Note, always with the expected revision. A failed save keeps the typed text on screen. On a failed save the desktop re-reads the Note: a newer revision means a conflict banner, otherwise the error is shown.

## Vertical TDD slices

1. **Core Note store:** v5 → v6 migration (existing data intact, failed migration rolls back), create/get/update/list, revision conflict, title from the first line, size limit.
2. **First Note end to end:** Tauri commands, Notes view with list and editor, lazy creation on the first character, autosave, reopen after reload.
3. **Markdown fidelity:** each supported format is *recognised* by the editor (a checklist is a checkbox, not a bullet with "[ ]" text) and comes back byte-for-byte; unsupported syntax survives as text. The existing round-trip cases only prove nothing is lost: removing the checklist transformer still passes them (checked 2026-10-22). Native QA on slice 2 found the typing side of the same gap: typing `- [ ] Passport` makes a bullet whose text starts with "[ ] " (the `- ` shortcut fires first), it saves as `- [ ] Passport`, and on reopen the item becomes a real checkbox. Typing and reopening must agree. Done 2026-10-22 (see the ticket checkpoint); escaped literals such as `\# text` lost their escape on save; fixed the same day on the owner's instruction.
4. **Conflict:** stale revision → banner → "Load latest" / "Save mine as a new Note". Done 2026-10-22; while the banner is up the Note cannot be left, so unsaved text is never dropped by navigating away.
5. **Recovery and acceptance:** rejected and interrupted writes keep the last saved version; native QA (typing, quit/reopen); independent review; ticket evidence. Native QA proved that typing within the autosave pause was lost on quit. Owner decision 2026-10-22 ("Simpan dulu saat keluar"): Cmd+Q and closing the window save pending Note text first; when it cannot be saved the app stays open and shows why. Quit from the Dock or a shutdown cannot be delayed (tao exits straight from `applicationWillTerminate`) and keeps up to one autosave pause of risk. Done 2026-10-22 with the review fixes; see the ticket checkpoint.

## Risks

- Schema v6 is forward-only like v4 and v5: no build from `main` may open the owner's database without a verified backup.
- jsdom types into Lexical only with two polyfills (`src/test/setup.ts`: `InputEvent.getTargetRanges`, `Range.getBoundingClientRect`); it still does not render CSS, so layout and real WKWebView typing are covered by native QA.
- Very large Notes stress the editor before they stress SQLite; Ticket 17 measures 10,000 Notes.
