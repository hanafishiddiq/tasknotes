# Ticket 12 — MCP stdio integration and safe bulk mutations: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at a05b6ec (Tickets 07–11, 14–16). Ticket 13 (backup/restore/export) is built in parallel; this ticket first needed no schema change; the 2026-10-26 owner decisions added schema v14 (`bulk_requests`, `audit_entries.requested_by`), leaving v13 to Ticket 13.

## Scope

A local MCP server, `tasknotes-mcp`, that a trusted AI client starts over stdio. It exposes Task, List, Note, Note Folder, Note Link, Removed and audit tools through the shared core, with the CLI's versioned JSON envelope, plus a preview → confirm flow for high-risk bulk changes. No network listener, no business or privacy rule in the adapter.

## Test seams (pre-agreed in the spec, Q48)

1. **Core interface** (`crates/tasknotes-core/tests/bulk.rs`): request, desktop-only approve/decline, persisted requests, stale guard (items and target List), generic stale audit, 24 h expiry, selection limits, Exclude from AI, container removals needing approval, status-change parity with `change_task`.
2. **Adapter contract** (`crates/tasknotes-mcp/tests/*.rs`): every test spawns the real `tasknotes-mcp` binary against a temporary data directory and speaks newline-delimited JSON-RPC 2.0 (`initialize`, `notifications/initialized`, `tools/list`, `tools/call`). The desktop side is the core store opened directly on the same file.
3. **CLI** (`crates/tasknotes-cli/tests/access.rs`, `bulk.rs`): `access grant|revoke --mcp`; `bulk preview|status`, `needs_approval`.
4. **Desktop** (`src/approvals.test.tsx`, Vitest over the Tauri command seam): Waiting for approval count, preview, Approve, Decline, stale, folder contents, refresh on focus.

## Decisions (saya pilih X karena Y; owner may revisit)

