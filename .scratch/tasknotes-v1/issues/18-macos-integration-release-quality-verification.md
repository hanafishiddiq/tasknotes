# 18: macOS integration and release-quality verification

**What to build:** Run full end-to-end user/AI/privacy/recovery regressions, accessibility checks, app packaging and startup tests, backup restore drills, audio CPU assessment, and document evidence for each acceptance gate. Never claim release readiness for unverified checks.

**Blocked by:** 12 — MCP stdio integration and safe bulk mutations, 13 — Backup, validated restore, and export, 15 — Menu bar and global Quick Capture, 16 — Quiet ambient and interaction sound, 17 — Large-dataset performance acceptance

**Status:** ready-for-agent

**Implementation:** automated gates done and code-reviewed on `feat/ticket-18-release-quality`; awaiting the owner's click check. [Readiness checklist](../../../docs/qa/release-readiness-2026-10-31.md), [click check](../../../docs/qa/owner-check-2026-10-30.md).

## Acceptance criteria

- [ ] Run macOS packaged-app smoke tests for first launch, persistence, offline use, keyboard capture, menu bar, audio and Notes. *(first launch, persistence and no network sockets pass on a QA bundle; the rest is in the click check, so this stays open)*
- [x] Run end-to-end UI/CLI/MCP scenarios that verify shared invariants, audit, privacy, Removed/restore and conflict rejection.
- [x] Verify backup corruption/restore, database migration, crash-restart consistency and non-destructive recovery drills.
- [ ] Review accessibility and tested performance targets using real evidence, documenting unresolved issues without overstating readiness. *(automated checks pass; VoiceOver and on-screen checks are in the click check, so this stays open)*
- [x] Audit the public repository and release bundle to exclude real Notes/Tasks, local databases, backups, secrets and unlicensed sounds.
- [x] Each earlier ticket has completion evidence and a code review; publish an honest release-readiness checklist with any exceptions.

## Incoming performance defect gate

- The user reported severe native GUI lag with only one synthetic Task on 2026-10-12. See [Bug 19](19-native-interaction-lag.md). Reproduce and measure it before asserting performance readiness. Do not mark this ticket complete while the bug is unresolved.

## Verification

- [x] Use a reproducible end-to-end release checklist and independent diff review; packaging is not equivalent to notarized public distribution.
- [x] Relevant tests pass and evidence is recorded here.
- [x] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
