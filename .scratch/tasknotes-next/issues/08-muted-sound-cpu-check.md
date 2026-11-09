# 08: Measure what muted sound costs

**What to build:** Measure CPU of the real app in three states and report (spec story 23).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] CPU of the Tasknotes processes (including WebKit audio XPC) is sampled for the same window of time with sound on, only rain muted, and rain plus interaction sounds muted, on a synthetic store with the screen awake.
- [ ] Results and method are recorded here; if "only rain muted" keeps an idle audio device measurably busy, a fix is proposed to the owner before any change.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09:** code reading shows muting rain stops its sources and that the audio device is suspended only when both channels are muted (`src/lib/sound/sound-engine.ts`, `quiet()`); CPU was never measured on the native app.
