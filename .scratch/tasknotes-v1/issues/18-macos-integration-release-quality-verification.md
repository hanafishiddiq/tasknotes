# 18: macOS integration and release-quality verification

**What to build:** Run full end-to-end user/AI/privacy/recovery regressions, accessibility checks, app packaging and startup tests, backup restore drills, audio CPU assessment, and document evidence for each acceptance gate. Never claim release readiness for unverified checks.

**Blocked by:** 12 — MCP stdio integration and safe bulk mutations, 13 — Backup, validated restore, and export, 15 — Menu bar and global Quick Capture, 16 — Quiet ambient and interaction sound, 17 — Large-dataset performance acceptance

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Run macOS packaged-app smoke tests for first launch, persistence, offline use, keyboard capture, menu bar, audio and Notes.
- [ ] Run end-to-end UI/CLI/MCP scenarios that verify shared invariants, audit, privacy, Removed/restore and conflict rejection.
- [ ] Verify backup corruption/restore, database migration, crash-restart consistency and non-destructive recovery drills.
- [ ] Review accessibility and tested performance targets using real evidence, documenting unresolved issues without overstating readiness.
- [ ] Audit the public repository and release bundle to exclude real Notes/Tasks, local databases, backups, secrets and unlicensed sounds.
- [ ] Each earlier ticket has completion evidence and a code review; publish an honest release-readiness checklist with any exceptions.

## Verification

- [ ] Use a reproducible end-to-end release checklist and independent diff review; packaging is not equivalent to notarized public distribution.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
