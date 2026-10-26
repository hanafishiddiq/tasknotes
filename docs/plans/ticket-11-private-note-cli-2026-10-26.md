# Ticket 11 — Private, AI-optimized Note CRUD in CLI: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at a73f031 (Tickets 07–10), then merged its later tip 57d0ec6 (Ticket 14, frontend only).

## Scope

The `tasknotes` CLI from Ticket 10 gains Notes, Note Folders, Note Links and the Removed view, in the same versioned JSON envelope, with Exclude from AI enforced by the core for every AI-facing interface. The desktop gets a toggle for the flag. MCP (Ticket 12) and export (Ticket 13) are out of scope; both open the core store as an AI interface and inherit the same rule without code of their own.

## Test seams (pre-agreed in the spec, Q48)

1. **Core interface** against a temporary SQLite file (`crates/tasknotes-core/tests/note_privacy.rs`): the flag, its migration, every read/write path for an AI-facing store versus the desktop store, audit history.
2. **Adapter contract** through the built binary (`crates/tasknotes-cli/tests/notes.rs`, `tests/privacy.rs`): envelopes, paging, revisions, folder paths and counts, and the privacy negative controls, comparing an excluded Note's output byte-for-byte with that of an id that never existed, and checking stderr stays empty.
3. **Desktop component test** (`src/note-privacy.test.tsx`): the toggle, and that autosave continues without a conflict.

## The privacy rule (core, `tasknotes_core::privacy`)

- Schema v11 adds `notes.exclude_from_ai` (0/1, default 0). Existing Notes stay visible to AI.
- `TaskStore::hides_private()` is true for every interface except the desktop. One SQL predicate (`note_shown_sql`) and one guard (`require_note_shown`) are used everywhere a Note is read, listed, counted or written:
  - `get_note` returns `None`; `query_notes` (list, folder scope, search and snippets) leaves the Note out; `list_task_notes` leaves it out; `list_note_tasks` and `get_link` treat the Note and its links as missing; `list_removed` leaves it out; `count_folder_notes` does not count it; `list_audit_entries` leaves out entries whose entity is the Note or one of its links.
  - Every write naming the Note (`update_note`, `move_note`, `set_note_pinned`, `set_note_removed`, `link_note`, `unlink_note`, `restore_link`, `set_note_exclude_from_ai`) runs the guard first inside its write transaction, before any Removed or revision check, so the refusal is always `NoteNotFound`/`LinkNotFound` with the generic message — exactly what an unused id gets. Since `not_found` is not an audited rejection, refused AI attempts leave no Audit Entry either.
- The flag and every AI write that checks it run in their own `BEGIN IMMEDIATE` transactions, so an AI change lands wholly before or wholly after a toggle.

## Decisions made by the implementer (owner may revisit)

