# 15: One build, install and cleanup

**What to build:** Ship batch 2 in one build, then free the disk.

**Blocked by:** 01, 02, 05–14

**Status:** ready-for-agent

**Implementation:** in progress — installed 2026-11-11; owner click check and cache removal remain

## Acceptance criteria

- [x] All tickets merged to `main` with the owner's confirmation; tests, typecheck, build and `cargo test` green. *(main `1bf9fe3`, CI success; Ticket 08 is a measurement, done on the installed app)*
- [x] One release build is installed to `/Applications` only after the owner confirms; the real store gets a Backup before the new schema first opens it. *(2026-11-11: app quit normally; data folder, old app, old CLI and MCP copied to `~/Library/Application Support/store.adamaja.tasknotes.pre-batch2-20261111/`, checksum identical, integrity ok, schema 15 with 143 Tasks / 4 Notes / 8 Lists; release build of `1bf9fe3` ad-hoc signed with `codesign --force --deep -s -` because the bundle came out linker-signed only; app, `tasknotes` and `tasknotes-mcp` replaced; on first open the store read schema 23, integrity ok, same counts, no network sockets)*
- [ ] The owner's click check on the installed app is recorded.
- [ ] Build caches (`target/`) and finished worktrees are removed after the owner confirms; remaining ones are reported.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han):** build once, then delete caches.

**2026-11-11 — install notes.** `target/release/tasknotes` is shared by the app and the CLI; build the app bundle before the CLI, or the bundle step and the CLI overwrite each other's binary (the bundle had already copied the app binary). Running `tasknotes-mcp` processes from before the install refuse schema 23 (`unsupported_schema`) until their client reconnects. Owner chose to keep build caches until the click check passes.
