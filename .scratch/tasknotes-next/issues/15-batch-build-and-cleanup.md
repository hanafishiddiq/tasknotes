# 15: One build, install and cleanup

**What to build:** Ship batch 2 in one build, then free the disk.

**Blocked by:** 01, 02, 05–14

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] All tickets merged to `main` with the owner's confirmation; tests, typecheck, build and `cargo test` green.
- [ ] One release build is installed to `/Applications` only after the owner confirms; the real store gets a Backup before the new schema first opens it.
- [ ] The owner's click check on the installed app is recorded.
- [ ] Build caches (`target/`) and finished worktrees are removed after the owner confirms; remaining ones are reported.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han):** build once, then delete caches.
