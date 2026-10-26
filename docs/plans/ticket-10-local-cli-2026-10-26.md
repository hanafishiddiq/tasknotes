# Ticket 10 — Local CLI Task CRUD with concurrency and audit: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at a4621ca (Tickets 06–09 done; schema v9).

## Scope

A local command-line program, `tasknotes`, that a trusted local AI client (or a person) uses to create, read, query, update, remove and restore Tasks and manage Lists through the shared Rust core, with the desktop window closed. Every Task and List change, from the desktop or the CLI, leaves an append-only Audit Entry. Notes, Note Links and folders in the CLI are Ticket 11; MCP is Ticket 12. Both reuse what this ticket builds: the error codes in core, the audit writer in core, and the JSON contract in the CLI library.

## Test seams (pre-agreed in the spec, Q48)

1. **Core interface** against a temporary SQLite file: audit entries, error codes, schema v10 migration and its rollback.
2. **Adapter contract**: the built `tasknotes` binary run as a separate process against a temporary data directory, checking the JSON envelope, paging, revisions, error codes, exit codes, trust refusal, and a UI-versus-CLI write race.

## Decisions made by the implementer (owner may revisit)

- **Binary `tasknotes` in a new crate `crates/tasknotes-cli`** (package `tasknotes-cli`, library + binary). Saya pilih crate terpisah karena desktop app (`src-tauri`, package `tasknotes`) tidak perlu ikut ter-build untuk CLI, dan Ticket 12 bisa memakai library-nya untuk kontrak JSON yang sama.
- **No argument-parsing dependency.** Saya pilih parser kecil sendiri karena setiap kesalahan input harus keluar sebagai JSON envelope dengan kode stabil, dan crate seperti clap belum ada di cache lokal. Only `serde_json` is added.
- **Database location = the desktop's.** `--data-dir <dir>`, else `TASKNOTES_DATA_DIR`, else the OS application-data directory for the app identifier `store.adamaja.tasknotes` (macOS: `~/Library/Application Support/store.adamaja.tasknotes`), file `tasknotes.db`. This is exactly where the desktop app keeps it in a release build. Saya pilih ini karena CLI harus bekerja pada data yang sama dengan UI tanpa konfigurasi tambahan; tests always pass a temporary `--data-dir`.
- **Trust configuration (Q47).** The CLI does nothing with data until the person has granted access: a file `ai-access.json` (`{"schema_version":1,"cli":true}`, mode 0600) in the data directory. `tasknotes access grant` writes it only when run from an interactive terminal where the person types `allow`; `access revoke` and `access status` work anywhere. Without it every data command answers `not_trusted` (exit 6) **before opening, creating or migrating the database**, so an unconfigured CLI cannot even create an empty database. Saya pilih syarat terminal interaktif karena klien AI biasanya menjalankan perintah tanpa terminal, sehingga ia tidak bisa memberi izin pada dirinya sendiri lewat CLI. This is an interface authorization boundary, not protection against a same-user process that edits files directly (ADR-0004, Q47).
- **The whole CLI is AI-facing.** One trust switch covers reads and writes. Saya pilih begitu karena CLI ini dirancang untuk klien AI dan dua tingkat izin belum diminta.
- **Audit Entries live in core** (`audit` module, schema v10 `audit_entries`), written inside the same transaction as the change they describe, so a change and its record commit or roll back together. The store knows which interface it serves: `TaskStore::open` is the desktop, `TaskStore::open_as(path, Interface::Cli)` the CLI (an `mcp` value is reserved for Ticket 12 so it needs no migration). An entry holds: sequence, id, time, interface, action (`task.create`, `task.update`, `task.change`, `task.progress`, `task.details`, `task.move`, `task.remove`, `task.restore`, `list.create`, `list.rename`, `list.remove`, `list.restore`), entity id, outcome (`applied`/`rejected`), error code and revision. **No titles, descriptions, notes or other text**, so an entry can never leak content or secrets. Triggers refuse UPDATE and DELETE on the table (append-only).
- **Which rejections are recorded:** revision conflicts and attempts to change something in Removed (`revision_conflict`, `removed`). Input validation errors (empty title, bad date) are not "meaningful" history and are not recorded. A rejection is written in its own small transaction after the failed one rolled back; if writing it fails, the original error is still returned (the caller must learn about the conflict first). Trust refusals are not recorded because the database is deliberately not opened.
- **Note, folder and link mutations are not audited yet.** Saya pilih menunda ke Ticket 11 karena di sana aturan privasi Exclude from AI untuk Audit Entry diputuskan; the writer is generic so Ticket 11 only adds actions.
- **Stable error codes live in core** (`TaskError::code()`), so CLI and MCP report the same identifiers: `invalid_input`, `invalid_cursor`, `not_found`, `revision_conflict`, `removed`, `name_conflict`, `invalid_transition`, `unchanged`, `unsupported_schema`, `storage_unavailable`; the CLI adds `not_trusted` and `internal`.
- **Exit codes:** 0 ok, 1 internal, 2 `invalid_input`/`invalid_cursor`, 3 `not_found`, 4 `revision_conflict`, 5 rule refusals (`removed`, `name_conflict`, `invalid_transition`, `unchanged`), 6 `not_trusted`, 7 `storage_unavailable`/`unsupported_schema`.
- **JSON envelope, schema version 1, always on stdout, nothing on stderr.** Success: `{"schema_version":1,"ok":true,"item":{...}}` or, for pages, `{"schema_version":1,"ok":true,"items":[...],"next_cursor":null|"..."}`. Failure: `{"schema_version":1,"ok":false,"error":{"code":"...","message":"..."}}`. Keys are snake_case; times are Unix milliseconds. Every Task and List carries `id` and `revision`. Default page size 50, `--limit` 1–100 (progress history 1–50). Cursors are the core's opaque, query-bound cursors.
- **Partial edits.** `task update` without `--title` or `--description`, and `task details` without tags or deadline flags, keep the current value; the `--revision` the caller supplied is still what the core checks, so a stale caller is refused rather than merged.

## Vertical TDD slices

1. Core: schema v10 audit table (data intact; failed migration rolls back); a desktop Task edit leaves an applied entry; a stale edit leaves a rejected entry and changes nothing.
2. Core: every Task and List mutation is audited with its interface; entries refuse UPDATE/DELETE; stable error codes.
3. CLI: unconfigured use is refused without creating a database; `access grant` refuses a non-interactive caller.
4. CLI: `task create/get/list` with envelope, default page 50 and `next_cursor`.
5. CLI: update/change/progress/details/move/remove/restore, `list` commands, `task history`, `audit list`.
6. CLI: malformed input, bad cursor, unsupported schema, locked database → stable codes and exit codes.
7. Race: desktop store and CLI process write the same Task from the same revision; exactly one wins, the other gets `revision_conflict`, both are audited.
8. Gates and evidence.
