# Engineering instructions

Tasknotes is in **planning**. Until the user approves completion of `/grilling` and the resulting specification, change documentation and tooling only: do not implement app features.

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
- For active items, maintain In Progress → Ready → Completed as the fixed display group order.
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