- **Official Rust SDK `rmcp`, pinned `=3.5.1`**, features `server` + `transport-io` only (no macros, no HTTP). Saya pilih SDK resmi karena 3.x stabil (40M+ downloads, rilis rutin) dan menangani negosiasi versi protokol (2024-11-05 … 2026-07-28); 3.5.1 (2026-10-05) dipilih, bukan 3.7.0 (4 hari), agar versi yang dipin sudah beredar beberapa minggu. `schemars` is pinned `=1.2.3` to match rmcp's. The handler is a hand-written `ServerHandler` (`list_tools`, `call_tool`) over a small tool table, so argument errors become our envelope instead of rmcp's protocol errors.
- **Separate crate and binary `crates/tasknotes-mcp` → `tasknotes-mcp`**, not a `tasknotes mcp` subcommand: the CLI binary stays free of tokio/rmcp, and an MCP client config names one executable with no subcommand. It depends on `tasknotes-core` and on the `tasknotes-cli` library.
- **JSON shapes stay in `tasknotes-cli::contract`** (no new shared crate). Saya pilih tidak memindahkan karena modul itu sudah publik dan murni bentuk JSON, dan memindahkannya akan menambah konflik dengan Ticket 13 yang berjalan paralel. The three shapes that need the store (Note with `folder_path`, Note summary, folder with `path`/`note_count`) moved from the CLI's private `notes.rs` into `contract` (`note_read`, `note_summary_read`, `folder_read`) so both adapters share them. `stale_preview` maps to CLI exit code 4.
- **Tool results**: each `tools/call` result carries the envelope as `structuredContent` and as one text block (for clients that read only text), with `isError` = `!ok`. Envelope `schema_version` stays 1.
- **Authorization (Q47)**: the person runs `tasknotes access grant --mcp` in a terminal and types `allow` (same rule as the CLI; `revoke --mcp`, and `access status` now shows `cli` and `mcp`). CLI access does not imply MCP access. The server answers `initialize` and `tools/list` without it (discovery reveals no data), but **every `tools/call` checks `ai_access::is_authorized(dir, Mcp)` before its arguments are read**, then opens the store with `ai_access::open_authorized(dir, Interface::Mcp)`, which refuses before the database is opened, created or migrated. Checking per call means a revoke applies to the next call without restarting the client. Refusal: `not_trusted`, message names `tasknotes access grant --mcp`.
- **Data directory**: `--data-dir DIR`, else `TASKNOTES_DATA_DIR`, else the desktop's directory (`tasknotes_cli::access::data_dir`). stdout carries only protocol messages; diagnostics go to stderr.
- **Only opening the store is serialized** (a mutex around `open_authorized`). Found by a test: pipelined calls on a fresh data directory raced in the first `TaskStore::open`/migration and one got `database is locked`. After opening, each call uses its own connection and waits out SQLite's busy timeout like the CLI and desktop do (review fix 5; the pipelined test still passes, 15/15 repeated runs).
- **Tool set (40)**: `task_list|get|create|update|change|progress|details|move|remove|restore|history|notes`, `list_list|create|rename|remove|restore`, `note_list|get|create|update|move|remove|restore|tasks`, `folder_list|get|create|rename|move|remove|restore`, `link_create|get|remove|restore`, `removed_list`, `audit_list`, `bulk_preview`, `bulk_status` (exact set asserted in `protocol.rs`). Tools live in `tools_task.rs`, `tools_note.rs`, `tools_bulk.rs` over `tools.rs`. Arguments mirror the CLI flags in snake_case (`revision`, `list_id`/`inbox`, `folder_id`/`no_folder`, `parent_id`/`top`, `deadline: null` clears, `tags: []` clears). Every argument object has `additionalProperties: false`; unknown fields, wrong types and unknown enum values are `invalid_input`. Read tools carry `readOnlyHint`; write tools `destructiveHint: false` (nothing is ever deleted). There is no tool for Exclude from AI or Pin (desktop-only, as in the CLI).
- **Bulk Changes need the person's approval in the desktop (owner decisions A and B, Han 2026-10-26; replaces the earlier preview → token → confirm design).** An AI (CLI or MCP) can only ask; no AI path can apply a Bulk Change, and the rule is the core's (`tasknotes_core::bulk`), so it holds for every AI interface:
  - What needs approval: one request that removes / restores / moves / sets the status of several Tasks, or removes / restores several Notes (1–100 distinct ids); and, through an AI interface, removing a List or Note Folder that holds anything (any Task; any subfolder or Note, Removed or not, Exclude from AI included). `list_remove` / `folder_remove` (and the CLI's `list|folder remove`) on such a container answer `needs_approval` (CLI exit 5) without saying how much it holds; an empty one is still removed directly. Restores stay direct.
  - `request_bulk` (MCP `bulk_preview`, CLI `tasknotes bulk preview --action A --id ID…`) runs the whole change in one `BEGIN IMMEDIATE` transaction through the same per-item code as single changes and rolls it back, so the preview is exactly what would happen, refused (`Item 2: …`, with the item's code) if any item would be. It then stores a pending request in `bulk_requests` (schema v14; v13 is Ticket 13's, renumber at merge if needed): action, every id with the revision it had and, for a move to a List, that List's revision. Requests live in the database, so they wait while the desktop is closed and survive the MCP server. They expire after 24 h (`BULK_REQUEST_TTL_MS`).
  - The answer to the AI: `request_id`, `status: pending`, the action (with `list_id` / `status`), `count`, times, a `message` saying it waits for the person in the Tasknotes app, and each item before/after (Notes without `markdown`). For a List or folder removal: no `count` and no `items`, only that it waits.
  - `bulk_status` (CLI `tasknotes bulk status ID`) reports `pending`, `applied`, `declined`, `stale` or `expired`. `bulk_confirm` and the in-memory tokens are gone.
  - The desktop's **Waiting for approval** view (sidebar entry with a count while anything waits; asked on start, on focus and every 30 s) shows each request as one sentence plus everything it changes or would hide, private Notes marked "Excluded from AI". Approve replays the stored plan in one transaction, all or nothing, under the AI's privacy rules (a Note excluded since makes it stale). Item Audit Entries are recorded under `desktop` with `requested_by` naming the AI; the request has `bulk.request` (AI), `bulk.approve` / `bulk.decline` (desktop) entries. If anything named, or the move's target List (renamed, removed, any revision change), changed since the request, nothing is applied, the request becomes `stale`, and **one generic Audit Entry** (`bulk.approve`, rejected, `stale_preview`, entity = the request id, no revision) records the refusal, naming no item, so a Note excluded in between is not singled out. A stale request offers only Decline.
  - Residual signal, accepted by decision B: an AI can tell that a folder whose AI-visible contents are empty still holds something (excluded Notes), because removing it needs approval instead of succeeding. It never learns how many, or which.
  - Bulk status changes go through `change_task`'s own pre-checks (`checked_change`) and transition code; a test compares twins changed one at a time and through approval.
- **Arguments:** `tasknotes_cli::bulk::BulkArgs`, a serde-tagged enum (`action` + per-action fields), shared by CLI and MCP; action names are the core's serde names (no string tables). The MCP tool advertises a flat object schema built from it (clients reject a root `oneOf`). One move-target rule, `contract::destination` ("exactly one of list_id or inbox: true" / "--list ID or --inbox"), serves single moves, note/folder moves and bulk moves.
- **Malformed requests**: unknown tool → JSON-RPC `-32602 "Unknown tool: …"`; unknown method → `-32601`; a `tools/call` without a name or with non-object arguments → JSON-RPC error; a line that is not JSON gets no reply at all (rmcp, like the other official SDKs, skips it) and the next request is answered (`malformed.rs`); JSON that is not JSON-RPC 2.0 gets an id-less `-32600`; the server keeps serving.

## Client setup

Build once: `cargo build --release --manifest-path crates/tasknotes-mcp/Cargo.toml` (binary: `crates/tasknotes-mcp/target/release/tasknotes-mcp`). Then the person grants access in a terminal: `tasknotes access grant --mcp` (type `allow`).

Claude Code:

```sh
claude mcp add tasknotes -- /absolute/path/to/tasknotes-mcp
```

Claude Desktop (`~/Library/Application Support/Claude/claude_desktop_config.json`), or a project `.mcp.json`:

```json
{
  "mcpServers": {
    "tasknotes": {
      "command": "/absolute/path/to/tasknotes-mcp",
      "args": []
    }
  }
}
```

Add `"args": ["--data-dir", "/path/to/data"]` to use another data directory. The client starts the process; nothing listens on a port, and the desktop window need not be open. Bulk Changes (and removing a List or folder that holds anything) are only *requested* through MCP; they apply after you approve them in Tasknotes under **Waiting for approval**, which you can do whenever you next open the app.

## Vertical TDD slices

1. MCP: handshake, `tools/list`, `task_create`/`task_get` shared with the desktop, clean exit on stdin close, no socket (`lsof -i` on the server pid is empty).
2. MCP: untrusted, CLI-only, damaged and wrong-version access files refused on every call without creating the database; existing vs missing ids answer identically; grant and revoke apply to the next call.
3. CLI: `access grant|revoke --mcp`, `status` reports both.
4. MCP: Task/List/Note/folder/link/Removed/audit round trips, cursor paging, revision conflicts against desktop edits (and the rejected Audit Entry).
5. MCP: Exclude from AI negative controls — every Note- or link-naming tool answers byte-identically (after masking the id) for an excluded Note and an unused id; listings, search, folder counts, Task links, Removed view and audit leave it out; no flag in output, unknown `exclude_from_ai` argument refused; stderr empty. Mutation check: opening the store as the desktop made 2 of the 3 privacy tests fail.
6. MCP: malformed lines, unknown method/tool, bad arguments; pipelined calls (red: `database is locked`, fixed by serializing).
7. Core: bulk preview/apply/stale/expiry/limits/privacy/interface.
8. MCP: `bulk_preview`/`bulk_status` over the persisted request; desktop approval, decline and staleness seen from MCP; no approving tool; excluded Notes; private folder reveals nothing.
9. Desktop: Waiting for approval view.

## Review fixes and owner decisions (2026-10-26)

1. Stale approvals record one generic `bulk.approve` refusal about the request (core `a_change_after_the_request_makes_it_stale_with_a_generic_audit_entry`, `bulk_notes_respect_exclude_from_ai_like_single_changes`).
2. CLI shares the flow: `tasknotes bulk preview|status` over `tasknotes_cli::bulk`, used by MCP too (CLI `tests/bulk.rs`).
3. A move binds its target List's revision; renaming or removing it makes the request stale (core `renaming_or_removing_the_target_list_makes_a_move_stale`, MCP `moves_bind_the_target_list_and_use_the_shared_target_rule`).
4. Tool count is 40 (exact set asserted); a non-JSON line gets no reply and the server answers the next request.
5. Shared move-target rule; `BulkAction` is a serde enum and `BulkArgs` a tagged enum; tools split by noun; mutex narrowed to opening; bulk status changes share `checked_change`.
6. **Open:** Ticket 13 integration (`ensure_daily_backup` at MCP start-up and periodically, bulk writes through the epoch-checked `begin_write`, MCP refused during restore, schema renumbering) waits for Ticket 13 to merge into `integration/tasknotes-v1-07-18`.

## Evidence (implementer's own runs, 2026-10-26)

First pass: core 146, CLI 34, MCP 24, src-tauri 41, `pnpm test` 233.

After review fixes and owner decisions (implementer's runs): `crates/tasknotes-core` cargo test 151 passed; `crates/tasknotes-cli` 38; `crates/tasknotes-mcp` 25 (1 unit + 24 stdio); `src-tauri` 41; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean in all four; `pnpm test` 239 passed (23 files, incl. `src/approvals.test.tsx`); `pnpm typecheck` and `pnpm build` ok. A mutation check (Approve left enabled for a stale request) failed the stale UI test as expected.

## Not claimed / open

- Not yet exercised with a real Claude Desktop or Claude Code session (the snippets follow their documented `mcpServers` format); the tests act as the client.
- Interface-scoped only (ADR-0004, Q47): any same-user process can write `ai-access.json` or read the SQLite file.
- Two *processes* opening a brand-new data directory at the same moment can still race in the core's first open/migration (`storage_unavailable`, retry succeeds). Serializing fixed it inside one MCP server; the cross-process case belongs to the core's open path (and Ticket 13's writer coordination), not this adapter.
- `deadline` in `task_details` shows `"default": null` in its schema (generated); leaving the field out keeps the Deadline, `null` clears it, as the description says.
