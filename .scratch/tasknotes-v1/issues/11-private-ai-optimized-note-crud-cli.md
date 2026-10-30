# 11: Private, AI-optimized Note CRUD in CLI

**What to build:** Extend the same local CLI schema to Notes and links; enforce Exclude from AI on get/list/search/write and relationship metadata; ensure private Notes are not leaked through error messages or audit/log output. Validate privacy in isolated command-line tests.

**Blocked by:** 07 — Note organization and searching, 08 — Reversible Task–Note links, 10 — Local CLI Task CRUD with concurrency and audit

**Status:** ready-for-agent

**Implementation:** review findings fixed; re-review done on 2026-10-30 and its findings fixed; awaiting the owner's native check and merge decision; on `integration/tasknotes-v1-07-18` (Draft PR #20). Plan: `docs/plans/ticket-11-private-note-cli-2026-10-26.md`.

## Acceptance criteria

- [x] AI-facing CLI Notes CRUD, listing, search and Task–Note linkage operations follow the same versioned envelope and revision rules as Tasks.
- [x] A Note marked Exclude from AI is invisible through AI list, get, search, linked-Note metadata and export-like operations. *(No export command exists yet; Ticket 13 must read through the same core methods. `note get`/`note list` are the full-content reads today and hide it.)*
- [x] AI writes, restores or link changes targeting an Exclude from AI Note are denied without confirming its existence through error details.
- [x] A user acting through the desktop UI can still view/edit their excluded Note; changing the privacy flag updates the shared core's policy atomically.
- [x] Logs, error messages and Audit Entries exposed to AI avoid disclosing restricted Note titles or content.

## Verification

- [x] Exercise direct, search, link, mutation and failure-side privacy negative controls through actual CLI subprocesses.
- [x] Relevant tests pass and evidence is recorded here.
- [x] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-22:** Note Folders can be nested (owner decision, see Ticket 07). Decide how folder paths appear in AI-facing output, and make sure a folder path, name or count never reveals a Note marked Exclude from AI.

**2026-10-25 — owner decision (Han):** AI-facing output shows a Note's folder as both a full path (for example `"Work/Client A"`) and a stable `folder_id`. Any per-folder count or listing exposed to AI counts only Notes that AI may see, so a Note marked Exclude from AI never shows up through folder names, paths or counts.

**2026-10-26 — implementation (AI implementer, awaiting independent review).** Core schema v11 adds `notes.exclude_from_ai`; the rule lives in `tasknotes_core::privacy` and applies to every store opened for an AI interface (CLI now, MCP in Ticket 12), so adapters hold no privacy code. To AI an excluded Note is exactly a never-used id: same `not_found` envelope on get, writes, restore and link changes; absent from listings, search and snippets, folder counts, linked-Note metadata, the Removed view and audit history; nothing on stderr. Decisions (owner may revisit), details in the plan:
- Saya pilih flag sebagai setelan seperti Pin (tidak menaikkan revision, tanpa revision check) karena nilainya absolut, hanya desktop yang boleh mengubahnya, dan autosave editor yang terbuka tidak boleh terputus. AI yang mencoba mengubahnya mendapat `not_found` (Note excluded) atau kode baru `desktop_only` (exit 5).
- Saya pilih output AI tidak memuat field `exclude_from_ai` karena semua yang terlihat AI memang dibagikan.
- Folder sesuai keputusan owner 2026-10-25: Note membawa `folder_id` + `folder_path` (`"Work/Client A"`); folder membawa `path` + `note_count`. Saya pilih `note_count` = Note langsung di folder itu yang boleh dilihat antarmuka ini, karena sama dengan isi `note list --folder`.
- Saya pilih folder sendiri tidak privat: AI boleh melihat, mengganti nama dan memindahkan folder yang hanya berisi Note excluded (terlihat `note_count: 0`). Menghapusnya mengikuti keputusan owner B di Ticket 12: folder itu dihitung berisi, jadi permintaan langsung dijawab `needs_approval` dan menunggu persetujuan di desktop, tanpa jumlah atau isi untuk AI. Sisa yang diterima: AI bisa menyimpulkan "ada yang tidak terlihat" dari `needs_approval` pada folder yang tampak kosong, tidak pernah apa atau berapa.
- Saya pilih move Note bisa membawa revision (`MoveNote.expected_revision`, wajib di CLI, opsional di desktop) karena aturan revision check berlaku untuk semua write AI; link tidak punya revision (desain Ticket 08).
- Saya pilih Audit Entry untuk Note/folder/link (`note.*`, `folder.*`, `link.*`) dari semua antarmuka, dengan aturan penolakan sama seperti Task; riwayat yang dilihat AI menyembunyikan entri tentang Note excluded dan link-nya.
- Saya pilih Pin tidak masuk CLI sampai diminta, dan snippet pencarian di CLI berupa teks polos.