- **Saya pilih flag Exclude from AI sebagai setelan seperti Pin** (tidak menaikkan revision, tidak mengubah teks/waktu edit) **dan tanpa revision check**, karena nilainya absolut (bukan edit di atas yang dibaca terakhir), hanya bisa diubah dari desktop, dan menaikkan revision akan memutus autosave editor yang sedang terbuka. Semua write AI tetap dicek revision dan privasi dalam transaksinya sendiri.
- **Saya pilih hanya desktop yang boleh mengubah flag.** Through an AI interface an excluded Note is `not_found`; for a shared Note the core refuses with the new stable code `desktop_only` (exit 5). The CLI has no command for it.
- **Saya pilih AI tidak pernah melihat field `exclude_from_ai`** in CLI output: everything an AI interface sees is shared by definition.
- **Folder in AI output (owner decision 2026-10-25):** a Note carries `folder_id` and `folder_path` (names from the top down joined with `/`, e.g. `"Work/Client A"`; `null` when not in a folder). A folder carries `path` and `note_count`. Saya pilih `note_count` = Notes langsung di folder itu (bukan subfolder) yang boleh dilihat antarmuka ini, 0 kalau folder tersembunyi oleh Removed, karena itu yang ditampilkan `note list --folder`. A folder name may itself contain `/`; `folder_id` is the stable reference.
- **Folders themselves are not private.** An AI may list, rename, move or remove a folder that holds only excluded Notes (it sees `note_count: 0`). Refusing would reveal that the folder holds a private Note; Removed is reversible, and the person sees their Note again on restore.
- **Moves can name a revision.** `MoveNote` gained an optional `expected_revision`; the CLI always sends it (`--revision` is required), the desktop's placement controls leave it out as before. A move does not bump the revision (Ticket 07 rule), so this guards against acting on a stale read, not against two simultaneous moves.
- **Links have no revision** (Ticket 08 design: one row per pair, idempotent link, unlink only an active link, restore only an unlinked one), so link commands take no `--revision`.
- **Audit for Notes, folders and links** (left to this ticket by Ticket 10): actions `note.create|update|move|pin|remove|restore|privacy`, `folder.create|rename|move|remove|restore`, `link.create|remove|restore`, written in the same transaction as the change, from every interface. Rejections recorded, as for Tasks, only for `revision_conflict` and `removed` against an existing row. Entries still hold no text. `revision` is null for link entries. Linking an already-linked pair returns the existing link and is still recorded as `link.create`.
- **Pin is not in the CLI.** It is a desktop presentation choice and has no revision; saya pilih tidak menambahkannya sampai diminta.
- **Search snippets in CLI output are plain text** (the core's U+0002/U+0003 match markers removed), since AI clients read JSON, not highlighting.

## CLI commands added (envelope `schema_version` 1, unchanged)

- `note create --markdown TEXT [--folder ID]`, `note get ID`, `note list [--text T] [--folder ID] [--limit N] [--cursor C]`, `note update ID --revision R --markdown TEXT`, `note move ID --revision R (--folder ID | --no-folder)`, `note remove|restore ID --revision R`, `note tasks ID [--limit --cursor]`
- `task notes ID [--limit --cursor]`
- `folder list [--parent ID] [--limit --cursor]`, `folder get ID`, `folder create --name N [--parent ID]`, `folder rename ID --revision R --name N`, `folder move ID --revision R (--parent ID | --top)`, `folder remove|restore ID --revision R`
- `link create --task T --note N`, `link get ID`, `link remove ID`, `link restore ID`
- `removed list [--text T] [--limit --cursor]` (Tasks, Notes, Lists and folders in Removed, with the revision a restore must name)

Shapes: Note `{id,title,markdown,revision,created_at,updated_at,folder_id,folder_path,pinned,removed}`; Note summary `{id,title,revision,updated_at,folder_id,folder_path,pinned,snippet}`; folder `{id,parent_id,name,path,revision,created_at,updated_at,has_children,note_count,removed}`; link `{id,task_id,note_id,linked_at,unlinked_at}`; linked Note `{link_id,note_id,title,updated_at}`; linked Task `{link_id,task_id,title,status}`; Removed item `{kind,id,title,revision,removed_at,in_removed_container}`.

## For Tickets 12 and 13

Open the store with `ai_access::open_authorized(dir, Interface::Mcp)` (or `open_as`) and call the same core methods; privacy needs no adapter code. An AI-facing export must read through the same methods (or `note_shown_sql`) so it cannot include an excluded Note.

## Not claimed

Interface-scoped only (ADR-0004): a process with same-user file access can read the SQLite file directly. A negligible timing side channel remains (a new Note's edit stamp is `max(now, newest stamp + 1)` across all Notes); no content, id or count is exposed.

## Vertical TDD slices

1. Core: schema v11 flag, desktop toggle, AI `get_note` hides it; v10 migration keeps Notes visible.
2. Core: AI listings and search (incl. folder scope, snippets) leave it out.
3. Core: link lists, `get_link`, `list_note_tasks` treat it as missing.
4. Core: every AI write/restore/link on it fails exactly like an unused id; only the desktop changes the flag; the desktop still edits it. Moves can name a revision.
5. Core: Removed view, folder paths, AI-visible folder counts.
6. Core: audit for Notes, folders, links; AI audit history hides the Note's and its links' entries.
7. CLI: Note, folder, link and Removed commands in the envelope.
8. CLI: subprocess negative controls (direct, list/search, folder counts, link metadata, mutations/restore/link, Removed view, audit, stderr), plus a re-share check. A mutation check (privacy predicate disabled) made 5 of the 6 privacy tests fail.
9. Desktop: toolbar toggle; autosave keeps working.
