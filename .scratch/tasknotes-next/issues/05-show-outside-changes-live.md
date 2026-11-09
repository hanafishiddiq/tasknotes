# 05: Show CLI and MCP changes without reopening

**What to build:** Already implemented in Draft PR #23 (`claude/youthful-elion-4ee635`): the window picks up Tasks, Lists, Notes and Note Folders changed by the CLI or MCP, keeping focus, scroll and unsaved drafts.

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** in progress — PR #23 merged 2026-11-09; the real-build check waits for the single batch-2 build (Ticket 15)

## Acceptance criteria

- [x] Independent review of PR #23 with findings addressed. *(standards + spec review 2026-11-09; fixes 0ed8660…c7e247e, Vitest 980, Rust green; combined with #22/#24/#25: Vitest 999, Rust core 239 / CLI 46 / MCP 35 / app 42)*
- [ ] A change made through the CLI appears in the open window within a few seconds without a reopen, on the real build.
- [ ] A Task with unsaved edits stays visible after such a reload until saved or discarded.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09:** recorded as a batch-2 ticket so the single build includes it. Implementation status: in progress — awaiting review.
