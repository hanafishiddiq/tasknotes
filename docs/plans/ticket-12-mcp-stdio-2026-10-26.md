# Ticket 12 — MCP stdio integration and safe bulk mutations: plan (2026-10-26)

Started on the owner's instruction (Tickets 05–18 autonomy: Draft PR per ticket, AI review gate, no merge, synthetic data only). Built on `integration/tasknotes-v1-07-18` at a05b6ec (Tickets 07–11, 14–16). Ticket 13 (backup/restore/export) is built in parallel; this ticket needed no schema change (still v12).

## Scope

A local MCP server, `tasknotes-mcp`, that a trusted AI client starts over stdio. It exposes Task, List, Note, Note Folder, Note Link, Removed and audit tools through the shared core, with the CLI's versioned JSON envelope, plus a preview → confirm flow for high-risk bulk changes. No network listener, no business or privacy rule in the adapter.

## Test seams (pre-agreed in the spec, Q48)

1. **Core interface** (`crates/tasknotes-core/tests/bulk.rs`): preview, apply, stale-preview guard, expiry, selection limits, Exclude from AI, interface binding.
2. **Adapter contract** (`crates/tasknotes-mcp/tests/*.rs`): every test spawns the real `tasknotes-mcp` binary against a temporary data directory and speaks newline-delimited JSON-RPC 2.0 (`initialize`, `notifications/initialized`, `tools/list`, `tools/call`). The desktop side is the core store opened directly on the same file.
3. **CLI** (`crates/tasknotes-cli/tests/access.rs`): `access grant|revoke --mcp`.

## Decisions (saya pilih X karena Y; owner may revisit)

