# 15: One build, install and cleanup

**What to build:** Ship batch 2 in one build, then free the disk.

**Blocked by:** 01, 02, 05–14

**Status:** ready-for-agent

**Implementation:** completed — installed 2026-11-11; click check 2026-11-12; caches and finished worktrees removed 2026-11-11

## Acceptance criteria

- [x] All tickets merged to `main` with the owner's confirmation; tests, typecheck, build and `cargo test` green. *(main `1bf9fe3`, CI success; Ticket 08 is a measurement, done on the installed app)*
- [x] One release build is installed to `/Applications` only after the owner confirms; the real store gets a Backup before the new schema first opens it. *(2026-11-11: app quit normally; data folder, old app, old CLI and MCP copied to `~/Library/Application Support/store.adamaja.tasknotes.pre-batch2-20261111/`, checksum identical, integrity ok, schema 15 with 143 Tasks / 4 Notes / 8 Lists; release build of `1bf9fe3` ad-hoc signed with `codesign --force --deep -s -` because the bundle came out linker-signed only; app, `tasknotes` and `tasknotes-mcp` replaced; on first open the store read schema 23, integrity ok, same counts, no network sockets)*
- [x] The owner's click check on the installed app is recorded. _Evidence: the ten-step check, done by the agent through Cua at the owner's request on 2026-11-12, all ten as expected; recorded in `progress/tasknotes-batch-2.md` and each ticket's 2026-11-12 comment._
- [x] Build caches (`target/`) and finished worktrees are removed after the owner confirms; remaining ones are reported. _Evidence: removed with the owner's OK on 2026-11-11 (PR #46, about 31 GB; the kept worktrees are listed in `progress/tasknotes-batch-3.md`); the Ticket 17 worktrees went on 2026-11-12._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han):** build once, then delete caches.

**2026-11-11 — install notes.** `target/release/tasknotes` is shared by the app and the CLI; build the app bundle before the CLI, or the bundle step and the CLI overwrite each other's binary (the bundle had already copied the app binary). Running `tasknotes-mcp` processes from before the install refuse schema 23 (`unsupported_schema`) until their client reconnects. Owner chose to keep build caches until the click check passes.
