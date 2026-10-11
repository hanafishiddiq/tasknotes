# 10: Local CLI Task CRUD with concurrency and audit

**What to build:** Structured JSON CLI Tasks CRUD, filtering, cursor pagination, revision-protected edits, stable error codes, and append-only Audit Entries. Operate while the desktop window is closed; race UI and CLI writers to prove no silent overwrite.

**Blocked by:** 03 — Progress history and Next Step, 05 — Indexed Task search and filters, 09 — Removed and restore

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] A trusted local CLI can create, read, update, query, remove and restore Tasks and manage Lists through the Rust core without opening the desktop window.
- [ ] AI-facing CLI capabilities require explicit local trust configuration, with safe refusal of unconfigured use. This is an interface authorization boundary, not a promise of defense against processes with unrestricted same-user filesystem access.
- [ ] Machine output uses a documented JSON schema version, stable IDs, revision metadata, 50-item default page size and next_cursor when applicable.
- [ ] Concurrent UI and CLI updates using an obsolete revision fail with a stable conflict error instead of overwriting the winner.
- [ ] Successful mutations and meaningful rejected mutations are reflected in append-only Audit Entries without leaking secrets.
- [ ] Malformed CLI inputs, pagination cursors and transaction failures are rejected safely with stable nonzero exit codes; no listening HTTP port is required.

## Verification

- [ ] Run separate-process CLI contract tests and a concurrent UI-versus-CLI write race using temporary SQLite.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
