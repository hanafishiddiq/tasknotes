# 12: MCP stdio integration and safe bulk mutations

**What to build:** MCP tools with same core operations and schema; no always-listening HTTP port; read/write from an authorized local client. For high-risk bulk operations, produce a preview and require an explicit confirmation step. Test the stdio protocol end-to-end.

**Blocked by:** 10 — Local CLI Task CRUD with concurrency and audit, 11 — Private, AI-optimized Note CRUD in CLI

**Status:** ready-for-agent

**Implementation:** implemented, awaiting independent review (branch `feat/ticket-12-mcp-stdio`; plan `docs/plans/ticket-12-mcp-stdio-2026-10-26.md`).

## Acceptance criteria

- [x] An explicitly trusted local AI client can invoke MCP stdio tools for Task, List and Note CRUD, search, progress and restore while the desktop window is closed. — `crates/tasknotes-mcp/tests/crud.rs` (40 tools, exact set in `protocol.rs`; no window involved)
- [x] The MCP adapter uses the same Rust core, revision checks, privacy restrictions and versioned JSON responses as the CLI. — store opened via `ai_access::open_authorized(dir, Interface::Mcp)`, envelopes from `tasknotes_cli::contract`; `crud.rs` conflict test, `privacy.rs`
- [x] Tasknotes does not start a network listener; local MCP access requires deliberate client setup. — stdio transport only; `protocol.rs::the_server_listens_on_no_network_port` (`lsof -i` on the pid is empty); `tasknotes access grant --mcp` + client config
- [x] An unconfigured/untrusted MCP client cannot perform mutations or read restricted Note metadata, and errors do not expose the restricted record. — `access.rs` (refused before arguments/DB; no DB created), `privacy.rs` (excluded Note byte-identical to an unused id)
- [x] High-risk bulk operations first expose a preview and require explicit confirmation, including a stale-preview guard before commit. — confirmation is the person's approval in the desktop (owner decision A): core `tasknotes_core::bulk` (`tests/bulk.rs`), CLI `bulk preview|status`, MCP `bulk_preview`/`bulk_status`, desktop Waiting for approval (`src/approvals.test.tsx`); native click-through still open (below)
- [x] Untrusted or malformed requests, unauthorized private Notes and concurrent write conflicts produce safe deterministic errors. — `malformed.rs`, `access.rs`, `privacy.rs`, `crud.rs::a_stale_revision_is_refused_and_the_desktop_edit_survives`

## Verification

- [x] Run MCP stdio protocol integration tests with synthetic records, including privacy and bulk-confirmation negative controls. — 24 tests spawning the real binary
- [x] Relevant tests pass and evidence is recorded here. — see the 2026-10-26 comment
- [x] Run `/code-review` and address findings before marking complete. — review findings 1–5 and 7 addressed 2026-10-26; finding 6 (Ticket 13 integration) waits for Ticket 13's merge

## Native click-checklist (owner, real desktop app, synthetic data only)

- [ ] With the app closed, ask through MCP (or `tasknotes bulk preview --action remove_tasks --id … --id …`) to remove two synthetic Tasks; open the app: the sidebar shows **Waiting for approval** with count 1.
- [ ] Open it: one sentence ("Move 2 Tasks to Removed"), both titles, "Asked by an MCP client" (or "the tasknotes command"), asked and expiry times. Nothing is removed yet in All tasks.
- [ ] Click **Approve**: the request disappears, the sidebar entry goes away, both Tasks are in Removed; `bulk_status` / `tasknotes bulk status ID` says `applied`; Audit shows the removals under desktop with requested_by mcp/cli.
- [ ] Ask again, then click **Decline**: nothing changes; status `declined`.
- [ ] Ask to move Tasks to a List, rename that List in the window, then open Waiting for approval: the request explains it changed and **Approve** is disabled; Decline works.
- [ ] Ask to remove a folder that holds only a Note marked Exclude from AI: the AI answer shows no count or items; the window lists that Note marked "Excluded from AI"; Approve removes the folder.
- [ ] Keyboard only: Tab reaches the sidebar entry, Approve and Decline; both themes (dark, light) read correctly; at 720px wide the view stays usable.

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

### 2026-10-26 — review fixes + owner decisions (implementer)

Owner decisions (Han, 2026-10-26), recorded here as decided:

- **A.** Bulk changes requested by an AI must be approved by the user in the desktop app; the AI can never approve its own request. `bulk_preview` (CLI `bulk preview`) creates a persisted pending request (schema v14; v13 is Ticket 13's, renumber at merge if needed) holding the exact items, their revisions and a move's target List revision. The desktop shows pending requests (sidebar **Waiting for approval** with a count) with the preview; Approve applies it in the core (stale guard at apply; audit: items under desktop with `requested_by` = the AI interface, plus `bulk.request` / `bulk.approve` / `bulk.decline`); Decline discards. `bulk_status` reports pending / applied / declined / stale / expired and replaces `bulk_confirm`. Requests expire after 24 h and wait while the desktop is closed. Enforced in the core for every AI interface.
- **B.** Removing a List or Note Folder that contains items, via CLI or MCP, goes through the same approval (`needs_approval` on the direct tool); empty ones stay direct. The AI sees no counts or items for such a request; a folder holding only excluded Notes counts as having contents; the desktop shows the full preview. Residual: the AI can infer "holds something it cannot see" from needs_approval on a folder that looks empty to it; never what or how many.

Review fixes: (1) stale refusals → one generic `bulk.approve` audit entry about the request; (2) CLI `tasknotes bulk preview|status` shares `tasknotes_cli::bulk` with MCP; (3) a move binds the target List revision, tested for rename and removal; (4) tool count corrected to 40 with an exact-set test, and a non-JSON stdin line deterministically gets no reply while the server keeps serving; (5) one move-target rule (`contract::destination`), `BulkAction` serde enum, `BulkArgs` tagged enum, tools split by noun, mutex narrowed to opening the store, bulk status changes share `checked_change` (parity test); (6) **open** until Ticket 13 merges into integration: `ensure_daily_backup` at MCP start-up and periodically, bulk writes through `begin_write`, MCP refused during restore; (7) plan, GLOSSARY, client setup and this ticket updated; native click-checklist added.

Evidence (implementer's runs): core 151 passed, CLI 38, MCP 25, src-tauri 41; clippy `-D warnings` and `fmt --check` clean in all four; `pnpm test` 239 passed; `pnpm typecheck`, `pnpm build` ok. Status unchanged.

