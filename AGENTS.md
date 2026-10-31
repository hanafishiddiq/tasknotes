# Engineering instructions

Tasknotes **completed planning** on 2026-10-11. The approved specification and 18 implementation tickets are under `.scratch/tasknotes-v1/`. Tickets 01–18 are merged to `main` with the owner's approval; no app deployment or migration of the owner's data has happened. Next-version requests are in `.scratch/tasknotes-next/`. Current position: `.scratch/tasknotes-v1/README.md`. Never merge to `main` or touch the owner's real database without the owner's approval, and never merge the unrelated Issue 19 experimental QA branches.

## Agent skills

### Issue tracker

Local Markdown issues and specs under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the five canonical roles, recorded in `Status:` fields. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context glossary in `GLOSSARY.md`; architecture decisions in `docs/adr/`. See `docs/agents/domain.md`.

## Engineering flow

Apply the installed Matt Pocock skills in order:
1. `/grill-with-docs` (uses `/grilling` and `/domain-modeling`) to resolve product decisions.
2. `/to-spec` to produce user stories, design decisions, test seams, and out-of-scope boundaries.
3. `/to-tickets` to plan vertical, demoable slices with explicit blockers, then `/implement` or `/implement-spec`.
4. Drive implementation using `/tdd`; close with `/code-review` for standards and spec adherence; `/retro` for process improvements.

Apply `/codebase-design`: concentrate behavior behind deep modules with small, explicit interfaces. Avoid speculative adapters, excessive abstraction, and duplicated business rules.

## Invariants

- Keep Task Work Status (In Progress / Ready / Completed) distinct from reversible Removed disposition.
- For active items, maintain In Progress → Ready → Completed as the fixed display group order. A started Task remains In Progress while idle; Ready is for not-yet-started work.
- Desktop UI, CLI, and MCP must share the same application-level invariants and write operations.
- AI-facing interfaces must never disclose notes marked Exclude from AI.
- Use revision checks and transactions to protect against concurrent edits.
- User-generated tasks, notes, SQLite files, backups, exports, credentials, and unlicensed audio do **not** belong in Git.
- GitHub repository is public: use synthetic fixtures and scrub screenshots/logs before publication.

## Working rules

- Read `GLOSSARY.md`, relevant ADRs, and issue-tracker instructions before changing behavior.
- Avoid changing agreed product decisions without recording the conflict and asking for approval.
- Prefer testable behavior at the highest practical seam; verify results, don't claim unrun tests.
- Do not silently introduce permanent deletion, cloud sync, or network exposure of private data.
