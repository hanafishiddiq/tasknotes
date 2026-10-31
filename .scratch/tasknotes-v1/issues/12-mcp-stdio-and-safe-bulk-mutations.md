# 12: MCP stdio integration and safe bulk mutations

**What to build:** MCP tools with same core operations and schema; no always-listening HTTP port; read/write from an authorized local client. For high-risk bulk operations, produce a preview and require an explicit confirmation step. Test the stdio protocol end-to-end.

**Blocked by:** 10 — Local CLI Task CRUD with concurrency and audit, 11 — Private, AI-optimized Note CRUD in CLI

**Status:** ready-for-agent

**Implementation:** implemented and merged into `integration/tasknotes-v1-07-18` with Ticket 13 on 2026-10-30; independent review of the meeting done and its findings resolved (see the last comment). Plan: `docs/plans/ticket-12-mcp-stdio-2026-10-26.md`.

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
- [x] Run `/code-review` and address findings before marking complete. — review findings 1–5 and 7 addressed 2026-10-26; finding 6 (Ticket 13 integration) addressed 2026-10-30 at the merge

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

### 2026-10-30 — Ticket 12 meets Ticket 13 (integration)

Both branches started from a05b6ec and merged into `integration/tasknotes-v1-07-18`. Backups take schema v13, Bulk Change requests v14; every write, a Bulk Change's included, goes through the epoch-checked `begin_write`.

Fixed at the merge, each with a test written first:

- A restore dropped `requested_by` from the Audit Entries it carries over (`a_restore_keeps_who_asked_for_an_approved_bulk_change`).
- The MCP server takes the day's backup: it checks at start-up, then hourly, and every 5 s until access and a database both exist; a failure goes to stderr and never reaches a tool (`crates/tasknotes-mcp/tests/backup.rs`).
- A revision read before a restore could match again and land on a later server call, because each call opens its own store. The server now remembers the Restore Epoch and refuses the first call after a restore with `database_restored` (`a_write_from_before_a_restore_is_refused_once_and_the_persons_edit_survives`).
- The desktop's approval poll bypassed the wrapper that reloads after a restore made elsewhere, so an idle window never reloaded (`src/approvals.test.tsx`).

**Owner decisions (Han, 2026-10-30), implemented with tests written first:**

1. A Note marked Exclude from AI at restore time stays excluded, even if the Backup holds it as shared (`a_note_private_before_or_after_the_backup_stays_private_through_a_restore`, `a_restore_does_not_open_a_private_note_to_a_running_server`). A Note excluded in the Backup also stays excluded; sharing it again is a choice made in the window.
2. The person's decision on a Bulk Change stands: a request decided since the Backup keeps that decision, and one still waiting keeps waiting until it expires (`a_decision_stands_through_a_restore`, `a_request_still_waiting_keeps_waiting_through_a_restore`). A request made after the Backup is not in the restored data; its Audit Entry remains.

**Known and left for Ticket 18:**

- ~~Two processes opening a database at the same moment can make one open fail.~~ Fixed the same day: they take turns upgrading (`stores_opening_at_the_same_moment_all_succeed`).
- A database created by a build of this branch alone (v14 with no `database_epoch` table) does not open. Throw such QA directories away; no real data was ever opened by that build.
- ~~The Restore Epoch guards one running MCP server only.~~ Fixed: a restore retires the number of whatever it replaces in `retired_revisions` (schema v15), which outlives restores, and gives an older row a new number, so a client holding an old one is refused, across restarts and CLI commands too (`crates/tasknotes-core/tests/restore_twice.rs`, `a_cli_write_from_before_a_restore_does_not_overwrite_the_persons_edit`). Restoring the same content twice in a row also moves the number: a false `revision_conflict`, which fails safe and only asks the client to read again.