- **Official Rust SDK `rmcp`, pinned `=3.5.1`**, features `server` + `transport-io` only (no macros, no HTTP). Saya pilih SDK resmi karena 3.x stabil (40M+ downloads, rilis rutin) dan menangani negosiasi versi protokol (2024-11-05 … 2026-07-28); 3.5.1 (2026-10-05) dipilih, bukan 3.7.0 (4 hari), agar versi yang dipin sudah beredar beberapa minggu. `schemars` is pinned `=1.2.3` to match rmcp's. The handler is a hand-written `ServerHandler` (`list_tools`, `call_tool`) over a small tool table, so argument errors become our envelope instead of rmcp's protocol errors.
- **Separate crate and binary `crates/tasknotes-mcp` → `tasknotes-mcp`**, not a `tasknotes mcp` subcommand: the CLI binary stays free of tokio/rmcp, and an MCP client config names one executable with no subcommand. It depends on `tasknotes-core` and on the `tasknotes-cli` library.
- **JSON shapes stay in `tasknotes-cli::contract`** (no new shared crate). Saya pilih tidak memindahkan karena modul itu sudah publik dan murni bentuk JSON, dan memindahkannya akan menambah konflik dengan Ticket 13 yang berjalan paralel. The three shapes that need the store (Note with `folder_path`, Note summary, folder with `path`/`note_count`) moved from the CLI's private `notes.rs` into `contract` (`note_read`, `note_summary_read`, `folder_read`) so both adapters share them. `stale_preview` maps to CLI exit code 4.
- **Tool results**: each `tools/call` result carries the envelope as `structuredContent` and as one text block (for clients that read only text), with `isError` = `!ok`. Envelope `schema_version` stays 1.
- **Authorization (Q47)**: the person runs `tasknotes access grant --mcp` in a terminal and types `allow` (same rule as the CLI; `revoke --mcp`, and `access status` now shows `cli` and `mcp`). CLI access does not imply MCP access. The server answers `initialize` and `tools/list` without it (discovery reveals no data), but **every `tools/call` checks `ai_access::is_authorized(dir, Mcp)` before its arguments are read**, then opens the store with `ai_access::open_authorized(dir, Interface::Mcp)`, which refuses before the database is opened, created or migrated. Checking per call means a revoke applies to the next call without restarting the client. Refusal: `not_trusted`, message names `tasknotes access grant --mcp`.
- **Data directory**: `--data-dir DIR`, else `TASKNOTES_DATA_DIR`, else the desktop's directory (`tasknotes_cli::access::data_dir`). stdout carries only protocol messages; diagnostics go to stderr.
- **Calls run one at a time** (a mutex around each call, on a blocking section). Found by a test: pipelined calls on a fresh data directory raced in the first `TaskStore::open` and one got `database is locked`. The core is synchronous SQLite with short transactions, so serializing costs nothing noticeable for one client.
- **Tool set (41)**: `task_list|get|create|update|change|progress|details|move|remove|restore|history|notes`, `list_list|create|rename|remove|restore`, `note_list|get|create|update|move|remove|restore|tasks`, `folder_list|get|create|rename|move|remove|restore`, `link_create|get|remove|restore`, `removed_list`, `audit_list`, `bulk_preview`, `bulk_confirm`. Arguments mirror the CLI flags in snake_case (`revision`, `list_id`/`inbox`, `folder_id`/`no_folder`, `parent_id`/`top`, `deadline: null` clears, `tags: []` clears). Every argument object has `additionalProperties: false`; unknown fields, wrong types and unknown enum values are `invalid_input`. Read tools carry `readOnlyHint`; write tools `destructiveHint: false` (nothing is ever deleted). There is no tool for Exclude from AI or Pin (desktop-only, as in the CLI).
- **What counts as a high-risk bulk change**: any single request that changes several items: remove / restore / move / set status of Tasks, remove / restore of Notes (1–100 distinct ids). These only exist as `bulk_preview` → `bulk_confirm`. Single-item tools need no confirmation (they name a revision and are reversible). Removing one List or Note Folder hides what is inside it, but it is one reversible change restored by one restore, so saya pilih tetap tool satu item (sama seperti CLI), bukan lewat preview.
- **Preview → confirm design (rules in core `tasknotes_core::bulk`, token in the adapter)**:
  - `preview_bulk` runs the whole change in one `BEGIN IMMEDIATE` transaction through the same per-item code as single changes, then rolls back. The preview is therefore exactly what would happen, and is refused (naming the item: `Item 2: …`, with the item's own code) if any one item would be.
  - It returns a `BulkPlan` that only the core can construct: action, each id with the revision it had, the interface, the time.
  - `apply_bulk` replays the plan in one transaction against those revisions and commits all or nothing. Any refusal during apply (an item edited, removed, excluded from AI, or the target List removed) becomes `stale_preview` with a generic message that names no item, so a Note excluded in between is not singled out. Storage errors stay `storage_unavailable`. Plans expire after 10 minutes (`BULK_PREVIEW_TTL_MS`) and apply only through the interface that made them.
  - The MCP adapter keeps plans in memory behind a random 128-bit token, per server process (session), at most 16 unconfirmed. A token is spent on the first `bulk_confirm`, whether it applies or is refused; unknown, reused or other-session tokens are `stale_preview`. Nothing is persisted; a restarted server forgets its previews.
  - Each applied item gets its ordinary Audit Entry (`task.remove`, `task.move`, …) in the same transaction. A preview writes no Audit Entry (rolled back); a stale confirm is not audited.
  - Note items in previews omit `markdown` so 100 Notes stay small.
- **Malformed requests**: unknown tool → JSON-RPC `-32602 "Unknown tool: …"`; unknown method → `-32601`; a `tools/call` without a name or with non-object arguments → JSON-RPC error; lines that are not JSON-RPC 2.0 get at most an id-less `-32600`/`-32700` error (rmcp behaviour) and the server keeps serving.

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

Add `"args": ["--data-dir", "/path/to/data"]` to use another data directory. The client starts the process; nothing listens on a port, and the desktop window need not be open.

## Vertical TDD slices

1. MCP: handshake, `tools/list`, `task_create`/`task_get` shared with the desktop, clean exit on stdin close, no socket (`lsof -i` on the server pid is empty).
2. MCP: untrusted, CLI-only, damaged and wrong-version access files refused on every call without creating the database; existing vs missing ids answer identically; grant and revoke apply to the next call.
3. CLI: `access grant|revoke --mcp`, `status` reports both.
4. MCP: Task/List/Note/folder/link/Removed/audit round trips, cursor paging, revision conflicts against desktop edits (and the rejected Audit Entry).
5. MCP: Exclude from AI negative controls — every Note- or link-naming tool answers byte-identically (after masking the id) for an excluded Note and an unused id; listings, search, folder counts, Task links, Removed view and audit leave it out; no flag in output, unknown `exclude_from_ai` argument refused; stderr empty. Mutation check: opening the store as the desktop made 2 of the 3 privacy tests fail.
6. MCP: malformed lines, unknown method/tool, bad arguments; pipelined calls (red: `database is locked`, fixed by serializing).
7. Core: bulk preview/apply/stale/expiry/limits/privacy/interface.
8. MCP: `bulk_preview`/`bulk_confirm` incl. stale-preview refusal after a desktop edit, spent and cross-session tokens, excluded Notes.

## Evidence (implementer's own runs, 2026-10-26)

`crates/tasknotes-core` cargo test 146 passed; `crates/tasknotes-cli` 34 passed; `crates/tasknotes-mcp` 24 passed (6 stdio test files); `src-tauri` 41 passed; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean in all four; `pnpm test` 233 passed; `pnpm build` ok.

## Not claimed / open

- Not yet exercised with a real Claude Desktop or Claude Code session (the snippets follow their documented `mcpServers` format); the tests act as the client.
- Interface-scoped only (ADR-0004, Q47): any same-user process can write `ai-access.json` or read the SQLite file.
- Two *processes* opening a brand-new data directory at the same moment can still race in the core's first open/migration (`storage_unavailable`, retry succeeds). Serializing fixed it inside one MCP server; the cross-process case belongs to the core's open path (and Ticket 13's writer coordination), not this adapter.
- `deadline` in `task_details` shows `"default": null` in its schema (generated); leaving the field out keeps the Deadline, `null` clears it, as the description says.
