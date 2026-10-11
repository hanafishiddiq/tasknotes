# 12: MCP stdio integration and safe bulk mutations

**What to build:** MCP tools with same core operations and schema; no always-listening HTTP port; read/write from an authorized local client. For high-risk bulk operations, produce a preview and require an explicit confirmation step. Test the stdio protocol end-to-end.

**Blocked by:** 10 — Local CLI Task CRUD with concurrency and audit, 11 — Private, AI-optimized Note CRUD in CLI

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] An explicitly trusted local AI client can invoke MCP stdio tools for Task, List and Note CRUD, search, progress and restore while the desktop window is closed.
- [ ] The MCP adapter uses the same Rust core, revision checks, privacy restrictions and versioned JSON responses as the CLI.
- [ ] Tasknotes does not start a network listener; local MCP access requires deliberate client setup.
- [ ] An unconfigured/untrusted MCP client cannot perform mutations or read restricted Note metadata, and errors do not expose the restricted record.
- [ ] High-risk bulk operations first expose a preview and require explicit confirmation, including a stale-preview guard before commit.
- [ ] Untrusted or malformed requests, unauthorized private Notes and concurrent write conflicts produce safe deterministic errors.

## Verification

- [ ] Run MCP stdio protocol integration tests with synthetic records, including privacy and bulk-confirmation negative controls.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
