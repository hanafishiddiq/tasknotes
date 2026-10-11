# 11: Private, AI-optimized Note CRUD in CLI

**What to build:** Extend the same local CLI schema to Notes and links; enforce Exclude from AI on get/list/search/write and relationship metadata; ensure private Notes are not leaked through error messages or audit/log output. Validate privacy in isolated command-line tests.

**Blocked by:** 07 — Note organization and searching, 08 — Reversible Task–Note links, 10 — Local CLI Task CRUD with concurrency and audit

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] AI-facing CLI Notes CRUD, listing, search and Task–Note linkage operations follow the same versioned envelope and revision rules as Tasks.
- [ ] A Note marked Exclude from AI is invisible through AI list, get, search, linked-Note metadata and export-like operations.
- [ ] AI writes, restores or link changes targeting an Exclude from AI Note are denied without confirming its existence through error details.
- [ ] A user acting through the desktop UI can still view/edit their excluded Note; changing the privacy flag updates the shared core's policy atomically.
- [ ] Logs, error messages and Audit Entries exposed to AI avoid disclosing restricted Note titles or content.

## Verification

- [ ] Exercise direct, search, link, mutation and failure-side privacy negative controls through actual CLI subprocesses.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