Desktop: toolbar button "Exclude from AI" (pressed state) beside Pin on the open Note; autosave continues over the same revision.

Evidence (implementer's own runs, 2026-10-26, after merging integration tip 57d0ec6): `crates/tasknotes-core` cargo test 134 passed (new `tests/note_privacy.rs` 9, `note_placement` +1); `crates/tasknotes-cli` cargo test 32 passed (new `tests/notes.rs` 4 and `tests/privacy.rs` 6 separate-process negative controls: direct get, list/search/folder scope, folder counts, task-notes/note-tasks/link get, update/stale update/move/remove/restore/link create/remove/restore, Removed view, audit list incl. `--entity`, stderr empty, each compared with a never-used id; re-share makes it visible at once). Mutation check: with the privacy predicate disabled, 5 of the 6 CLI privacy tests failed. `src-tauri` cargo test 1 passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean in all three; `pnpm test` 188 passed (new `src/note-privacy.test.tsx` 2); `pnpm build` ok.

Open: `/code-review` not run yet; native GUI check of the toggle not done (component test only); export privacy belongs to Ticket 13.

**2026-10-26 — review fixes (AI implementer).** Independent `/code-review` findings fixed test-first; details and evidence in the plan, section "2026-10-26 — review fixes":
- AI audit list no longer shows the global append number, and its cursor is now the id of the last visible entry (`a2|<id>`), so gaps can no longer count a private Note's entries. Proven through the real CLI: history with a private Note edited 4 times reads, page by page, exactly like history without it.
- **Coordinator decision (owner may revisit):** sharing a Note again does not open what happened while it was private. AI never sees `note.privacy` entries, nor entries about the Note or its links recorded while it was excluded (schema v12 `audit_entries.hidden_from_ai`).
- One privacy predicate in `privacy.rs` (audit, links, stamps); exhaustive audit entity-table match.
- Saya pilih konvensi terdokumentasi + tes katalog (semua read Note publik dengan store AI) daripada view SQLite yang menyaring otomatis, karena view itu memaksa semua write lewat `main.notes` dan diam-diam merusak query yang memang perlu melihat Note excluded.
- Note and link stamps for AI are computed over what it can see, so an excluded Note's edit time cannot show through an AI's own timestamps.

Evidence (implementer's own runs): core 137 passed, cli 33 passed, src-tauri 1 passed; clippy `-D warnings` and fmt clean in all three; `pnpm test` 188 passed; `pnpm build` ok. Still open: native GUI check of the toggle; export privacy belongs to Ticket 13.

**2026-10-30 — re-review outcome (implementer).**
- **Privacy, fixed (`a49afeb`).** An AI search could receive a snippet of text written after the Note was excluded, because the page and its snippets were separate reads with no shared transaction. `list_note_tasks` had the same gap between the privacy check and the page, so a link made after the exclusion could reach the AI. Both now run in one deferred read transaction on the `connect()` connection; `every_write_transaction_checks_the_restore_epoch` still passes.
- **Tests.** `privacy::tests::an_ai_search_cuts_snippets_from_the_state_it_chose_the_page_in` and `an_ai_link_list_reads_the_state_it_checked_privacy_in` write through a `#[cfg(test)]` step between the two reads, so the race is deterministic. Both failed before the fix (the snippet read `Lease lease zzsecretword private`; the private Task was listed) and pass after it. The reviewer's 25-second race, run once as a throwaway: 441 queries, 350 writer cycles, 0 leaks.
- **Docs.** The folder decision above and the plan now match owner decision B in Ticket 12: an AI may still list, rename and move a folder holding only excluded Notes, but removing it answers `needs_approval`.

Evidence (implementer's own runs, synthetic data; these fixes have had no further independent review): `cargo test` core 204, CLI 45, MCP 34, src-tauri 42, all passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean in all four; `pnpm test` 261 passed in 25 files; `pnpm typecheck` and `pnpm build` clean. Still open: native GUI check of the toggle.
