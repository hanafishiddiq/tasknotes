# 12: MCP stdio integration and safe bulk mutations

**What to build:** MCP tools with same core operations and schema; no always-listening HTTP port; read/write from an authorized local client. For high-risk bulk operations, produce a preview and require an explicit confirmation step. Test the stdio protocol end-to-end.

**Blocked by:** 10 — Local CLI Task CRUD with concurrency and audit, 11 — Private, AI-optimized Note CRUD in CLI

**Status:** ready-for-agent

**Implementation:** implemented, awaiting independent review (branch `feat/ticket-12-mcp-stdio`; plan `docs/plans/ticket-12-mcp-stdio-2026-10-26.md`).

## Acceptance criteria

- [x] An explicitly trusted local AI client can invoke MCP stdio tools for Task, List and Note CRUD, search, progress and restore while the desktop window is closed. — `crates/tasknotes-mcp/tests/crud.rs` (41 tools; no window involved)
- [x] The MCP adapter uses the same Rust core, revision checks, privacy restrictions and versioned JSON responses as the CLI. — store opened via `ai_access::open_authorized(dir, Interface::Mcp)`, envelopes from `tasknotes_cli::contract`; `crud.rs` conflict test, `privacy.rs`
- [x] Tasknotes does not start a network listener; local MCP access requires deliberate client setup. — stdio transport only; `protocol.rs::the_server_listens_on_no_network_port` (`lsof -i` on the pid is empty); `tasknotes access grant --mcp` + client config
- [x] An unconfigured/untrusted MCP client cannot perform mutations or read restricted Note metadata, and errors do not expose the restricted record. — `access.rs` (refused before arguments/DB; no DB created), `privacy.rs` (excluded Note byte-identical to an unused id)
- [x] High-risk bulk operations first expose a preview and require explicit confirmation, including a stale-preview guard before commit. — core `tasknotes_core::bulk` (`tests/bulk.rs`), MCP `bulk_preview`/`bulk_confirm` (`tasknotes-mcp/tests/bulk.rs`)
- [x] Untrusted or malformed requests, unauthorized private Notes and concurrent write conflicts produce safe deterministic errors. — `malformed.rs`, `access.rs`, `privacy.rs`, `crud.rs::a_stale_revision_is_refused_and_the_desktop_edit_survives`

## Verification

- [x] Run MCP stdio protocol integration tests with synthetic records, including privacy and bulk-confirmation negative controls. — 24 tests spawning the real binary
- [x] Relevant tests pass and evidence is recorded here. — see the 2026-10-26 comment
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

### 2026-10-26 — implementer (implemented, awaiting independent review)

- New crate `crates/tasknotes-mcp` (binary `tasknotes-mcp`), official Rust SDK `rmcp` pinned `=3.5.1`, stdio only. Thin adapter: every tool calls one core operation on a store opened as `Interface::Mcp` and returns the CLI's envelope (`structuredContent` + text, `isError` = `!ok`).
- Access: `tasknotes access grant|revoke --mcp` (terminal + typed `allow`); checked on every `tools/call` before arguments are read or the DB is opened.
- Bulk: core `preview_bulk` (whole change run and rolled back) / `apply_bulk` (all or nothing against the previewed revisions; `stale_preview` if anything changed, expired after 10 min, or another interface); MCP keeps plans behind single-use per-session tokens. Actions: remove/restore/move/set status of Tasks, remove/restore of Notes.
- Found and fixed: pipelined calls raced on the first DB open (`database is locked`); calls are now serialized.
- Evidence (implementer's runs): core 146 passed, CLI 34, MCP 24, src-tauri 41; clippy `-D warnings` and `fmt --check` clean in all four; `pnpm test` 233 passed; `pnpm build` ok.
- Open: `/code-review` not yet run; no real Claude client session yet; cross-process first-open race noted in the plan.

